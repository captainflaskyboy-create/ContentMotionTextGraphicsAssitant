use crate::{
    error::{Error, Result},
    graphics::Graphics,
    layout,
    scene::*,
    timing,
};
use windows::{
    core::Interface,
    Win32::{
        Foundation::GENERIC_READ,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Imaging::*,
        },
        System::Com::*,
    },
};
use windows_numerics::{Matrix3x2, Vector2};

fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
fn color(theme: &Theme, name: &str) -> Result<D2D1_COLOR_F> {
    let c = theme.color(name)?;
    Ok(D2D1_COLOR_F {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    })
}
struct Prepared {
    brush: ID2D1SolidColorBrush,
    secondary: ID2D1SolidColorBrush,
    text: Option<IDWriteTextLayout>,
    image: Option<ID2D1Bitmap1>,
    font: String,
    size: f32,
    last_counter: String,
    utf16_len: u32,
}
pub struct Compositor {
    pub output: ID2D1Bitmap1,
    cpu: ID2D1Bitmap1,
    pixels: Vec<u8>,
    prepared: Vec<Prepared>,
    factory: IDWriteFactory,
    outline: ID2D1SolidColorBrush,
    hidden: ID2D1SolidColorBrush,
    pub warnings: Vec<String>,
    pub layouts: Vec<serde_json::Value>,
}
impl Compositor {
    pub fn new(g: &Graphics, s: &Scene) -> Result<Self> {
        unsafe {
            let output = g.bitmap(s.canvas.width, s.canvas.height, D2D1_BITMAP_OPTIONS_TARGET)?;
            let cpu = g.bitmap(
                s.canvas.width,
                s.canvas.height,
                D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            )?;
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let outline = g
                .context
                .CreateSolidColorBrush(&color(&s.theme, "outline")?, None)?;
            let hidden = g.context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.,
                },
                None,
            )?;
            let mut prepared = Vec::new();
            let mut warnings = Vec::new();
            let mut layouts = Vec::new();
            let mut total_image_pixels = 0u64;
            for l in &s.layers {
                let mut p = Prepared {
                    brush: g
                        .context
                        .CreateSolidColorBrush(&color(&s.theme, "white")?, None)?,
                    secondary: g
                        .context
                        .CreateSolidColorBrush(&color(&s.theme, "panel")?, None)?,
                    text: None,
                    image: None,
                    font: String::new(),
                    size: 0.,
                    last_counter: String::new(),
                    utf16_len: 0,
                };
                match &l.content {
                    Content::Text {
                        text,
                        font,
                        font_size,
                        min_font_size,
                        color: ink,
                        align,
                        padding,
                        wrap,
                        scroll_lines,
                        ..
                    } => {
                        let requested = match font.as_str() {
                            "heading" => &s.theme.heading_font,
                            "body" => &s.theme.body_font,
                            "mono" => &s.theme.mono_font,
                            _ => font,
                        };
                        p.font = layout::font(&factory, requested, &mut warnings)?;
                        let available_height =
                            l.height - 2. * padding + *scroll_lines as f32 * font_size * 1.5;
                        let fitted = layout::fit(
                            &factory,
                            text,
                            &p.font,
                            *font_size,
                            *min_font_size,
                            l.width - 2. * padding,
                            available_height,
                            *wrap,
                            align,
                        )
                        .map_err(|e| {
                            Error::new(e.code, format!("layer {}: {}", l.id, e.message))
                        })?;
                        p.size = fitted.size;
                        p.text = Some(fitted.layout);
                        p.utf16_len = text.encode_utf16().count() as u32;
                        p.brush.SetColor(&color(&s.theme, ink)?);
                        layouts.push(serde_json::json!({"layer":l.id,"font":p.font,"font_size":p.size,"bounds":[l.x,l.y,l.width,l.height]}));
                    }
                    Content::Rectangle {
                        color: ink, stroke, ..
                    } => {
                        p.brush.SetColor(&color(&s.theme, ink)?);
                        p.secondary
                            .SetColor(&color(&s.theme, stroke.as_deref().unwrap_or("outline"))?);
                    }
                    Content::Line { color: ink, .. } => {
                        p.brush.SetColor(&color(&s.theme, ink)?);
                    }
                    Content::Counter {
                        from,
                        to,
                        decimals,
                        suffix,
                        font_size,
                        color: ink,
                        ..
                    } => {
                        p.brush.SetColor(&color(&s.theme, ink)?);
                        p.font = layout::font(&factory, &s.theme.heading_font, &mut warnings)?;
                        // Preflight endpoints and a wide-digit envelope before starting encoding.
                        let a = format!("{:.*}{}", *decimals as usize, from, suffix);
                        let b = format!("{:.*}{}", *decimals as usize, to, suffix);
                        let envelope =
                            format!("{}{}", "8".repeat(a.len().max(b.len()).min(64)), suffix);
                        p.size = *font_size;
                        for value in [&a, &b, &envelope] {
                            p.size = layout::fit(
                                &factory,
                                value,
                                &p.font,
                                p.size,
                                8.,
                                l.width,
                                l.height,
                                false,
                                &Alignment::Left,
                            )?
                            .size;
                        }
                    }
                    Content::Progress {
                        color: ink, track, ..
                    } => {
                        p.brush.SetColor(&color(&s.theme, ink)?);
                        p.secondary.SetColor(&color(&s.theme, track)?);
                    }
                    Content::Image { path } => {
                        let wic: IWICImagingFactory =
                            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                        let decoder = wic.CreateDecoderFromFilename(
                            &crate::media::native_path(path),
                            None,
                            GENERIC_READ,
                            WICDecodeMetadataCacheOnLoad,
                        )?;
                        let frame = decoder.GetFrame(0)?;
                        let (mut width, mut height) = (0, 0);
                        frame.GetSize(&mut width, &mut height)?;
                        total_image_pixels += width as u64 * height as u64;
                        if width == 0 || height == 0 || total_image_pixels > 32_000_000 {
                            return Err(Error::new(
                                "invalid_scene",
                                "Decoded images exceed 32 megapixel scene budget",
                            ));
                        }
                        let converter = wic.CreateFormatConverter()?;
                        converter.Initialize(
                            &frame,
                            &GUID_WICPixelFormat32bppPBGRA,
                            WICBitmapDitherTypeNone,
                            None,
                            0.,
                            WICBitmapPaletteTypeCustom,
                        )?;
                        p.image = Some(g.context.CreateBitmapFromWicBitmap(&converter, None)?);
                    }
                }
                prepared.push(p);
            }
            warnings.sort();
            warnings.dedup();
            Ok(Self {
                output,
                cpu,
                pixels: Vec::new(),
                prepared,
                factory,
                outline,
                hidden,
                warnings,
                layouts,
            })
        }
    }
    pub fn draw(&mut self, g: &Graphics, s: &Scene, frame: u64) -> Result<()> {
        unsafe {
            let c = &g.context;
            let time = s.canvas.fps.seconds(frame);
            c.SetTarget(&self.output);
            c.SetTransform(&Matrix3x2::identity());
            c.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            c.BeginDraw();
            c.Clear(Some(&color(&s.theme, &s.canvas.background)?));
            // Always balance BeginDraw/EndDraw, including a failed dynamic layout.
            let drawing = (|| -> Result<()> {
                for (l, p) in s.layers.iter().zip(&mut self.prepared) {
                    let Some(state) = timing::state(l, time, s.seed, frame) else {
                        continue;
                    };
                    let sx = s.canvas.width as f32 / 1920.;
                    let sy = s.canvas.height as f32 / 1080.;
                    let scale = state.scale;
                    c.SetTransform(&Matrix3x2 {
                        M11: sx * scale,
                        M12: 0.,
                        M21: 0.,
                        M22: sy * scale,
                        M31: (state.x + l.width * (1. - scale) / 2.) * sx,
                        M32: (state.y + l.height * (1. - scale) / 2.) * sy,
                    });
                    p.brush.SetOpacity(state.opacity);
                    p.secondary.SetOpacity(state.opacity);
                    self.outline.SetOpacity(state.opacity);
                    c.PushAxisAlignedClip(
                        &rect(0., 0., l.width * state.reveal, l.height),
                        D2D1_ANTIALIAS_MODE_ALIASED,
                    );
                    match &l.content {
                        Content::Rectangle {
                            radius,
                            stroke_width,
                            ..
                        } => {
                            let r = D2D1_ROUNDED_RECT {
                                rect: rect(0., 0., l.width, l.height),
                                radiusX: *radius,
                                radiusY: *radius,
                            };
                            c.FillRoundedRectangle(&r, &p.brush);
                            if *stroke_width > 0. {
                                c.DrawRoundedRectangle(&r, &p.secondary, *stroke_width, None);
                            }
                        }
                        Content::Line { thickness, .. } => {
                            c.DrawLine(
                                Vector2 { X: 0., Y: 0. },
                                Vector2 {
                                    X: if l.width <= 4. { 0. } else { l.width },
                                    Y: if l.height <= 4. { 0. } else { l.height },
                                },
                                &p.brush,
                                *thickness,
                                None,
                            );
                        }
                        Content::Image { .. } => {
                            c.DrawBitmap(
                                p.image.as_ref().unwrap(),
                                Some(&rect(0., 0., l.width, l.height)),
                                state.opacity,
                                D2D1_INTERPOLATION_MODE_LINEAR,
                                None,
                                None,
                            );
                        }
                        Content::Text {
                            text,
                            padding,
                            typing_seconds,
                            cursor,
                            scroll_lines,
                            ..
                        } => {
                            let layout = p.text.as_ref().unwrap();
                            let local = time - l.start;
                            let visible = if *typing_seconds > 0. {
                                timing::typed(text, local / typing_seconds)
                                    .encode_utf16()
                                    .count() as u32
                            } else {
                                p.utf16_len
                            };
                            layout.SetDrawingEffect(
                                None,
                                DWRITE_TEXT_RANGE {
                                    startPosition: 0,
                                    length: p.utf16_len,
                                },
                            )?;
                            if visible < p.utf16_len {
                                layout.SetDrawingEffect(
                                    &self.hidden.cast::<windows::core::IUnknown>()?,
                                    DWRITE_TEXT_RANGE {
                                        startPosition: visible,
                                        length: p.utf16_len - visible,
                                    },
                                )?;
                            }
                            let mut text_metrics = DWRITE_TEXT_METRICS::default();
                            layout.GetMetrics(&mut text_metrics)?;
                            let overflow =
                                (text_metrics.height - (l.height - 2. * padding)).max(0.);
                            let scroll = if *scroll_lines > 0 && *typing_seconds > 0. {
                                (local / typing_seconds).clamp(0., 1.) as f32
                                    * overflow.min(*scroll_lines as f32 * p.size * 1.25)
                            } else {
                                0.
                            };
                            let origin = Vector2 {
                                X: *padding,
                                Y: *padding - scroll,
                            };
                            for (dx, dy) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)] {
                                c.DrawTextLayout(
                                    Vector2 {
                                        X: origin.X + dx * s.theme.outline_width,
                                        Y: origin.Y + dy * s.theme.outline_width,
                                    },
                                    layout,
                                    &self.outline,
                                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                                );
                            }
                            c.DrawTextLayout(origin, layout, &p.brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
                            if *cursor && ((local * 2.).floor() as u64).is_multiple_of(2) {
                                let (mut x, mut y) = (0., 0.);
                                let mut metrics = DWRITE_HIT_TEST_METRICS::default();
                                layout.HitTestTextPosition(
                                    visible,
                                    false,
                                    &mut x,
                                    &mut y,
                                    &mut metrics,
                                )?;
                                c.FillRectangle(
                                    &rect(origin.X + x, origin.Y + y, 3., p.size),
                                    &p.brush,
                                );
                            }
                        }
                        Content::Counter {
                            from,
                            to,
                            decimals,
                            suffix,
                            font_size,
                            count_seconds,
                            ..
                        } => {
                            let amount = timing::ease(
                                ((time - l.start) / count_seconds) as f32,
                                &Easing::OutCubic,
                            ) as f64;
                            let value = from + (to - from) * amount;
                            let label = format!("{:.*}{}", *decimals as usize, value, suffix);
                            if label != p.last_counter {
                                let fitted = layout::fit(
                                    &self.factory,
                                    &label,
                                    &p.font,
                                    p.size.min(*font_size),
                                    8.,
                                    l.width,
                                    l.height,
                                    false,
                                    &Alignment::Left,
                                )?;
                                p.text = Some(fitted.layout);
                                p.last_counter = label;
                            }
                            c.DrawTextLayout(
                                Vector2::default(),
                                p.text.as_ref().unwrap(),
                                &p.brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                        Content::Progress {
                            from,
                            to,
                            fill_seconds,
                            easing,
                            ..
                        } => {
                            c.FillRectangle(&rect(0., 0., l.width, l.height), &p.secondary);
                            let fraction = from
                                + (to - from)
                                    * timing::ease(
                                        ((time - l.start) / fill_seconds) as f32,
                                        easing,
                                    );
                            c.FillRectangle(&rect(0., 0., l.width * fraction, l.height), &p.brush);
                        }
                    }
                    c.PopAxisAlignedClip();
                }
                Ok(())
            })();
            let end = c.EndDraw(None, None);
            drawing?;
            end?;
            Ok(())
        }
    }
    pub fn pixels(&mut self, g: &Graphics) -> Result<&[u8]> {
        g.readback(&self.output, &self.cpu, &mut self.pixels)?;
        Ok(&self.pixels)
    }
}
