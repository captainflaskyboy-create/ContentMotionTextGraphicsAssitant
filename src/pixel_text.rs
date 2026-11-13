//! Nearest-neighbour glyph masks with independently filtered, bounded cached shadows.
use crate::{
    error::{Error, Result},
    fonts::PixelFont,
    graphics::Graphics,
    kinetic::{self, KineticRequest, TimedWord},
    scene::Theme,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use windows::Win32::Graphics::{
    Direct2D::{Common::*, *},
    DirectWrite::*,
};
use windows_numerics::Matrix3x2;

const CACHE_BUDGET: usize = 64 * 1024 * 1024;
fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
fn ink(name: &str) -> Result<D2D1_COLOR_F> {
    let c = Theme::default().color(name)?;
    Ok(D2D1_COLOR_F {
        r: c[0],
        g: c[1],
        b: c[2],
        a: 1.,
    })
}

struct Metrics {
    glyphs: Vec<u16>,
    advances: Vec<f32>,
    bounds: D2D_RECT_F,
}
fn outline(
    g: &Graphics,
    font: &PixelFont,
    glyphs: &[u16],
    advances: &[f32],
) -> Result<ID2D1PathGeometry> {
    unsafe {
        let path = g.context.GetFactory()?.CreatePathGeometry()?;
        let sink = path.Open()?;
        font.face.GetGlyphRunOutline(
            8.,
            glyphs.as_ptr(),
            Some(advances.as_ptr()),
            None,
            glyphs.len() as u32,
            false,
            false,
            &sink,
        )?;
        sink.Close()?;
        Ok(path)
    }
}
fn measure(g: &Graphics, font: &PixelFont, text: &str, index: usize) -> Result<Metrics> {
    unsafe {
        let glyphs = font.glyphs(text, index)?;
        let mut design = vec![DWRITE_GLYPH_METRICS::default(); glyphs.len()];
        let mut metrics = DWRITE_FONT_METRICS::default();
        font.face.GetMetrics(&mut metrics);
        font.face.GetDesignGlyphMetrics(
            glyphs.as_ptr(),
            glyphs.len() as u32,
            design.as_mut_ptr(),
            false,
        )?;
        let advances: Vec<f32> = design
            .iter()
            .map(|m| 8. * m.advanceWidth as f32 / metrics.designUnitsPerEm as f32)
            .collect();
        let raw = outline(g, font, &glyphs, &advances)?.GetBounds(None)?;
        let bounds = D2D_RECT_F {
            left: raw.left.floor(),
            top: raw.top.floor(),
            right: raw.right.ceil(),
            bottom: raw.bottom.ceil(),
        };
        if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
            return Err(Error::new(
                "invalid_scene",
                format!("words[{index}]: word has no visible glyphs"),
            ));
        }
        Ok(Metrics {
            glyphs,
            advances,
            bounds,
        })
    }
}
struct CachedWord {
    glyph: ID2D1Bitmap1,
    shadow: ID2D1Bitmap1,
    width: f32,
    height: f32,
    padding: f32,
    bytes: usize,
    last_used: u64,
}
struct Scratch {
    bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
}
fn scratch<'a>(
    slot: &'a mut Option<Scratch>,
    g: &Graphics,
    w: u32,
    h: u32,
) -> Result<&'a ID2D1Bitmap1> {
    if slot.as_ref().is_none_or(|s| s.width < w || s.height < h) {
        let width = w.max(slot.as_ref().map_or(0, |s| s.width));
        let height = h.max(slot.as_ref().map_or(0, |s| s.height));
        *slot = Some(Scratch {
            bitmap: g.transparent_bitmap(width, height, D2D1_BITMAP_OPTIONS_TARGET)?,
            width,
            height,
        });
    }
    Ok(&slot.as_ref().unwrap().bitmap)
}
pub struct PixelCompositor {
    pub output: ID2D1Bitmap1,
    cpu: ID2D1Bitmap1,
    pixels: Vec<u8>,
    font: PixelFont,
    pub words: Vec<TimedWord>,
    metrics: HashMap<String, Metrics>,
    pub fitted_font_size: f32,
    unit: f32,
    cache: HashMap<(String, bool), CachedWord>,
    cache_bytes: usize,
    peak_cache_bytes: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    mask_scratch: Option<Scratch>,
    shadow_scratch: Option<Scratch>,
    shadow_effect: ID2D1Effect,
    glyph_brush: ID2D1SolidColorBrush,
}
impl PixelCompositor {
    pub fn new(g: &Graphics, request: &KineticRequest) -> Result<Self> {
        unsafe {
            let words = request.validate()?;
            let font = PixelFont::new()?;
            let mut metrics = HashMap::new();
            let unit =
                (request.canvas.width as f32 / 1920.).min(request.canvas.height as f32 / 1080.);
            let style = &request.style;
            let max_scale = [
                style.initial_scale,
                style.peak_scale,
                style.settle_scale,
                style.hold_scale,
            ]
            .into_iter()
            .fold(0., f32::max);
            let sigma = style.shadow.blur_radius / 2.;
            let spread = 3. * sigma;
            let extra_x = 2. * (spread + style.shadow.offset_x.abs() + 2.) * unit * max_scale;
            let extra_y = 2. * (spread + style.shadow.offset_y.abs() + 2.) * unit * max_scale;
            let available_w = request.canvas.width as f32 - 2. * style.margin * unit - extra_x;
            let available_h = request.canvas.height as f32 - 2. * style.margin * unit - extra_y;
            let mut fitted = style.font_size;
            for word in &words {
                if !metrics.contains_key(&word.text) {
                    metrics.insert(
                        word.text.clone(),
                        measure(g, &font, &word.text, word.index)?,
                    );
                }
                let m = &metrics[&word.text];
                let emphasis = if word.emphasis {
                    style.emphasis_scale
                } else {
                    1.
                };
                fitted = fitted.min(
                    available_w * 8.
                        / ((m.bounds.right - m.bounds.left) * unit * emphasis * max_scale),
                );
                fitted = fitted.min(
                    available_h * 8.
                        / ((m.bounds.bottom - m.bounds.top) * unit * emphasis * max_scale),
                );
            }
            if fitted < style.min_font_size {
                return Err(Error::new("invalid_scene",format!("words: sequence requires font size {fitted:.2}, below style.min_font_size {}; shorten long words, reduce margins/emphasis, or change the canvas",style.min_font_size)));
            }
            let shadow_effect = g.context.CreateEffect(&CLSID_D2D1Shadow)?;
            shadow_effect.SetValue(
                D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &(sigma * unit).to_ne_bytes(),
            )?;
            let c = Theme::default().color(&style.shadow.color)?;
            let components = [c[0], c[1], c[2], style.shadow.opacity];
            let bytes: Vec<u8> = components.into_iter().flat_map(f32::to_ne_bytes).collect();
            shadow_effect.SetValue(
                D2D1_SHADOW_PROP_COLOR.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR4,
                &bytes,
            )?;
            Ok(Self {
                output: g.bitmap(
                    request.canvas.width,
                    request.canvas.height,
                    D2D1_BITMAP_OPTIONS_TARGET,
                )?,
                cpu: g.bitmap(
                    request.canvas.width,
                    request.canvas.height,
                    D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                )?,
                pixels: Vec::new(),
                font,
                words,
                metrics,
                fitted_font_size: fitted,
                unit,
                cache: HashMap::new(),
                cache_bytes: 0,
                peak_cache_bytes: 0,
                cache_hits: 0,
                cache_misses: 0,
                mask_scratch: None,
                shadow_scratch: None,
                shadow_effect,
                glyph_brush: g.context.CreateSolidColorBrush(&ink(&style.color)?, None)?,
            })
        }
    }
    pub fn report(&self) -> Value {
        json!({"font":"Press Start 2P","font_source":"embedded","font_size":self.fitted_font_size,"pixel_grid_size":8,"words":self.words,"cache_budget_bytes":CACHE_BUDGET,"cache_bytes":self.cache_bytes,"peak_cache_bytes":self.peak_cache_bytes,"cache_hits":self.cache_hits,"cache_misses":self.cache_misses})
    }
    fn prepare_word(
        &mut self,
        g: &Graphics,
        r: &KineticRequest,
        index: usize,
        frame: u64,
    ) -> Result<()> {
        unsafe {
            let word = &self.words[index];
            let key = (word.text.clone(), word.emphasis);
            if let Some(cached) = self.cache.get_mut(&key) {
                cached.last_used = frame;
                self.cache_hits += 1;
                return Ok(());
            }
            self.cache_misses += 1;
            let m = &self.metrics[&word.text];
            let w = (m.bounds.right - m.bounds.left) as u32;
            let h = (m.bounds.bottom - m.bounds.top) as u32;
            let geometry = outline(g, &self.font, &m.glyphs, &m.advances)?;
            let mask = scratch(&mut self.mask_scratch, g, w, h)?;
            let c = &g.context;
            c.SetTarget(mask);
            c.SetTransform(&Matrix3x2 {
                M31: -m.bounds.left,
                M32: -m.bounds.top,
                ..Matrix3x2::identity()
            });
            c.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            c.BeginDraw();
            c.Clear(Some(&D2D1_COLOR_F::default()));
            c.FillGeometry(&geometry, &self.glyph_brush, None);
            c.EndDraw(None, None)?;
            let glyph = g.transparent_bitmap(w, h, D2D1_BITMAP_OPTIONS_NONE)?;
            glyph.CopyFromBitmap(
                None,
                mask,
                Some(&D2D_RECT_U {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: h,
                }),
            )?;
            let factor = self.fitted_font_size / 8.
                * self.unit
                * if word.emphasis {
                    r.style.emphasis_scale
                } else {
                    1.
                };
            let width = (w as f32 * factor).round().max(1.);
            let height = (h as f32 * factor).round().max(1.);
            let padding = ((1.5 * r.style.shadow.blur_radius
                + r.style
                    .shadow
                    .offset_x
                    .abs()
                    .max(r.style.shadow.offset_y.abs())
                + 2.)
                * self.unit)
                .ceil()
                + 2.;
            let sw = width as u32 + 2 * padding as u32;
            let sh = height as u32 + 2 * padding as u32;
            let bytes = (w as usize * h as usize + sw as usize * sh as usize) * 4;
            if bytes > CACHE_BUDGET {
                return Err(Error::new(
                    "invalid_scene",
                    "A kinetic word exceeds the 64 MiB texture cache budget",
                ));
            }
            while self.cache_bytes + bytes > CACHE_BUDGET {
                let oldest = self
                    .cache
                    .iter()
                    .min_by_key(|(_, v)| v.last_used)
                    .map(|(k, _)| k.clone())
                    .unwrap();
                self.cache_bytes -= self.cache.remove(&oldest).unwrap().bytes;
            }
            let source = scratch(&mut self.shadow_scratch, g, sw, sh)?;
            c.SetTarget(source);
            c.SetTransform(&Matrix3x2::identity());
            c.BeginDraw();
            c.Clear(Some(&D2D1_COLOR_F::default()));
            c.DrawBitmap(
                &glyph,
                Some(&rect(
                    padding + r.style.shadow.offset_x * self.unit,
                    padding + r.style.shadow.offset_y * self.unit,
                    width,
                    height,
                )),
                1.,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
                None,
            );
            c.EndDraw(None, None)?;
            self.shadow_effect.SetInput(0, source, true);
            let shadow = g.transparent_bitmap(sw, sh, D2D1_BITMAP_OPTIONS_TARGET)?;
            c.SetTarget(&shadow);
            c.BeginDraw();
            c.Clear(Some(&D2D1_COLOR_F::default()));
            c.DrawImage(
                &self.shadow_effect.GetOutput()?,
                None,
                Some(&rect(0., 0., sw as f32, sh as f32)),
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            c.EndDraw(None, None)?;
            // Release the effect's input reference before a scratch surface is reused.
            self.shadow_effect.SetInput(0, None, true);
            self.cache.insert(
                key,
                CachedWord {
                    glyph,
                    shadow,
                    width,
                    height,
                    padding,
                    bytes,
                    last_used: frame,
                },
            );
            self.cache_bytes += bytes;
            self.peak_cache_bytes = self.peak_cache_bytes.max(self.cache_bytes);
            Ok(())
        }
    }
    pub fn draw(&mut self, g: &Graphics, r: &KineticRequest, frame: u64) -> Result<()> {
        unsafe {
            let active = kinetic::active_word(&self.words, frame);
            if let Some(index) = active {
                self.prepare_word(g, r, index, frame)?;
            }
            let c = &g.context;
            c.SetTarget(&self.output);
            c.SetTransform(&Matrix3x2::identity());
            c.BeginDraw();
            c.Clear(Some(&ink(&r.canvas.background)?));
            if let Some(index) = active {
                let word = &self.words[index];
                let cached = &self.cache[&(word.text.clone(), word.emphasis)];
                let fps = &r.canvas.fps;
                let scale = kinetic::scale(
                    &r.style,
                    fps.seconds(frame - word.start_frame),
                    fps.seconds(word.end_frame - word.start_frame),
                );
                let width = (cached.width * scale).round().max(1.);
                let height = (cached.height * scale).round().max(1.);
                let left = ((r.canvas.width as f32 - width) / 2.).floor();
                let top = ((r.canvas.height as f32 - height) / 2.).floor();
                let p = cached.padding * scale;
                c.DrawBitmap(
                    &cached.shadow,
                    Some(&rect(left - p, top - p, width + 2. * p, height + 2. * p)),
                    1.,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
                c.DrawBitmap(
                    &cached.glyph,
                    Some(&rect(left, top, width, height)),
                    1.,
                    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                    None,
                    None,
                );
            }
            c.EndDraw(None, None)?;
            Ok(())
        }
    }
    pub fn pixels(&mut self, g: &Graphics) -> Result<&[u8]> {
        g.readback(&self.output, &self.cpu, &mut self.pixels)?;
        Ok(&self.pixels)
    }
}
