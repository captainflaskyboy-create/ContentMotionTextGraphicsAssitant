use crate::scene::{Easing, Layer};
use unicode_segmentation::UnicodeSegmentation;
pub fn ease(t: f32, kind: &Easing) -> f32 {
    let t = t.clamp(0., 1.);
    match kind {
        Easing::Linear => t,
        Easing::OutCubic => 1. - (1. - t).powi(3),
        Easing::InOutCubic => {
            if t < 0.5 {
                4. * t * t * t
            } else {
                1. - (-2. * t + 2.).powi(3) / 2.
            }
        }
        Easing::OutBack => 1. + 2.70158 * (t - 1.).powi(3) + 1.70158 * (t - 1.).powi(2),
    }
}
pub struct State {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub reveal: f32,
}
pub fn state(layer: &Layer, time: f64, seed: u64, frame: u64) -> Option<State> {
    let local = time - layer.start;
    if local < 0. || local >= layer.duration {
        return None;
    }
    let m = &layer.motion;
    let enter = if m.entrance == 0. {
        1.
    } else {
        ease((local / m.entrance) as f32, &m.easing)
    };
    let exit = if m.exit == 0. {
        0.
    } else {
        ((local - layer.duration + m.exit) / m.exit).clamp(0., 1.) as f32
    };
    let jitter = if m.glitch > 0. && local < m.entrance {
        noise(seed, frame) * m.glitch
    } else {
        0.
    };
    Some(State {
        x: layer.x + m.from_x * (1. - enter) + m.to_x * exit + jitter,
        y: layer.y + m.from_y * (1. - enter) + m.to_y * exit,
        scale: m.from_scale + (1. - m.from_scale) * enter,
        opacity: m.opacity * enter.clamp(0., 1.) * (1. - exit),
        reveal: if m.reveal { enter.clamp(0., 1.) } else { 1. },
    })
}
pub fn noise(seed: u64, frame: u64) -> f32 {
    let mut n = seed.wrapping_add(frame.wrapping_mul(0x9E3779B97F4A7C15));
    n = (n ^ (n >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94D049BB133111EB);
    ((n ^ (n >> 31)) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
}
pub fn typed(text: &str, progress: f64) -> String {
    let count = text.graphemes(true).count();
    text.graphemes(true)
        .take((count as f64 * progress.clamp(0., 1.)).floor() as usize)
        .collect()
}
