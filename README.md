<div align="center">

# Sh_Images

**Native image viewer · Lightweight · Fast · No Electron**

<a href="README.es.md" style="display:inline-block; padding:8px 24px; background:#2ea44f; color:#ffffff; border-radius:6px; text-decoration:none; font-weight:600; margin-top:8px;">🌐 Español</a>

</div>

---

## English

A native image viewer built in Rust using `egui` + `eframe`. No Electron, no WebView — just a fast,
responsive desktop app powered by an immediate-mode GUI with wgpu rendering.

### Architecture

| Layer | Responsibility |
|-------|---------------|
| **UI** (`src/ui/`) | Immediate-mode rendering: viewer, sidebar, toolbar, info panel, toasts |
| **Core** (`src/core/`) | Image loading, LRU cache, navigation, preloading, EXIF, thumbnails |
| **Config** (`src/config/`) | TOML settings persistence (atomic write), shortcut mapping |
| **Utils** (`src/utils/`) | Error handling, path resolution, platform defaults |

**Concurrency model:** Image decoding, EXIF reading, and thumbnail generation run on dedicated
thread pools. Communication uses `mpsc` channels; shared state (`ImageCache`, `ThumbnailCache`)
is guarded by `Mutex` and wrapped in `Arc`.

**LRU Image Cache:** Bounded by configurable memory limit (default 512 MiB). Eviction removes
least-recently-used entries first. Tracks hit ratio for observability.

**Preloading:** Adjacent folder images are pre-decoded in the background with configurable depth,
ensuring instant navigation.

**Thumbnail Generation:** Capped dimensions (configurable), processed via a bounded thread pool
(3 workers) with a separate LRU cache for decoded thumbnails.

### Features

- **Instant open** — images load in <200 ms (4K) with async decoding
- **Zoom & pan** — centered zoom, mouse wheel, drag to pan, fit-to-window
- **Rotate** — 90° CW/CCW via GPU mesh transform (no re-decode)
- **Navigation** — arrow keys through folder images, sidebar thumbnails with scroll
- **EXIF metadata** — camera model, ISO, aperture, focal length, date, dimensions
- **Animated GIF** — looped playback with frame timing
- **Slideshow** — auto-advance (configurable 1–60 s interval)
- **Fullscreen** — F11 toggle, borderless window
- **Dark/light themes** — persisted between sessions
- **Configurable shortcuts** — edit keybindings via in-app dialog
- **Windows MSI installer** — associates PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF

### Supported Formats

| Format | Encoding | Status |
|--------|----------|--------|
| PNG | 8/16-bit, RGBA | ✅ Supported |
| JPEG | Baseline + progressive | ✅ Supported |
| GIF | Static + animated | ✅ Supported |
| BMP | Uncompressed | ✅ Supported |
| WebP | Lossy + lossless | ✅ Supported |
| TIFF | Multi-page | ✅ Supported |
| AVIF | AV1-based | ✅ Supported |

### Installation

#### Windows (release)
[Download the MSI](https://github.com/Shoking01/Sh_Images/releases) from GitHub Releases and run it.
Supported image extensions are associated automatically.

Other platforms (macOS, Linux) — build from source.

#### From source
```bash
# Requires Rust 1.92+ (stable)
git clone https://github.com/Shoking01/Sh_Images.git
cd Sh_Images
cargo run --release
```

**Build-time requirements:**
- `rustc` 1.92+ (MSRV)
- `cargo` (stable toolchain)
- On Windows (MSI): `cargo-wix` or WiX Toolset v3
- SVG toolchain for icon rendering: `resvg` (bundled via build.rs)

### CLI
```bat
sh_images.exe "C:\path\to\image.png"
```

Pass an image path as the first argument to open it directly.

### Configuration

Settings are stored in `settings.toml` (platform-specific config directory):

```toml
cache_memory_limit_mb = 512
theme = "dark"
slideshow_interval_secs = 5
language = "en"
```

First run creates defaults. Atomic write (temp + rename) prevents corruption.

### Development
```bash
# Run all checks before committing
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test

# Integration tests (9 flows: open, navigate, zoom, error, config, rotate, EXIF, GIF, slideshow)
cargo test --test integration

# Performance benchmarks (criterion)
cargo bench
```

**Code quality:** `clippy -- -D warnings` enforced. `cargo fmt` for formatting. Snapshot testing
via `insta` for UI state.

### Contributing
1. Fork the repo
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Write tests for new functionality
4. Run `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings`
5. Open a Pull Request

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for full guidelines.

### License
MIT License — see [LICENSE](LICENSE).

---

## Technical Details

**Dependencies** (`Cargo.toml`):
- `eframe` 0.35 + `egui` 0.35 — immediate-mode GUI framework with wgpu backend
- `image` 0.25 — image decoding (PNG, JPEG, GIF, BMP, WebP, TIFF, AVIF)
- `kamadak-exif` 0.6.1 — EXIF metadata extraction
- `rfd` 0.15 — native file dialog
- `serde` + `toml` — settings serialization
- `thiserror` — centralized error handling
- `tracing` — structured logging

**Release optimizations:** LTO enabled, single codegen unit, symbols stripped — targeting
binaries under 20 MB.

**Platform:** Windows (primary, MSI installer), macOS and Linux (build from source).

**Version:** 0.2.2 | **MSRV:** Rust 1.92+
