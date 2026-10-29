# Verification — 29 October 2026

Verified locally on Windows x64 using Rust 1.99.1, the Windows GNU toolchain and an Intel Iris Xe graphics adapter. MSVC builds are configured in Windows CI; local MSVC execution was not performed.

## Checks completed

- `cargo fmt --check` and `cargo clippy --all-targets --locked -- -D warnings` passed.
- Fourteen unit tests and one CLI integration test passed. Three additional native integration tests passed explicitly: H.264 generation/decode, cancellation preserving prior output, and WIC image rendering with exact chroma-background pixels.
- All six presets rendered successfully. Decoded frames were inspected for text bounds, hierarchy, wrapping and reveal timing. The challenge opening, four rules and ending were inspected during their holds and selected entrances.
- Native verification decoded every demo frame. Independent development-only FFprobe confirmed H.264, yuv420p, 1920x1080, 30/1 fps, 480 frames and container duration 15.999967 seconds (timestamp quantization).
- Software and hardware paths rendered actual MP4s. Hardware transform metadata was observed for the GPU path. Batch rendering, Unicode/space-containing paths, output conflicts and frame export were exercised.
- The release executable rendered successfully with an empty process PATH. Import inspection found only Windows system DLLs. No FFmpeg, Python or browser runtime is used by the executable.
- The distributable executable was built with profile-path remapping and checked for embedded local profile paths. Public source and metadata are audited separately before publication.

## Measurements

Optimized builds, 1080p30. `render_seconds` includes preparation, frame production, encoding and MP4 finalization. `total_seconds` additionally includes native full-frame decode verification and publication; process startup is excluded. These are individual measurements, not statistical throughput guarantees.

| Scene/path | Frames | Render + encode | Total | File size | Peak working set |
|---|---:|---:|---:|---:|---:|
| 16-second rules, hardware | 480 | 2.35 s | 3.57 s | 38,123,721 bytes | 298.7 MiB |
| 16-second rules, software | 480 | 4.31 s | 5.65 s | 3,308,440 bytes | 307.3 MiB |
| 64-second rules, hardware | 1,920 | 8.24 s | 12.24 s | 158,117,448 bytes | 300.3 MiB |

The hardware encoder produced approximately constant-bitrate output, whereas the software encoder used much less data for the same mostly static content. A requested bitrate is not a guarantee of identical encoder rate control. An earlier cold hardware run took 7.65 seconds including verification, illustrating warm-up and system variation.

The application uses an eight-surface GPU pool, a reused CPU readback buffer when needed, and cached static drawing resources. Increasing scene duration fourfold did not cause proportional process-memory growth in this workload. Working-set figures include native codecs, drivers and decode verification; they do not isolate application allocations. GPU memory was not measured separately. Counter layouts are regenerated only when their displayed value changes.

Raw sanitized measurements are in [benchmark-results.json](benchmark-results.json). Reproduce the longer case by multiplying the challenge scene duration, all layer starts/durations, and progress fill duration by four. Keep entrance/exit times unchanged.

## Limits and reproduction

Arial Black was unavailable on the verification machine; the documented Segoe UI fallback was used and reported. Visual layout depends on installed fonts. CapCut import/playback was not tested. The YouTube reference could not be inspected; the written specification guided the design.

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --locked -- --ignored --test-threads=1
.\scripts\build.ps1
.\scripts\verify.ps1
.\scripts\benchmark.ps1
.\scripts\package.ps1
```

`frame` is an optional noninteractive diagnostic command; normal rendering has no preview stage. Native integration fixtures stay inside the workspace so restricted hosts can run them. Abrupt termination can leave partial files; cooperative Ctrl+C cleanup is tested.
