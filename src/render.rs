use crate::{
    compositor::Compositor,
    encoder::{Encoder, SurfacePool},
    error::{Error, Result},
    graphics::Graphics,
    media,
    output::StagedOutput,
    scene::Scene,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

pub fn validate(s: &Scene) -> Result<Value> {
    let mut warnings = s.validate()?;
    let _runtime = media::Runtime::new()?;
    let g = Graphics::new(true)?;
    let c = Compositor::new(&g, s)?;
    warnings.extend(c.warnings);
    Ok(
        json!({"ok":true,"valid":true,"frames":s.frames(),"duration":s.canvas.fps.seconds(s.frames()),"layouts":c.layouts,"warnings":warnings}),
    )
}
pub fn render(
    s: &Scene,
    output: &Path,
    overwrite: bool,
    software: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<Value> {
    let _runtime = media::Runtime::new()?;
    let started = Instant::now();
    let warnings = s.validate()?;
    let attempt = |cpu: bool| -> Result<Value> {
        media::check_cancel(cancel)?;
        let stage = StagedOutput::new(output, overwrite)?;
        let g = Graphics::new(software || cpu)?;
        let mut c = Compositor::new(&g, s)?;
        let encoder = Encoder::new(
            stage.path(),
            s,
            &g,
            !software && !cpu && g.hardware,
            software || cpu,
        )?;
        let encoder_info = encoder.info.clone();
        let gpu_input = encoder.gpu_input;
        let pool = if gpu_input {
            Some(SurfacePool::new(&g, s.canvas.width, s.canvas.height)?)
        } else {
            None
        };
        let mut last = Instant::now();
        for frame in 0..s.frames() {
            media::check_cancel(cancel)?;
            c.draw(&g, s, frame)?;
            let a = s.canvas.fps.ticks(frame);
            let b = s.canvas.fps.ticks(frame + 1);
            if let Some(pool) = &pool {
                pool.write(&g, &encoder, &c.output, a, b - a, cancel)?;
            } else {
                encoder.video(c.pixels(&g)?, a, b - a)?;
            }
            if last.elapsed().as_secs_f64() > 0.5 || frame + 1 == s.frames() {
                media::event(
                    json!({"event":"progress","completed_frames":frame+1,"total_frames":s.frames(),"elapsed_seconds":started.elapsed().as_secs_f64()}),
                );
                last = Instant::now();
            }
        }
        encoder.finish()?;
        let encode_seconds = started.elapsed().as_secs_f64();
        media::check_cancel(cancel)?;
        let inspection = media::verify_video(stage.path(), cancel)?;
        if inspection["frames"] != s.frames()
            || inspection["width"] != s.canvas.width
            || inspection["height"] != s.canvas.height
            || inspection["first_timestamp_ticks"] != 0
            || inspection["last_timestamp_ticks"]
                .as_i64()
                .is_none_or(|last| last.abs_diff(s.canvas.fps.ticks(s.frames() - 1)) > 1)
        {
            return Err(Error::new(
                "verification_failed",
                format!("Encoded output does not match scene: {inspection}"),
            ));
        }
        let bytes = std::fs::metadata(stage.path())?.len();
        let mut all_warnings = warnings.clone();
        all_warnings.extend(c.warnings.clone());
        media::check_cancel(cancel)?;
        stage.publish(output, overwrite)?;
        Ok(
            json!({"ok":true,"output":output,"frames":s.frames(),"duration":s.canvas.fps.seconds(s.frames()),"width":s.canvas.width,"height":s.canvas.height,"file_bytes":bytes,"render_seconds":encode_seconds,"total_seconds":started.elapsed().as_secs_f64(),"render_fps":s.frames() as f64/encode_seconds,"peak_working_set_bytes":media::peak_memory(),"gpu_memory_bytes":null,"graphics":g.info,"encoder":encoder_info,"application_gpu_pool_size":if gpu_input{8}else{0},"warnings":all_warnings,"verification":inspection}),
        )
    };
    match attempt(false) {
        Err(e) if !software && matches!(e.code, "native_failed" | "gpu_surface_failed") => {
            media::event(
                json!({"event":"fallback","reason":e.to_string(),"path":"warp_system_memory"}),
            );
            let mut value = attempt(true)?;
            value["warnings"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!("Native fallback: {e}")));
            Ok(value)
        }
        result => result,
    }
}
