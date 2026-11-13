# Kinetic typography verification — 13 November 2026

Locally verified on Windows x64, Rust 1.99.1, optimized Windows GNU build, Intel Iris Xe. MSVC verification is performed by the repository's Windows CI.

## Completed checks

- Formatting and Clippy with warnings denied passed. Twenty-four unit tests and three CLI integration tests passed; four additional native tests passed explicitly, including 64 MiB texture-cache eviction and recreation.
- Existing layer-scene tests, the challenge-rules demo, software/GPU smoke renders and diagnostic frame exports passed. Mixed batches containing a legacy scene and a kinetic request passed.
- The kinetic demo rendered through both native hardware and software paths. CMTGA decoded all frames before publishing each MP4. Independent development-only FFprobe confirmed H.264 yuv420p, 1920x1080, 60/1 fps, 456 decoded frames, and 7.599983-second container duration (timestamp quantization).
- Uncompressed pixel tests verified exact `#121212` background, binary sharp amber glyph edges with shadow disabled, centering within one pixel, emphasis scaling, safe margins at peak scale, unsupported-glyph errors and distinct lower/uppercase glyphs. Decoded frames were inspected at entrances, hard cuts, held gaps, repeated words and long words; shadows were visually checked separately from crisp foreground pixels.
- The executable ran alone in a separate directory with an empty PATH, read its request from stdin, and rendered/decoded 456 frames. No sidecar font file was present. Press Start 2P was absent from the installed font collection; the reported font source was embedded. Embedded font licence notices were verified.
- Cache stress drew 120 unique words, stayed within the 64 MiB cache budget, and successfully recreated the evicted first word. Cached glyph/shadow resources are separate from scratch surfaces, encoder buffers, word metadata and driver allocations.

## Measurements

Single optimized runs, 1080p60. Render time includes preparation, encoding and MP4 finalization. Total additionally includes full native decode verification and publication; process startup is excluded. Native codecs may vary in rate control and output size despite the same requested bitrate.

| Path / duration | Frames | Render + encode | Total | File bytes | Peak process working set | Cached word textures |
|---|---:|---:|---:|---:|---:|---:|
| Hardware, 7.6 s | 456 | 2.07 s | 2.72 s | 16,499,058 | 288.2 MiB | 5.46 MiB |
| Software, 7.6 s | 456 | 3.31 s | 3.98 s | 705,207 | 310.3 MiB | 5.46 MiB |
| Hardware, 76 s | 4,560 | 14.64 s | 21.08 s | 187,483,310 | 302.1 MiB | 5.46 MiB |

The long sequence repeats the demo ten times. All three runs generated 11 unique word textures; repeated words reused the cache. Process memory did not grow in proportion to the tenfold duration increase in this workload. GPU memory was not measured separately. Sanitized measurements: [kinetic-benchmark-results.json](kinetic-benchmark-results.json).

The demonstration's timings are illustrative, with no associated speech recording. Its reusable request is `examples/kinetic-typography.json`, and its frame-boundary/fitting report is [kinetic-demo-timing.json](kinetic-demo-timing.json). No CapCut import, chroma keying or playback was tested; those are separate from successful asset generation.

## Reproduction

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --release --locked -- --ignored --test-threads=1
.\scripts\build.ps1
.\scripts\verify.ps1
.\scripts\benchmark.ps1 -Request examples/kinetic-typography.json
.\scripts\benchmark.ps1 -Request examples/kinetic-typography.json -Acceleration software
cmtga validate --request examples/kinetic-typography.json
```

To reproduce the longer workload, append ten copies of the demo's words, offsetting each copy's start/end times by 7.6 seconds. Keep style and frame rate unchanged. Visual QA is internal and does not introduce a mandatory preview workflow.
