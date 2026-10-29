//! Presets compile into the same public layer model as hand-authored scenes.
use crate::{
    error::{Error, Result},
    scene::*,
};
use serde_json::json;
pub const NAMES: [&str; 6] = [
    "rules",
    "impact",
    "terminal",
    "counter",
    "callout",
    "flowchart",
];
fn layer(
    id: &str,
    start: f64,
    duration: f64,
    bounds: [f32; 4],
    content: serde_json::Value,
    motion: serde_json::Value,
) -> Layer {
    serde_json::from_value(json!({"id":id,"start":start,"duration":duration,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"content":content,"motion":motion})).expect("built-in preset is valid")
}
fn text(
    id: &str,
    start: f64,
    duration: f64,
    bounds: [f32; 4],
    value: &str,
    size: f32,
    color: &str,
) -> Layer {
    layer(
        id,
        start,
        duration,
        bounds,
        json!({"type":"text","text":value,"font_size":size,"min_font_size":size.min(24.),"color":color}),
        json!({"from_x":-32,"from_y":0}),
    )
}
pub fn rules() -> Scene {
    let cards = [
        ("THE CHALLENGE RULES", "Create a PNGTuber from scratch", 2.0),
        ("NO AI", "I have to create the PNGTuber manually.", 2.6),
        (
            "NO MODIFICATIONS",
            "I can’t modify an existing PNGTuber.",
            2.8,
        ),
        (
            "NO ONLINE AVATAR\nCREATOR TOOLS",
            "No online builders or automatic makers.",
            3.3,
        ),
        (
            "NO DOWNLOADED\nIMAGE EDITS",
            "I can’t download an image and modify it.",
            3.1,
        ),
        ("FROM SCRATCH.\nBY HAND.", "CHALLENGE ACCEPTED", 2.2),
    ];
    let mut s = Scene {
        version: 1,
        duration: 16.,
        canvas: Canvas::default(),
        theme: Theme::default(),
        seed: 42,
        layers: Vec::new(),
    };
    let mut start = 0.;
    for (i, (heading, body, duration)) in cards.into_iter().enumerate() {
        let id = |part: &str| format!("card-{i}-{part}");
        s.layers.push(layer(&id("panel"),start,duration,[140.,230.,1640.,620.],json!({"type":"rectangle","color":"panel","radius":8.,"stroke":"outline","stroke_width":4.}),json!({"from_x":-65,"from_y":0,"reveal":true})));
        s.layers.push(layer(
            &id("accent"),
            start + 0.04,
            duration - 0.04,
            [140., 230., 12., 620.],
            json!({"type":"rectangle","color":"yellow"}),
            json!({"from_y":0,"reveal":true}),
        ));
        s.layers.push(text(
            &id("eyebrow"),
            start + 0.08,
            duration - 0.08,
            [205., 275., 1430., 55.],
            &if i == 0 {
                "CHALLENGE PROTOCOL  /  01".into()
            } else if i == 5 {
                "ALL SYSTEMS READY".into()
            } else {
                format!("RULE {i:02}  /  04")
            },
            28.,
            "muted",
        ));
        if (1..=4).contains(&i) {
            s.layers.push(text(
                &id("number"),
                start + 0.09,
                duration - 0.09,
                [205., 365., 180., 180.],
                &format!("{i:02}"),
                112.,
                "yellow",
            ));
        }
        let x = if (1..=4).contains(&i) { 440. } else { 205. };
        s.layers.push(text(
            &id("heading"),
            start + 0.12,
            duration - 0.12,
            [x, 365., 1660. - x, 245.],
            heading,
            if i == 3 { 84. } else { 92. },
            "yellow",
        ));
        let mut explanation = text(
            &id("body"),
            start + 0.19,
            duration - 0.19,
            [x, 640., 1660. - x, 110.],
            body,
            40.,
            "white",
        );
        if let Content::Text { font, .. } = &mut explanation.content {
            *font = "body".into();
        }
        s.layers.push(explanation);
        s.layers.push(text(
            &id("footer"),
            start + 0.2,
            duration - 0.2,
            [205., 790., 1400., 40.],
            "CAPTAIN FLASKY BOY   //   PNGTUBER CHALLENGE",
            22.,
            "muted",
        ));
        start += duration;
    }
    s.layers.push(layer("timeline",0.,16.,[140.,900.,1640.,7.],json!({"type":"progress","from":0.,"to":1.,"fill_seconds":16.,"color":"yellow","track":"panel"}),json!({"entrance":0.,"exit":0.,"from_y":0.})));
    s
}
pub fn make(name: &str, value: &str, duration: f64) -> Result<Scene> {
    if !duration.is_finite() || !(0.6..=600.).contains(&duration) {
        return Err(Error::new(
            "invalid_arguments",
            "duration must be 0.6..600 seconds",
        ));
    }
    if name == "rules" {
        return Ok(rules());
    }
    let mut s = Scene {
        version: 1,
        duration,
        canvas: Canvas::default(),
        theme: Theme::default(),
        seed: 42,
        layers: Vec::new(),
    };
    match name {
        "impact" => {
            s.layers.push(layer(
                "backing",
                0.,
                duration,
                [170., 350., 1580., 370.],
                json!({"type":"rectangle","color":"panel"}),
                json!({"from_x":-100,"from_y":0,"reveal":true}),
            ));
            let mut title = text(
                "title",
                0.08,
                duration - 0.08,
                [230., 425., 1460., 220.],
                value,
                116.,
                "yellow",
            );
            title.motion.from_scale = 0.78;
            title.motion.easing = Easing::OutBack;
            title.motion.glitch = 3.;
            s.layers.push(title);
        }
        "terminal" => {
            s.layers.push(layer(
                "terminal-panel",
                0.,
                duration,
                [160., 200., 1600., 680.],
                json!({"type":"rectangle","color":"panel","stroke":"muted","stroke_width":2.}),
                json!({"reveal":true,"from_y":0.}),
            ));
            s.layers.push(text(
                "terminal-header",
                0.,
                duration,
                [210., 235., 1480., 60.],
                "C:\\CHALLENGE> RUN PROTOCOL",
                30.,
                "yellow",
            ));
            s.layers.push(layer("terminal-text",0.2,duration-0.2,[210.,350.,1480.,460.],json!({"type":"text","text":value,"font":"mono","font_size":46.,"typing_seconds":(duration-0.5)*0.7,"cursor":true,"scroll_lines":3}),json!({"from_y":0.})));
        }
        "counter" => {
            s.layers.push(text(
                "counter-label",
                0.,
                duration,
                [220., 270., 1480., 130.],
                value,
                60.,
                "white",
            ));
            s.layers.push(layer("counter",0.,duration,[220.,440.,1480.,220.],json!({"type":"counter","from":0.,"to":100.,"suffix":"%","font_size":160.,"count_seconds":duration*0.7}),json!({"from_y":0.})));
            s.layers.push(layer(
                "progress",
                0.,
                duration,
                [220., 750., 1480., 24.],
                json!({"type":"progress","fill_seconds":duration*0.7,"easing":"out_cubic"}),
                json!({"from_y":0.}),
            ));
        }
        "callout" => {
            for (id, bounds) in [
                ("top", [400., 330., 1100., 3.]),
                ("left", [400., 330., 3., 380.]),
                ("bottom", [400., 710., 1100., 3.]),
            ] {
                s.layers.push(layer(
                    id,
                    0.,
                    duration,
                    bounds,
                    json!({"type":"line","color":"yellow","thickness":4.}),
                    json!({"reveal":true,"from_y":0.,"entrance":0.4}),
                ));
            }
            s.layers.push(text(
                "label",
                0.25,
                duration - 0.25,
                [455., 450., 1010., 200.],
                value,
                72.,
                "white",
            ));
        }
        "flowchart" => {
            let nodes: Vec<_> = value.split('|').map(str::trim).collect();
            if nodes.is_empty() || nodes.len() > 4 || nodes.iter().any(|x| x.is_empty()) {
                return Err(Error::new(
                    "invalid_arguments",
                    "flowchart requires 1..4 nonempty labels separated by |",
                ));
            }
            let w = (1600. - (nodes.len() - 1) as f32 * 60.) / nodes.len() as f32;
            for (i, node) in nodes.iter().enumerate() {
                let start = i as f64 * duration * 0.15;
                let x = 160. + i as f32 * (w + 60.);
                s.layers.push(layer(
                    &format!("node-{i}"),
                    start,
                    duration - start,
                    [x, 400., w, 260.],
                    json!({"type":"rectangle","color":"panel","stroke":"yellow","stroke_width":3.}),
                    json!({"from_y":25.}),
                ));
                s.layers.push(text(
                    &format!("label-{i}"),
                    start + 0.08,
                    duration - start - 0.08,
                    [x + 22., 445., w - 44., 170.],
                    node,
                    48.,
                    "white",
                ));
                if i > 0 {
                    s.layers.push(layer(
                        &format!("edge-{i}"),
                        start - 0.08,
                        duration - start + 0.08,
                        [x - 60., 530., 60., 1.],
                        json!({"type":"line","color":"yellow"}),
                        json!({"reveal":true,"from_y":0.}),
                    ));
                }
            }
        }
        _ => {
            return Err(Error::new(
                "invalid_arguments",
                format!("Unknown preset {name}; supported: {}", NAMES.join(", ")),
            ))
        }
    }
    s.validate()?;
    Ok(s)
}
