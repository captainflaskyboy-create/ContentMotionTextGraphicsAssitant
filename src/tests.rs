use super::*;
use crate::{scene::*, timing::*};
#[test]
fn demo_is_480_frames_and_every_layer_fits() {
    let s = presets::rules();
    assert!(s.validate().unwrap().is_empty());
    assert_eq!(s.frames(), 480);
    assert_eq!(s.canvas.fps.ticks(480), 160_000_000);
}
#[test]
fn rational_clock_has_no_accumulated_drift() {
    let fps = FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    assert_eq!(fps.ticks(30000), 10_010_000_000);
    for frame in 0..1000 {
        assert!(fps.ticks(frame + 1) > fps.ticks(frame));
    }
}
#[test]
fn strict_requests_reject_unknown_fields_and_versions() {
    let mut v = serde_json::to_value(presets::rules()).unwrap();
    v["surprise"] = json!(true);
    assert!(serde_json::from_value::<Scene>(v).is_err());
    let mut s = presets::rules();
    s.version = 2;
    assert!(s.validate().unwrap_err().message.contains("version"));
}
#[test]
fn invalid_timing_bounds_and_colours_are_rejected() {
    let mut s = presets::rules();
    s.layers[0].duration = 17.;
    assert!(s.validate().is_err());
    let mut s = presets::rules();
    s.layers[0].width = 2000.;
    assert!(s.validate().is_err());
    let mut s = presets::rules();
    s.theme.yellow = "#ZZZZZZ".into();
    assert!(s.validate().is_err());
}
#[test]
fn invalid_animation_numbers_are_rejected() {
    let mut s = presets::rules();
    s.layers[0].motion.from_scale = f32::NAN;
    assert!(s.validate().is_err());
    let mut s = presets::rules();
    s.canvas.fps.denominator = 0;
    assert!(s.validate().is_err());
}
#[test]
fn entrance_hold_exit_and_exclusive_end() {
    let s = presets::rules();
    let l = &s.layers[0];
    assert!(state(l, -0.1, 0, 0).is_none());
    assert_eq!(state(l, 0., 0, 0).unwrap().opacity, 0.);
    assert_eq!(state(l, 0.5, 0, 15).unwrap().opacity, 1.);
    assert!(state(l, 1.99, 0, 59).unwrap().opacity < 0.1);
    assert!(state(l, 2., 0, 60).is_none());
}
#[test]
fn easing_endpoints_and_seeded_glitch() {
    for e in [
        Easing::Linear,
        Easing::OutCubic,
        Easing::InOutCubic,
        Easing::OutBack,
    ] {
        assert!(ease(0., &e).abs() < 1e-5);
        assert!((ease(1., &e) - 1.).abs() < 1e-5);
    }
    let a: Vec<_> = (0..20).map(|f| noise(42, f)).collect();
    assert_eq!(a, (0..20).map(|f| noise(42, f)).collect::<Vec<_>>());
    assert_ne!(a, (0..20).map(|f| noise(43, f)).collect::<Vec<_>>());
    assert!(a.iter().all(|v| (-1. ..=1.).contains(v)));
}
#[test]
fn typing_preserves_grapheme_clusters() {
    assert_eq!(typed("A👩‍💻e\u{301}", 1. / 3.), "A");
    assert_eq!(typed("A👩‍💻e\u{301}", 2. / 3.), "A👩‍💻");
    assert_eq!(typed("é", 1.), "é");
}
#[test]
fn output_conflict_and_cleanup_preserve_previous_file() {
    let dir = test_directory();
    let path = dir.path().join("video.mp4");
    std::fs::write(&path, b"original").unwrap();
    assert!(output::StagedOutput::new(&path, false).is_err());
    {
        let stage = output::StagedOutput::new(&path, true).unwrap();
        std::fs::write(stage.path(), b"partial").unwrap();
    }
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn output_publication_does_not_clobber_racing_writer() {
    let dir = test_directory();
    let path = dir.path().join("video.mp4");
    let stage = output::StagedOutput::new(&path, false).unwrap();
    std::fs::write(stage.path(), b"render").unwrap();
    std::fs::write(&path, b"other writer").unwrap();
    assert!(stage.publish(&path, false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"other writer");
}
#[test]
fn all_presets_have_valid_distinct_scenes() {
    for name in presets::NAMES {
        let s = presets::make(
            name,
            if name == "flowchart" {
                "DRAW | RIG | ANIMATE"
            } else {
                "CHALLENGE ACCEPTED"
            },
            3.,
        )
        .unwrap();
        s.validate().unwrap();
    }
}
#[test]
fn image_paths_resolve_against_request_directory() {
    let value = json!({"version":1,"duration":1,"layers":[{"id":"image","start":0,"duration":1,"x":0,"y":0,"width":100,"height":100,"content":{"type":"image","path":"assets/logo.png"}}]});
    let s = resolve_scene(value, Path::new("C:/project")).unwrap();
    if let Content::Image { path } = &s.layers[0].content {
        assert_eq!(*path, Path::new("C:/project/assets/logo.png"));
    } else {
        panic!("wrong content");
    }
}
#[test]
fn native_text_fitting_reports_impossible_bounds() {
    let _runtime = media::Runtime::new().unwrap();
    unsafe {
        let factory = windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
            windows::Win32::Graphics::DirectWrite::DWRITE_FACTORY_TYPE_SHARED,
        )
        .unwrap();
        assert!(layout::fit(
            &factory,
            "UNBREAKABLE LONG HEADING",
            "Segoe UI",
            80.,
            24.,
            20.,
            10.,
            false,
            &Alignment::Left
        )
        .is_err());
        let result = layout::fit(
            &factory,
            "READABLE",
            "Segoe UI",
            100.,
            20.,
            400.,
            100.,
            false,
            &Alignment::Left,
        )
        .unwrap();
        assert!(result.size >= 20. && result.size <= 100.);
    }
}
#[test]
#[ignore = "Native H.264 integration: run explicitly on Windows"]
fn native_video_generation_and_decode() {
    let dir = test_directory();
    let path = dir.path().join("smoke.mp4");
    let mut s = presets::make("impact", "NATIVE TEST", 1.).unwrap();
    s.canvas.width = 640;
    s.canvas.height = 360;
    let result = render::render(&s, &path, false, true, &Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(result["verification"]["frames"], 30);
    assert!(std::fs::metadata(path).unwrap().len() > 1000);
}
#[test]
#[ignore = "Native cancellation: run explicitly on Windows"]
fn cancelling_render_preserves_final_and_cleans_partial() {
    let dir = test_directory();
    let path = dir.path().join("cancel.mp4");
    std::fs::write(&path, b"previous").unwrap();
    let s = presets::make("impact", "CANCEL TEST", 60.).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        signal.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let result = render::render(&s, &path, true, true, &cancel);
    worker.join().unwrap();
    assert_eq!(result.unwrap_err().code, "cancelled");
    assert_eq!(std::fs::read(path).unwrap(), b"previous");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn test_directory() -> tempfile::TempDir {
    // Keep native codec I/O within the workspace in restricted development hosts.
    tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap()
}

#[test]
fn native_paths_preserve_unicode_and_normalize_extended_prefixes() {
    assert_eq!(
        media::native_path(Path::new(r"\\?\C:\scenes\árvíz.png")).to_string_lossy(),
        r"C:\scenes\árvíz.png"
    );
    assert_eq!(
        media::native_path(Path::new(r"\\?\UNC\server\share\asset.png")).to_string_lossy(),
        r"\\server\share\asset.png"
    );
}

#[test]
#[ignore = "Native WIC and Direct2D pixel integration"]
fn native_image_and_chroma_background() {
    let dir = test_directory();
    let _runtime = media::Runtime::new().unwrap();
    let image = dir.path().join("red.png");
    let pixels: Vec<u8> = [0, 0, 255, 255].repeat(32 * 32);
    graphics::save_png(&image, 32, 32, &pixels).unwrap();
    let s: Scene = serde_json::from_value(json!({"version":1,"duration":1,"layers":[{"id":"image","start":0,"duration":1,"x":100,"y":100,"width":100,"height":100,"motion":{"entrance":0,"exit":0,"from_y":0},"content":{"type":"image","path":image}}]})).unwrap();
    s.validate().unwrap();
    let g = graphics::Graphics::new(true).unwrap();
    let mut c = compositor::Compositor::new(&g, &s).unwrap();
    c.draw(&g, &s, 10).unwrap();
    let output = c.pixels(&g).unwrap();
    assert_eq!(&output[0..4], &[0, 255, 0, 255]);
    let offset = (150 * 1920 + 150) * 4;
    assert_eq!(&output[offset..offset + 4], &[0, 0, 255, 255]);
}
