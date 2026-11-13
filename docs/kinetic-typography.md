# Full-screen kinetic typography

Use `cmtga kinetic --request words.json --output kinetic.mp4` to render one word at a time over a matte background. The same request works with `render`, `validate`, `frame`, and mixed `batch` jobs. Generate its schema with `cmtga schema --kind kinetic`; plain `cmtga schema` still describes layer scenes.

```json
{
  "kind": "kinetic",
  "version": 1,
  "words": [
    {"word": "from", "start": 0.0, "end": 0.25},
    {"word": "scratch", "start": 0.3, "end": 0.9, "emphasis": true}
  ]
}
```

Times are seconds relative to the exported video. Supply real word timings from your editing agent, caption alignment or manual timing. CMTGA does not transcribe, align or embed narration. `examples/kinetic-typography.json` has illustrative beats and no associated recording; do not describe it as synchronized to real speech.

## Timing and casing

Default `gap_mode: "hold"` keeps each word until the next word starts, then cuts immediately. The last word ends at its supplied end. Before the first word, only the background appears. Use `gap_mode: "blank"` for gaps between supplied ends and starts.

Regular words become lowercase; emphasized words become uppercase at 1.2 times the fitted base size. Punctuation stays attached. Inputs require a nonempty word without whitespace and ordered non-overlapping finite intervals from 0 to 600 seconds. Limits: 10,000 words, 1,024 UTF-8 bytes per word, 4 MiB request size. Unsupported glyphs produce indexed errors instead of font substitution.

Boundaries map to the first frame at or after their timestamps, subject to floating-point tolerance. Words receiving no display frames are rejected. A subframe speech interval can still have visible frames when hold mode extends it to the next start. Validation returns `start_frame`, exclusive `end_frame` (including any held gap), and `speech_end_frame`. The 80 ms curve is sampled at the chosen rate: approximately five frames at 60 fps. Repeated words restart their own animation.

## Style

`canvas` defaults to 1920x1080, `fps: {"numerator":60,"denominator":1}`, `background: "#121212"`, and `bitrate_kbps: 20000`. Partial overrides retain these kinetic defaults. A uniform design-to-output scale keeps pixel cells square at other aspect ratios. Existing layer scenes retain their green 1080p30 defaults.

| `style` setting | Default |
|---|---:|
| color | `#FFCC00` |
| font_size | 160 logical pixels |
| min_font_size | 64 logical pixels |
| emphasis_scale | 1.2 |
| margin | 96 logical pixels |
| pop_ms | 80 |
| initial_scale | 0.85 |
| peak_scale | 1.04 |
| settle_scale | 1.0 |
| hold_scale | 1.04 |

The pop's first half uses ease-out cubic to reach peak scale; its second half uses ease-in/out cubic to settle. Remaining display time grows linearly toward `hold_scale`. Short words compress the pop to their display interval. Set `pop_ms: 0` to disable it. Opacity remains fully visible: no fades or overlapping words.

`style.shadow` defaults to `{"color":"#000000","opacity":0.8,"offset_x":0,"offset_y":6,"blur_radius":16}`. Gaussian standard deviation is half the blur radius. Font sizes, margins, offsets and blur use logical pixels and scale with resolution. Colours use `#RRGGBB`.

All words share a fitted base size reserving margin, maximum scale, emphasis and shadow space. If fitting would require less than `min_font_size`, validation fails. Ordinary word sizes stay consistent instead of every short word enlarging independently. Centering uses visible glyph bounds, excluding the shadow.

The unmodified regular **Press Start 2P** font is embedded and loaded privately through DirectWrite. No installation or runtime font file is needed. Its OFL notice is included in `cmtga licenses` and `third-party/press-start-2p/OFL.txt`. Silkscreen is not bundled because its lowercase forms do not provide the requested visible casing contrast.

Glyphs rasterize without antialiasing at their native 8-pixel grid, then enlarge with nearest-neighbour sampling and pixel-aligned placement. Pixel steps during zoom are intentional; shadows are separately filtered. H.264 chroma subsampling and playback resizing can soften decoded coloured edges even when source pixels are exact.

The word/shadow texture cache has a 64 MiB eviction budget. CPU word metadata, reusable scratch targets, codec buffers and driver allocations are separate. Results report cache usage and peak process working set; GPU memory is not measured separately.

## Workflow

Create timed JSON, validate, then render directly. Kinetic requests configure `style` and `canvas`; `--theme` applies to layer scenes. Use `frame --time` for internal QA without a mandatory preview stage.

The matte background is part of the finished video, which can cover the screen over existing narration without chroma keying. Unknown CapCut compatibility never blocks rendering. Report editor import/playback only if actually tested.
