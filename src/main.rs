#![allow(non_snake_case)]
mod compositor;
mod encoder;
mod error;
mod graphics;
mod layout;
mod media;
mod output;
mod presets;
mod render;
mod scene;
mod timing;
use error::{Error, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

fn options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i]
            .strip_prefix("--")
            .ok_or_else(|| Error::new("invalid_arguments", "Expected --named options"))?;
        if !allowed.contains(&key) || out.contains_key(key) {
            return Err(Error::new(
                "invalid_arguments",
                format!("Unknown or repeated option --{key}"),
            ));
        }
        if key == "overwrite" {
            out.insert(key.into(), "true".into());
            i += 1;
        } else {
            let value = args
                .get(i + 1)
                .filter(|s| !s.starts_with("--"))
                .ok_or_else(|| {
                    Error::new("invalid_arguments", format!("Missing value for --{key}"))
                })?;
            out.insert(key.into(), value.clone());
            i += 2;
        }
    }
    Ok(out)
}
fn required<'a>(o: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    o.get(key)
        .map(String::as_str)
        .ok_or_else(|| Error::new("invalid_arguments", format!("Missing --{key}")))
}
fn read_json(path: &str) -> Result<(Value, PathBuf)> {
    let (text, base) = if path == "-" {
        let mut text = String::new();
        std::io::stdin()
            .take(4 * 1024 * 1024 + 1)
            .read_to_string(&mut text)?;
        (text, std::env::current_dir()?)
    } else {
        let p = Path::new(path).canonicalize()?;
        if std::fs::metadata(&p)?.len() > 4 * 1024 * 1024 {
            return Err(Error::new("invalid_scene", "Request exceeds 4 MiB"));
        }
        (
            std::fs::read_to_string(&p)?,
            p.parent().unwrap().to_path_buf(),
        )
    };
    if text.len() > 4 * 1024 * 1024 {
        return Err(Error::new("invalid_scene", "Request exceeds 4 MiB"));
    }
    Ok((
        serde_json::from_str(text.trim_start_matches('\u{feff}'))?,
        base,
    ))
}
fn resolve_scene(value: Value, base: &Path) -> Result<scene::Scene> {
    let mut s: scene::Scene = serde_json::from_value(value)?;
    for layer in &mut s.layers {
        if let scene::Content::Image { path } = &mut layer.content {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
    }
    Ok(s)
}
fn load(path: &str) -> Result<scene::Scene> {
    let (v, base) = read_json(path)?;
    resolve_scene(v, &base)
}
fn apply_theme(s: &mut scene::Scene, o: &BTreeMap<String, String>) -> Result<()> {
    if let Some(path) = o.get("theme") {
        let (value, _) = read_json(path)?;
        s.theme = serde_json::from_value(value)?;
    }
    Ok(())
}
fn software(o: &BTreeMap<String, String>) -> Result<bool> {
    match o.get("acceleration").map(String::as_str).unwrap_or("auto") {
        "auto" => Ok(false),
        "software" => Ok(true),
        _ => Err(Error::new(
            "invalid_arguments",
            "--acceleration must be auto or software",
        )),
    }
}
fn execute(cancel: &Arc<AtomicBool>) -> Result<Value> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let rest = args.get(1..).unwrap_or_default();
    match command {
        "help" | "--help" | "-h" | "capabilities" => {
            options(rest, &[])?;
            Ok(
                json!({"ok":true,"name":"ContentMotionTextGraphicsAssitant","version":env!("CARGO_PKG_VERSION"),"scene_version":1,"presets":presets::NAMES,"commands":["doctor","capabilities","schema","preset --name rules","validate --request scene.json","render --request scene.json --output video.mp4 [--overwrite] [--acceleration auto|software]","text --text TITLE --preset impact --duration 3 --output title.mp4","batch --request jobs.json [--overwrite]","frame --request scene.json --time 1 --output frame.png"],"examples":["cmtga render --request examples/challenge-rules.json --output challenge-rules.mp4","cmtga text --text CHALLENGE --preset impact --duration 3 --output title.mp4"],"schema_command":"cmtga schema","layer_types":["text","rectangle","line","image","counter","progress"]}),
            )
        }
        "--version" => {
            options(rest, &[])?;
            Ok(json!({"ok":true,"version":env!("CARGO_PKG_VERSION")}))
        }
        "schema" => {
            options(rest, &[])?;
            Ok(serde_json::to_value(schemars::schema_for!(scene::Scene))?)
        }
        "doctor" => {
            options(rest, &[])?;
            media::doctor()
        }
        "preset" => {
            let o = options(rest, &["name", "text", "duration"])?;
            let duration = o
                .get("duration")
                .map_or(Ok(3.), |s| s.parse::<f64>())
                .map_err(|_| Error::new("invalid_arguments", "Invalid duration"))?;
            Ok(serde_json::to_value(presets::make(
                required(&o, "name")?,
                o.get("text")
                    .map(String::as_str)
                    .unwrap_or("CHALLENGE ACCEPTED"),
                duration,
            )?)?)
        }
        "validate" => {
            let o = options(rest, &["request", "theme"])?;
            let mut s = load(required(&o, "request")?)?;
            apply_theme(&mut s, &o)?;
            render::validate(&s)
        }
        "render" | "text" => {
            let allowed = if command == "render" {
                vec!["request", "output", "overwrite", "acceleration", "theme"]
            } else {
                vec![
                    "text",
                    "preset",
                    "duration",
                    "output",
                    "overwrite",
                    "acceleration",
                    "theme",
                ]
            };
            let o = options(rest, &allowed)?;
            let mut s = if command == "render" {
                load(required(&o, "request")?)?
            } else {
                let duration = o
                    .get("duration")
                    .map_or(Ok(3.), |s| s.parse::<f64>())
                    .map_err(|_| Error::new("invalid_arguments", "Invalid duration"))?;
                presets::make(
                    o.get("preset").map(String::as_str).unwrap_or("impact"),
                    required(&o, "text")?,
                    duration,
                )?
            };
            apply_theme(&mut s, &o)?;
            if command == "text" && o.contains_key("theme") {
                for layer in &mut s.layers {
                    layer.motion.entrance = s.theme.entrance.min(layer.duration / 2.);
                    layer.motion.exit = s.theme.exit.min(layer.duration / 2.);
                }
            }
            render::render(
                &s,
                Path::new(required(&o, "output")?),
                o.contains_key("overwrite"),
                software(&o)?,
                cancel,
            )
        }
        "frame" => {
            let o = options(rest, &["request", "time", "output", "overwrite"])?;
            let s = load(required(&o, "request")?)?;
            s.validate()?;
            let time = required(&o, "time")?
                .parse::<f64>()
                .map_err(|_| Error::new("invalid_arguments", "Invalid time"))?;
            if !time.is_finite() || time < 0. || time >= s.duration {
                return Err(Error::new(
                    "invalid_arguments",
                    "Frame time must lie inside scene",
                ));
            }
            let output = Path::new(required(&o, "output")?);
            if output.exists() && !o.contains_key("overwrite") {
                return Err(Error::new("output_exists", "Frame output exists"));
            }
            let _runtime = media::Runtime::new()?;
            let g = graphics::Graphics::new(true)?;
            let mut c = compositor::Compositor::new(&g, &s)?;
            c.draw(
                &g,
                &s,
                (time * s.canvas.fps.numerator as f64 / s.canvas.fps.denominator as f64).floor()
                    as u64,
            )?;
            graphics::save_png(output, s.canvas.width, s.canvas.height, c.pixels(&g)?)?;
            Ok(json!({"ok":true,"output":output,"time":time}))
        }
        "batch" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Batch {
                version: u32,
                jobs: Vec<Job>,
            }
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Job {
                request: PathBuf,
                output: PathBuf,
            }
            let o = options(rest, &["request", "overwrite", "acceleration"])?;
            let (v, base) = read_json(required(&o, "request")?)?;
            let batch: Batch = serde_json::from_value(v)?;
            if batch.version != 1 || batch.jobs.is_empty() || batch.jobs.len() > 100 {
                return Err(Error::new(
                    "invalid_scene",
                    "Batch version must be 1 with 1..100 jobs",
                ));
            }
            let mut outputs = std::collections::HashSet::new();
            let mut jobs = Vec::new();
            for job in batch.jobs {
                let request = base.join(job.request);
                let output = base.join(job.output);
                let parent = output.parent().unwrap_or(Path::new("."));
                let canonical_parent = parent.canonicalize()?;
                let file = output.file_name().ok_or_else(|| {
                    Error::new("invalid_scene", "Batch output requires a filename")
                })?;
                let key = canonical_parent.join(file).to_string_lossy().to_lowercase();
                if !outputs.insert(key) {
                    return Err(Error::new(
                        "invalid_scene",
                        "Batch output paths must be unique",
                    ));
                }
                let s =
                    load(request.to_str().ok_or_else(|| {
                        Error::new("invalid_scene", "Request path must be Unicode")
                    })?)?;
                s.validate()?;
                jobs.push((s, output));
            }
            let mut results = Vec::new();
            for (s, output) in jobs {
                media::check_cancel(cancel)?;
                match render::render(
                    &s,
                    &output,
                    o.contains_key("overwrite"),
                    software(&o)?,
                    cancel,
                ) {
                    Ok(v) => results.push(v),
                    Err(e) if e.code == "cancelled" => return Err(e),
                    Err(e) => results.push(e.json()),
                }
            }
            Ok(json!({"ok":results.iter().all(|v|v["ok"]==true),"jobs":results}))
        }
        _ => Err(Error::new(
            "invalid_arguments",
            format!("Unknown command {command}; run cmtga help"),
        )),
    }
}
fn main() {
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    if let Err(e) =
        ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::SeqCst))
    {
        let e = Error::new("native_failed", e.to_string());
        eprintln!("{}", e.json());
        println!("{}", e.json());
        std::process::exit(e.exit_code());
    }
    match execute(&cancel) {
        Ok(value) => {
            let failed = value.get("ok") == Some(&Value::Bool(false));
            println!("{value}");
            if failed {
                std::process::exit(5);
            }
        }
        Err(e) => {
            eprintln!("{}", e.json());
            println!("{}", e.json());
            std::process::exit(e.exit_code());
        }
    }
}

#[cfg(test)]
mod tests;
