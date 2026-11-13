//! Common presentation boundary: old layer scenes and new timed-word tracks share export.
use crate::{
    compositor::Compositor, error::Result, graphics::Graphics, kinetic::KineticRequest,
    pixel_text::PixelCompositor, scene::Scene,
};
use serde_json::{json, Value};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
#[derive(Clone)]
pub enum Request {
    Scene(Scene),
    Kinetic(KineticRequest),
}
impl Request {
    pub fn settings(&self) -> Scene {
        match self {
            Self::Scene(s) => s.clone(),
            Self::Kinetic(k) => k.settings(),
        }
    }
    pub fn validate(&self) -> Result<Vec<String>> {
        match self {
            Self::Scene(s) => s.validate(),
            Self::Kinetic(k) => {
                k.validate()?;
                Ok(Vec::new())
            }
        }
    }
}
pub enum Prepared {
    Scene(Box<Compositor>),
    Kinetic(Box<PixelCompositor>),
}
impl Prepared {
    pub fn new(g: &Graphics, request: &Request) -> Result<Self> {
        match request {
            Request::Scene(s) => Ok(Self::Scene(Box::new(Compositor::new(g, s)?))),
            Request::Kinetic(k) => Ok(Self::Kinetic(Box::new(PixelCompositor::new(g, k)?))),
        }
    }
    pub fn draw(&mut self, g: &Graphics, request: &Request, frame: u64) -> Result<()> {
        match (self, request) {
            (Self::Scene(c), Request::Scene(s)) => c.draw(g, s, frame),
            (Self::Kinetic(c), Request::Kinetic(k)) => c.draw(g, k, frame),
            _ => unreachable!("prepared presentation and request must match"),
        }
    }
    pub fn output(&self) -> &ID2D1Bitmap1 {
        match self {
            Self::Scene(c) => &c.output,
            Self::Kinetic(c) => &c.output,
        }
    }
    pub fn pixels(&mut self, g: &Graphics) -> Result<&[u8]> {
        match self {
            Self::Scene(c) => c.pixels(g),
            Self::Kinetic(c) => c.pixels(g),
        }
    }
    pub fn warnings(&self) -> Vec<String> {
        match self {
            Self::Scene(c) => c.warnings.clone(),
            Self::Kinetic(_) => Vec::new(),
        }
    }
    pub fn report(&self) -> Value {
        match self {
            Self::Scene(c) => json!({"layouts":c.layouts}),
            Self::Kinetic(c) => json!({"kinetic":c.report()}),
        }
    }
}
