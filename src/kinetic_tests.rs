use crate::{
    graphics::Graphics,
    kinetic::*,
    media,
    request::{Prepared, Request},
};
use serde_json::json;
fn sample() -> KineticRequest {
    serde_json::from_value(json!({"kind":"kinetic","version":1,"words":[{"word":"From","start":0.1,"end":0.2},{"word":"scratch","start":0.3,"end":0.7,"emphasis":true},{"word":"scratch","start":0.7,"end":1.0,"emphasis":true}]})).unwrap()
}
#[test]
fn defaults_casing_and_exclusive_hard_cuts() {
    let r = sample();
    let w = r.validate().unwrap();
    assert_eq!(r.canvas.fps.numerator, 60);
    assert_eq!(r.canvas.background, "#121212");
    assert_eq!(w[0].text, "from");
    assert_eq!(w[1].text, "SCRATCH");
    assert_eq!(active_word(&w, 5), None);
    assert_eq!(active_word(&w, 6), Some(0));
    assert_eq!(active_word(&w, 17), Some(0));
    assert_eq!(active_word(&w, 18), Some(1));
    assert_eq!(active_word(&w, 42), Some(2));
    assert_eq!(active_word(&w, 60), None);
}
#[test]
fn blank_mode_and_rational_boundaries() {
    let mut r = sample();
    r.gap_mode = GapMode::Blank;
    let w = r.validate().unwrap();
    assert_eq!(active_word(&w, 12), None);
    assert_eq!(active_word(&w, 18), Some(1));
    let fps = crate::scene::FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    assert_eq!(boundary(1., &fps), 30);
    assert_eq!(boundary(fps.seconds(30), &fps), 30);
    assert_eq!(boundary(0.301, &r.canvas.fps), 19);
}

#[test]
fn hold_mode_keeps_a_subframe_speech_word_when_its_display_interval_has_frames() {
    let mut r = sample();
    r.words[0].start = 0.001;
    r.words[0].end = 0.002;
    let words = r.validate().unwrap();
    assert_eq!(words[0].start_frame, 1);
    assert_eq!(words[0].speech_end_frame, 1);
    assert_eq!(active_word(&words, 1), Some(0));
    r.gap_mode = GapMode::Blank;
    assert!(r.validate().is_err());
}
#[test]
fn scale_milestones_hold_and_short_words() {
    let s = KineticStyle::default();
    for (time, expected) in [
        (0., 0.85),
        (0.04, 1.04),
        (0.08, 1.),
        (0.54, 1.02),
        (1., 1.04),
    ] {
        assert!((scale(&s, time, 1.) - expected).abs() < 1e-5);
    }
    assert!((scale(&s, 0.01, 0.02) - 1.04).abs() < 1e-5);
    assert!((scale(&s, 0.02, 0.02) - 1.).abs() < 1e-5);
}
#[test]
fn invalid_intervals_and_zero_frame_words_fail() {
    for bad in [
        json!({"word":"two words","start":0.,"end":0.1}),
        json!({"word":"","start":0.,"end":0.1}),
        json!({"word":"tiny","start":0.001,"end":0.002}),
        json!({"word":"backwards","start":0.2,"end":0.1}),
    ] {
        let mut r = sample();
        r.words = vec![serde_json::from_value(bad).unwrap()];
        assert!(r.validate().is_err());
    }
    let mut r = sample();
    r.words[1].start = 0.15;
    assert!(r.validate().is_err());
    let mut r = sample();
    r.style.pop_ms = f64::NAN;
    assert!(r.validate().is_err());
}
#[test]
fn partial_canvas_retains_kinetic_defaults_and_unknown_fields_fail() {
    let v = json!({"kind":"kinetic","version":1,"canvas":{"width":640,"height":360},"words":[{"word":"hello","start":0.,"end":1.}]});
    let r: KineticRequest = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(r.canvas.fps.numerator, 60);
    assert_eq!(r.canvas.background, "#121212");
    r.validate().unwrap();
    let mut bad = v;
    bad["surprise"] = json!(true);
    assert!(serde_json::from_value::<KineticRequest>(bad).is_err());
}
#[test]
fn embedded_font_has_distinct_case_and_reports_missing_glyphs() {
    let _runtime = media::Runtime::new().unwrap();
    let font = crate::fonts::PixelFont::new().unwrap();
    assert_ne!(font.glyphs("a", 0).unwrap(), font.glyphs("A", 0).unwrap());
    let error = font.glyphs("🦀", 3).unwrap_err();
    assert!(error.message.contains("words[3]"));
}
#[test]
fn pixel_track_fitting_cache_and_background() {
    let _runtime = media::Runtime::new().unwrap();
    let g = Graphics::new(true).unwrap();
    let r = sample();
    let request = Request::Kinetic(r.clone());
    let mut c = Prepared::new(&g, &request).unwrap();
    c.draw(&g, &request, 0).unwrap();
    assert!(c
        .pixels(&g)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| *p == [18, 18, 18, 255]));
    c.draw(&g, &request, 20).unwrap();
    let first = c.pixels(&g).unwrap().to_vec();
    assert!(first.as_chunks::<4>().0.contains(&[0, 204, 255, 255]));
    c.draw(&g, &request, 43).unwrap();
    assert_eq!(c.report()["kinetic"]["cache_misses"], 1);
    assert!(c.report()["kinetic"]["cache_hits"].as_u64().unwrap() >= 1);
    assert!(c.report()["kinetic"]["peak_cache_bytes"].as_u64().unwrap() <= 64 * 1024 * 1024);
}

fn foreground_bounds(pixels: &[u8], width: u32) -> (i32, i32, i32, i32) {
    let mut b = (i32::MAX, i32::MAX, -1, -1);
    for (index, p) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if *p == [0, 204, 255, 255] {
            let x = (index % width as usize) as i32;
            let y = (index / width as usize) as i32;
            b.0 = b.0.min(x);
            b.1 = b.1.min(y);
            b.2 = b.2.max(x);
            b.3 = b.3.max(y);
        }
    }
    assert!(b.2 >= b.0);
    b
}
#[test]
fn nearest_pixel_edges_centering_and_emphasis_are_exact() {
    let _runtime = media::Runtime::new().unwrap();
    let g = Graphics::new(true).unwrap();
    let r:KineticRequest=serde_json::from_value(json!({"kind":"kinetic","version":1,"style":{"pop_ms":0,"hold_scale":1,"shadow":{"opacity":0}},"words":[{"word":"123","start":0,"end":0.5},{"word":"123","start":0.5,"end":1,"emphasis":true}]})).unwrap();
    let request = Request::Kinetic(r);
    let mut c = Prepared::new(&g, &request).unwrap();
    let mut bounds = Vec::new();
    for frame in [0, 30] {
        c.draw(&g, &request, frame).unwrap();
        let pixels = c.pixels(&g).unwrap();
        assert!(pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [18, 18, 18, 255] || *p == [0, 204, 255, 255]));
        let b = foreground_bounds(pixels, 1920);
        assert!((b.0 + b.2 - 1919).abs() <= 1);
        assert!((b.1 + b.3 - 1079).abs() <= 1);
        bounds.push(b);
    }
    let width = |b: (i32, i32, i32, i32)| b.2 - b.0 + 1;
    assert!((width(bounds[1]) as f32 - width(bounds[0]) as f32 * 1.2).abs() <= 2.);
}
#[test]
fn fitting_accounts_for_emphasis_shadow_and_peak_scale() {
    let _runtime = media::Runtime::new().unwrap();
    let g = Graphics::new(true).unwrap();
    let mut r = sample();
    r.words[1].word = "DETERMINATION".into();
    let request = Request::Kinetic(r);
    let mut c = Prepared::new(&g, &request).unwrap();
    assert!(c.report()["kinetic"]["font_size"].as_f64().unwrap() < 160.);
    for frame in [18, 20, 21, 23, 41] {
        c.draw(&g, &request, frame).unwrap();
        let b = foreground_bounds(c.pixels(&g).unwrap(), 1920);
        assert!(b.0 >= 96 && b.2 < 1824 && b.1 >= 96 && b.3 < 984);
    }
}
#[test]
#[ignore = "Native 64 MiB kinetic LRU eviction stress"]
fn many_unique_words_evict_textures_and_recreate_them() {
    let _runtime = media::Runtime::new().unwrap();
    let g = Graphics::new(true).unwrap();
    let words: Vec<_> = (0..120)
        .map(|i| json!({"word":format!("w{i:08}"),"start":i as f64*0.1,"end":(i+1) as f64*0.1}))
        .collect();
    let r: KineticRequest =
        serde_json::from_value(json!({"kind":"kinetic","version":1,"words":words})).unwrap();
    let request = Request::Kinetic(r);
    let mut c = Prepared::new(&g, &request).unwrap();
    for i in 0..120 {
        c.draw(&g, &request, i * 6).unwrap();
    }
    let report = c.report();
    assert_eq!(report["kinetic"]["cache_misses"], 120);
    assert!(report["kinetic"]["cache_bytes"].as_u64().unwrap() <= 64 * 1024 * 1024);
    c.draw(&g, &request, 0).unwrap();
    assert_eq!(c.report()["kinetic"]["cache_misses"], 121);
}
