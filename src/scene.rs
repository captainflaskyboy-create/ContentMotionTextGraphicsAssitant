//! Versioned, strict scene contract. All coordinates use a 1920 x 1080 design canvas.
use crate::error::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    #[schemars(range(max = 600))]
    pub duration: f64,
    #[serde(default)]
    pub canvas: Canvas,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub seed: u64,
    #[schemars(length(min = 1, max = 512))]
    pub layers: Vec<Layer>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Canvas {
    #[schemars(range(min = 64, max = 3840))]
    pub width: u32,
    #[schemars(range(min = 64, max = 2160))]
    pub height: u32,
    pub fps: FrameRate,
    pub background: String,
    #[schemars(range(min = 1000, max = 150000))]
    pub bitrate_kbps: u32,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: FrameRate::default(),
            background: "#00FF00".into(),
            bitrate_kbps: 20000,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct FrameRate {
    #[schemars(range(min = 1, max = 120000))]
    pub numerator: u32,
    #[schemars(range(min = 1, max = 10000))]
    pub denominator: u32,
}
impl Default for FrameRate {
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
    }
}
impl FrameRate {
    pub fn seconds(&self, frame: u64) -> f64 {
        frame as f64 * self.denominator as f64 / self.numerator as f64
    }
    pub fn ticks(&self, frame: u64) -> i64 {
        (frame as u128 * self.denominator as u128 * 10_000_000 / self.numerator as u128) as i64
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub heading_font: String,
    pub body_font: String,
    pub mono_font: String,
    pub yellow: String,
    pub white: String,
    pub panel: String,
    pub muted: String,
    pub outline: String,
    pub outline_width: f32,
    pub entrance: f64,
    pub exit: f64,
}
impl Default for Theme {
    fn default() -> Self {
        Self {
            heading_font: "Arial Black".into(),
            body_font: "Segoe UI".into(),
            mono_font: "Consolas".into(),
            yellow: "#FFDB3D".into(),
            white: "#F8F8F2".into(),
            panel: "#15151D".into(),
            muted: "#9B9BA8".into(),
            outline: "#09090D".into(),
            outline_width: 2.,
            entrance: 0.22,
            exit: 0.15,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: String,
    pub start: f64,
    pub duration: f64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub motion: Motion,
    pub content: Content,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    #[default]
    OutCubic,
    InOutCubic,
    OutBack,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    pub entrance: f64,
    pub exit: f64,
    pub easing: Easing,
    pub from_x: f32,
    pub from_y: f32,
    pub from_scale: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub opacity: f32,
    pub reveal: bool,
    pub glitch: f32,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            entrance: 0.22,
            exit: 0.15,
            easing: Easing::default(),
            from_x: 0.,
            from_y: 28.,
            from_scale: 1.,
            to_x: 0.,
            to_y: -12.,
            opacity: 1.,
            reveal: false,
            glitch: 0.,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Content {
    Text {
        text: String,
        #[serde(default = "heading")]
        font: String,
        #[serde(default = "font_size")]
        font_size: f32,
        #[serde(default = "min_font")]
        min_font_size: f32,
        #[serde(default = "white")]
        color: String,
        #[serde(default)]
        align: Alignment,
        #[serde(default)]
        padding: f32,
        #[serde(default = "yes")]
        wrap: bool,
        #[serde(default)]
        typing_seconds: f64,
        #[serde(default)]
        cursor: bool,
        #[serde(default)]
        scroll_lines: u32,
    },
    Rectangle {
        color: String,
        #[serde(default)]
        radius: f32,
        #[serde(default)]
        stroke_width: f32,
        #[serde(default)]
        stroke: Option<String>,
    },
    Line {
        color: String,
        #[serde(default = "line_width")]
        thickness: f32,
    },
    Image {
        path: PathBuf,
    },
    Counter {
        from: f64,
        to: f64,
        #[serde(default)]
        decimals: u32,
        #[serde(default)]
        suffix: String,
        #[serde(default = "font_size")]
        font_size: f32,
        #[serde(default = "yellow")]
        color: String,
        #[serde(default = "one")]
        count_seconds: f64,
    },
    Progress {
        #[serde(default)]
        from: f32,
        #[serde(default = "one_f32")]
        to: f32,
        #[serde(default = "yellow")]
        color: String,
        #[serde(default = "panel")]
        track: String,
        #[serde(default = "one")]
        fill_seconds: f64,
        #[serde(default = "linear")]
        easing: Easing,
    },
}
fn linear() -> Easing {
    Easing::Linear
}
fn heading() -> String {
    "heading".into()
}
fn font_size() -> f32 {
    80.
}
fn min_font() -> f32 {
    24.
}
fn white() -> String {
    "white".into()
}
fn yellow() -> String {
    "yellow".into()
}
fn panel() -> String {
    "panel".into()
}
fn yes() -> bool {
    true
}
fn line_width() -> f32 {
    4.
}
fn one() -> f64 {
    1.
}
fn one_f32() -> f32 {
    1.
}

impl Theme {
    pub fn color(&self, value: &str) -> Result<[f32; 4]> {
        let hex = match value {
            "yellow" => &self.yellow,
            "white" => &self.white,
            "panel" => &self.panel,
            "muted" => &self.muted,
            "outline" => &self.outline,
            _ => value,
        };
        if hex.len() != 7 || !hex.starts_with('#') || !hex.is_ascii() {
            return Err(Error::new(
                "invalid_scene",
                format!("Invalid colour {value}; use #RRGGBB or a theme colour"),
            ));
        }
        let mut rgba = [0., 0., 0., 1.];
        for (i, c) in rgba.iter_mut().take(3).enumerate() {
            *c = u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16)
                .map_err(|_| Error::new("invalid_scene", format!("Invalid colour {value}")))?
                as f32
                / 255.;
        }
        Ok(rgba)
    }
}
impl Scene {
    pub fn frames(&self) -> u64 {
        (self.duration * self.canvas.fps.numerator as f64 / self.canvas.fps.denominator as f64
            - 1e-8)
            .ceil() as u64
    }
    pub fn validate(&self) -> Result<Vec<String>> {
        let fail = |m: String| Error::new("invalid_scene", m);
        if self.version != 1 {
            return Err(fail("version: supported version is 1".into()));
        }
        if !self.duration.is_finite() || self.duration <= 0. || self.duration > 600. {
            return Err(fail("duration: expected 0 < seconds <= 600".into()));
        }
        let c = &self.canvas;
        if c.width < 64
            || c.height < 64
            || c.width > 3840
            || c.height > 2160
            || !c.width.is_multiple_of(2)
            || !c.height.is_multiple_of(2)
        {
            return Err(fail(
                "canvas: even dimensions required, width 64..3840, height 64..2160".into(),
            ));
        }
        let fps = c.fps.numerator as f64 / c.fps.denominator as f64;
        if c.fps.numerator == 0
            || c.fps.denominator == 0
            || c.fps.numerator > 120_000
            || c.fps.denominator > 10_000
            || !(1. ..=120.).contains(&fps)
        {
            return Err(fail(
                "canvas.fps: expected a rational rate between 1 and 120 fps".into(),
            ));
        }
        if !(1000..=150000).contains(&c.bitrate_kbps) {
            return Err(fail("canvas.bitrate_kbps: expected 1000..150000".into()));
        }
        if self.layers.is_empty() || self.layers.len() > 512 {
            return Err(fail("layers: expected 1..512 layers".into()));
        }
        if !self.theme.outline_width.is_finite()
            || !(0. ..=16.).contains(&self.theme.outline_width)
            || !self.theme.entrance.is_finite()
            || self.theme.entrance < 0.
            || !self.theme.exit.is_finite()
            || self.theme.exit < 0.
        {
            return Err(fail("theme: invalid outline width or timing".into()));
        }
        let mut warnings = Vec::new();
        let mut ids = HashSet::new();
        self.theme.color(&c.background)?;
        for color in [
            &self.theme.yellow,
            &self.theme.white,
            &self.theme.panel,
            &self.theme.muted,
            &self.theme.outline,
        ] {
            self.theme.color(color)?;
        }
        for (i, l) in self.layers.iter().enumerate() {
            let field = |s: &str| fail(format!("layers[{i}].{s}"));
            if l.id.is_empty() || !ids.insert(&l.id) {
                return Err(field("id: must be unique and nonempty"));
            }
            if !l.start.is_finite()
                || !l.duration.is_finite()
                || l.start < 0.
                || l.duration <= 0.
                || l.start + l.duration > self.duration + 1e-6
            {
                return Err(field("timing: must lie within scene duration"));
            }
            if [l.x, l.y, l.width, l.height].iter().any(|v| !v.is_finite())
                || l.width <= 0.
                || l.height <= 0.
                || l.x < 0.
                || l.y < 0.
                || l.x + l.width > 1920.01
                || l.y + l.height > 1080.01
            {
                return Err(field("bounds: must fit 1920x1080 logical canvas"));
            }
            let m = &l.motion;
            if !m.entrance.is_finite()
                || !m.exit.is_finite()
                || m.entrance < 0.
                || m.exit < 0.
                || m.entrance + m.exit > l.duration
                || [
                    m.from_x,
                    m.from_y,
                    m.from_scale,
                    m.to_x,
                    m.to_y,
                    m.opacity,
                    m.glitch,
                ]
                .iter()
                .any(|v| !v.is_finite())
                || !(0. ..=1.).contains(&m.opacity)
                || !(0.1..=4.).contains(&m.from_scale)
                || !(0. ..=32.).contains(&m.glitch)
            {
                return Err(field(
                    "motion: invalid values or entrance + exit exceeds duration",
                ));
            }
            let mut colors = Vec::new();
            match &l.content {
                Content::Text {
                    text,
                    font_size,
                    min_font_size,
                    padding,
                    color,
                    typing_seconds,
                    scroll_lines,
                    ..
                } => {
                    if text.is_empty()
                        || text.len() > 16_384
                        || !font_size.is_finite()
                        || !min_font_size.is_finite()
                        || *min_font_size < 8.
                        || *font_size < *min_font_size
                        || *font_size > 400.
                        || !padding.is_finite()
                        || *padding < 0.
                        || *padding * 2. >= l.width.min(l.height)
                        || !typing_seconds.is_finite()
                        || *typing_seconds < 0.
                        || *typing_seconds > l.duration
                        || *scroll_lines > 1000
                    {
                        return Err(field(
                            "content: invalid text size, fitting, padding or typing duration",
                        ));
                    }
                    colors.push(color);
                }
                Content::Rectangle {
                    color,
                    radius,
                    stroke_width,
                    stroke,
                } => {
                    if !radius.is_finite()
                        || *radius < 0.
                        || *radius > l.width.min(l.height) / 2.
                        || !stroke_width.is_finite()
                        || !(0. ..=32.).contains(stroke_width)
                    {
                        return Err(field("content: invalid radius or stroke"));
                    }
                    colors.push(color);
                    if let Some(s) = stroke {
                        colors.push(s);
                    }
                }
                Content::Line { color, thickness } => {
                    if !thickness.is_finite() || !(0.1..=32.).contains(thickness) {
                        return Err(field("content.thickness: expected 0.1..32"));
                    }
                    colors.push(color);
                }
                Content::Image { path } => {
                    if !path.is_file() {
                        return Err(field("content.path: local image file does not exist"));
                    }
                    if std::fs::metadata(path)?.len() > 32 * 1024 * 1024 {
                        return Err(field("content.path: image exceeds 32 MiB"));
                    }
                }
                Content::Counter {
                    from,
                    to,
                    decimals,
                    font_size,
                    color,
                    count_seconds,
                    ..
                } => {
                    if !from.is_finite()
                        || !to.is_finite()
                        || from.abs() > 1e12
                        || to.abs() > 1e12
                        || *decimals > 3
                        || !font_size.is_finite()
                        || !(8. ..=400.).contains(font_size)
                        || !count_seconds.is_finite()
                        || *count_seconds <= 0.
                        || *count_seconds > l.duration
                    {
                        return Err(field("content: invalid counter range, precision or timing"));
                    }
                    colors.push(color);
                }
                Content::Progress {
                    from,
                    to,
                    color,
                    track,
                    fill_seconds,
                    ..
                } => {
                    if !from.is_finite()
                        || !to.is_finite()
                        || !(0. ..=1.).contains(from)
                        || !(0. ..=1.).contains(to)
                        || !fill_seconds.is_finite()
                        || *fill_seconds <= 0.
                        || *fill_seconds > l.duration
                    {
                        return Err(field("content: invalid progress range or timing"));
                    }
                    colors.extend([color, track]);
                }
            }
            for color in colors {
                let rgba = self.theme.color(color)?;
                if rgba[1] > 0.5 && rgba[1] > rgba[0] * 1.3 && rgba[1] > rgba[2] * 1.3 {
                    warnings.push(format!(
                        "layers[{i}]: green foreground may disappear during chroma keying"
                    ));
                }
            }
        }
        Ok(warnings)
    }
}
