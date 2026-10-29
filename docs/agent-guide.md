# Agent usage

Turn the natural-language brief into explicit text, layout, theme, timing and seed choices. Make creative decisions yourself; do not ask the user to choose previews. No local AI model or MCP server is needed.

1. Run `cmtga capabilities` and `cmtga doctor` once when establishing the environment.
2. Start with `cmtga preset --name rules` or another preset. Capture its stdout as UTF-8 JSON; modify the scene for the brief. The rules preset supplies the full 16-second challenge sequence and ignores the text/duration shortcuts.
3. Keep headings and supporting text separate, maintain generous margins, and allocate readable hold time. Entrance and exit durations count toward each layer's total duration. Layer starts are scene-relative seconds. No layer may end after the scene.
4. Run `cmtga validate --request scene.json`. Check fitted font sizes and warnings. Correct invalid geometry or insufficient text space.
5. Render directly with `cmtga render --request scene.json --output final.mp4`. Existing output requires explicit `--overwrite`.
6. Read the final JSON, including frame count, warnings, fallback information and performance. Report a finished file only when `ok` is true. Inspect frames internally when producing or revising a visual design; never turn this into a user-facing selection stage.

`text` renders a convenience preset immediately. `preset` emits its editable scene. Flowchart text uses `|` to separate up to four nodes. Counters and progress ranges can be edited in JSON. Terminal text accepts newlines, Unicode and a blinking cursor.

Use saved themes through `--theme themes/captain-flasky-boy.json` with render, text or validate. Absolute asset paths work, but relative paths make scenes portable. Never place credentials or identifying machine paths in published scenes.

For automated callers, invoke the executable with an argument array, consume stdout JSON separately from stderr progress, and honor exit codes. Send Ctrl+C for cooperative cancellation. Batch is sequential to bound application work; native codecs manage their own worker threads.
