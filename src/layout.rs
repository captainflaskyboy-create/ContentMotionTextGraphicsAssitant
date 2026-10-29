//! DirectWrite is the authority for fitting, including font fallback and wrapping.
use crate::{
    error::{Error, Result},
    scene::Alignment,
};
use windows::{core::HSTRING, Win32::Graphics::DirectWrite::*};
pub struct Fitted {
    pub layout: IDWriteTextLayout,
    pub size: f32,
}
#[allow(clippy::too_many_arguments)]
pub fn fit(
    factory: &IDWriteFactory,
    text: &str,
    font: &str,
    max: f32,
    min: f32,
    width: f32,
    height: f32,
    wrap: bool,
    align: &Alignment,
) -> Result<Fitted> {
    unsafe {
        let text: Vec<u16> = text.encode_utf16().collect();
        let create = |size: f32| -> Result<(IDWriteTextLayout, bool)> {
            let format = factory.CreateTextFormat(
                &HSTRING::from(font),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                &HSTRING::from("en-US"),
            )?;
            format.SetWordWrapping(if wrap {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            })?;
            format.SetTextAlignment(match align {
                Alignment::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Alignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Alignment::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            let layout = factory.CreateTextLayout(&text, &format, width, height)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            let fits = metrics.widthIncludingTrailingWhitespace <= width + 0.1
                && metrics.height <= height + 0.1;
            Ok((layout, fits))
        };
        let (layout, fits) = create(max)?;
        if fits {
            return Ok(Fitted { layout, size: max });
        }
        let (mut best, fits) = create(min)?;
        if !fits {
            return Err(Error::new(
                "invalid_scene",
                format!(
                    "Text cannot fit {width:.0}x{height:.0} at minimum font size {min}: {}",
                    String::from_utf16_lossy(&text)
                        .chars()
                        .take(60)
                        .collect::<String>()
                ),
            ));
        }
        let (mut low, mut high) = (min, max);
        for _ in 0..12 {
            let size = (low + high) / 2.;
            let (layout, fits) = create(size)?;
            if fits {
                low = size;
                best = layout;
            } else {
                high = size;
            }
        }
        Ok(Fitted {
            layout: best,
            size: low,
        })
    }
}
pub fn font(
    factory: &IDWriteFactory,
    requested: &str,
    warnings: &mut Vec<String>,
) -> Result<String> {
    unsafe {
        let mut collection = None;
        factory.GetSystemFontCollection(&mut collection, false)?;
        let collection = collection
            .ok_or_else(|| Error::new("native_unavailable", "No Windows font collection"))?;
        for candidate in [requested, "Segoe UI", "Arial"] {
            let (mut index, mut exists) = (0, windows::core::BOOL(0));
            collection.FindFamilyName(&HSTRING::from(candidate), &mut index, &mut exists)?;
            if exists.as_bool() {
                if candidate != requested {
                    warnings.push(format!("Missing font {requested}; using {candidate}"));
                }
                return Ok(candidate.into());
            }
        }
        Err(Error::new(
            "native_unavailable",
            "No suitable installed font; install Segoe UI or Arial",
        ))
    }
}
