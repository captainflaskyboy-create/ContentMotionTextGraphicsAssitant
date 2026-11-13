//! Timed word requests and frame-indexed animation, independent of drawing resources.
use crate::{
    error::{Error, Result},
    scene::Easing,
    scene::{Canvas, FrameRate, Scene, Theme},
    timing::ease,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KineticRequest {
    pub kind: KineticKind,
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    #[serde(default)]
    pub canvas: KineticCanvas,
    #[serde(default)]
    pub style: KineticStyle,
    #[serde(default)]
    pub gap_mode: GapMode,
    #[schemars(length(min = 1, max = 10000))]
    pub words: Vec<Word>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KineticKind {
    Kinetic,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GapMode {
    #[default]
    Hold,
    Blank,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub emphasis: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct KineticCanvas {
    pub width: u32,
    pub height: u32,
    pub fps: FrameRate,
    pub background: String,
    pub bitrate_kbps: u32,
}
impl Default for KineticCanvas {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: FrameRate {
                numerator: 60,
                denominator: 1,
            },
            background: "#121212".into(),
            bitrate_kbps: 20000,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct KineticStyle {
    pub color: String,
    pub font_size: f32,
    pub min_font_size: f32,
    pub emphasis_scale: f32,
    pub margin: f32,
    pub pop_ms: f64,
    pub initial_scale: f32,
    pub peak_scale: f32,
    pub settle_scale: f32,
    pub hold_scale: f32,
    pub shadow: Shadow,
}
impl Default for KineticStyle {
    fn default() -> Self {
        Self {
            color: "#FFCC00".into(),
            font_size: 160.,
            min_font_size: 64.,
            emphasis_scale: 1.2,
            margin: 96.,
            pop_ms: 80.,
            initial_scale: 0.85,
            peak_scale: 1.04,
            settle_scale: 1.,
            hold_scale: 1.04,
            shadow: Shadow::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Shadow {
    pub color: String,
    pub opacity: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
}
impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "#000000".into(),
            opacity: 0.8,
            offset_x: 0.,
            offset_y: 6.,
            blur_radius: 16.,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct TimedWord {
    pub index: usize,
    pub text: String,
    pub emphasis: bool,
    pub start_frame: u64,
    pub end_frame: u64,
    pub speech_end_frame: u64,
}
pub fn boundary(seconds: f64, fps: &FrameRate) -> u64 {
    (seconds * fps.numerator as f64 / fps.denominator as f64 - 1e-9)
        .ceil()
        .max(0.) as u64
}
impl KineticRequest {
    pub fn settings(&self) -> Scene {
        Scene {
            version: 1,
            duration: self.canvas.fps.seconds(boundary(
                self.words.last().map_or(0., |w| w.end),
                &self.canvas.fps,
            )),
            canvas: Canvas {
                width: self.canvas.width,
                height: self.canvas.height,
                fps: self.canvas.fps.clone(),
                background: self.canvas.background.clone(),
                bitrate_kbps: self.canvas.bitrate_kbps,
            },
            theme: Theme::default(),
            seed: 0,
            layers: Vec::new(),
        }
    }
    pub fn validate(&self) -> Result<Vec<TimedWord>> {
        let fail = |message: &str| Error::new("invalid_scene", message);
        if self.version != 1 {
            return Err(fail("version: kinetic supports version 1"));
        }
        if self.words.is_empty() || self.words.len() > 10000 {
            return Err(fail("words: expected 1..10000 entries"));
        }
        // Reuse the established canvas limits without allocating any word drawing resources.
        let mut probe = crate::presets::make("impact", "probe", 1.)?;
        probe.canvas = self.settings().canvas;
        probe.validate()?;
        let s = &self.style;
        if [
            s.font_size,
            s.min_font_size,
            s.emphasis_scale,
            s.margin,
            s.initial_scale,
            s.peak_scale,
            s.settle_scale,
            s.hold_scale,
            s.shadow.opacity,
            s.shadow.offset_x,
            s.shadow.offset_y,
            s.shadow.blur_radius,
        ]
        .iter()
        .any(|n| !n.is_finite())
            || !(64. ..=400.).contains(&s.min_font_size)
            || s.font_size < s.min_font_size
            || s.font_size > 400.
            || !(1. ..=3.).contains(&s.emphasis_scale)
            || !(0. ..=480.).contains(&s.margin)
            || [s.initial_scale, s.peak_scale, s.settle_scale, s.hold_scale]
                .iter()
                .any(|n| !(0.1..=3.).contains(n))
            || !s.pop_ms.is_finite()
            || !(0. ..=2000.).contains(&s.pop_ms)
            || !(0. ..=1.).contains(&s.shadow.opacity)
            || !(0. ..=64.).contains(&s.shadow.blur_radius)
            || s.shadow.offset_x.abs() > 128.
            || s.shadow.offset_y.abs() > 128.
        {
            return Err(fail(
                "style: invalid size, scale, margin, pop timing or shadow",
            ));
        }
        for color in [&self.canvas.background, &s.color, &s.shadow.color] {
            if !color.starts_with('#') {
                return Err(fail("kinetic colours must use #RRGGBB"));
            }
            Theme::default().color(color)?;
        }
        let mut words = Vec::with_capacity(self.words.len());
        let mut previous_end = 0.;
        for (index, w) in self.words.iter().enumerate() {
            if w.word.is_empty()
                || w.word.len() > 1024
                || w.word.chars().any(char::is_whitespace)
                || !w.start.is_finite()
                || !w.end.is_finite()
                || w.start < 0.
                || w.end <= w.start
                || w.end > 600.
                || w.start < previous_end
            {
                return Err(Error::new("invalid_scene",format!("words[{index}]: expected a nonempty single word and ordered non-overlapping 0..600 second intervals")));
            }
            previous_end = w.end;
            let start_frame = boundary(w.start, &self.canvas.fps);
            let speech_end_frame = boundary(w.end, &self.canvas.fps);
            let end = if matches!(self.gap_mode, GapMode::Hold) {
                self.words.get(index + 1).map_or(w.end, |next| next.start)
            } else {
                w.end
            };
            let end_frame = boundary(end, &self.canvas.fps);
            if end_frame <= start_frame {
                return Err(Error::new("invalid_scene",format!("words[{index}]: interval receives no display frames at the selected frame rate")));
            }
            words.push(TimedWord {
                index,
                text: if w.emphasis {
                    w.word.to_uppercase()
                } else {
                    w.word.to_lowercase()
                },
                emphasis: w.emphasis,
                start_frame,
                end_frame,
                speech_end_frame,
            });
        }
        Ok(words)
    }
}
pub fn active_word(words: &[TimedWord], frame: u64) -> Option<usize> {
    let i = words
        .partition_point(|w| w.start_frame <= frame)
        .checked_sub(1)?;
    (frame < words[i].end_frame).then_some(i)
}
pub fn scale(style: &KineticStyle, local: f64, duration: f64) -> f32 {
    let pop = (style.pop_ms / 1000.).min(duration);
    let half = pop / 2.;
    if pop > 0. && local < half {
        style.initial_scale
            + (style.peak_scale - style.initial_scale)
                * ease((local / half) as f32, &Easing::OutCubic)
    } else if pop > 0. && local < pop {
        style.peak_scale
            + (style.settle_scale - style.peak_scale)
                * ease(((local - half) / half) as f32, &Easing::InOutCubic)
    } else {
        let t = if duration > pop {
            ((local - pop) / (duration - pop)).clamp(0., 1.) as f32
        } else {
            0.
        };
        style.settle_scale + (style.hold_scale - style.settle_scale) * t
    }
}
