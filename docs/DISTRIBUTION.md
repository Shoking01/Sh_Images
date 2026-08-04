# Distribution & Release Guide — Sh_Images

> Fase 5 (Windows). macOS/Linux son stubs — ver final de este documento.

---

## Windows

### Requisitos locales (desarrollador)

| Herramienta | Versión | Instalación |
|---|---|---|
| Rust toolchain | ≥ 1.92 (stable) | `rustup install stable` |
| Visual C++ Build Tools | 14.x | Incluido en VS Build Tools / `choco install visualstudio2022buildtools` |
| WiX Toolset | 3.14 | `choco install wix --version=3.14.0` |
| (Opcional) Git | cualquier | para `cargo publish` / taggear |

El `rc.exe` (Resource Compiler) de MSVC es necesario para que `winres` incruste
el icono en el `.exe`. Viene con VS Build Tools. En CI Windows (`windows-latest`)
ya está incluido.

### Generar el icono

No es necesario hacerlo manualmente — `build.rs` genera `OUT_DIR/icon.ico`
desde `assets/icon.svg` en cada build. Para copiarlo al dir del instalador:

```bat
:: Después de cargo build --release
for /r target\release\build \%i in (icon.ico) do copy "%i" installer\windows\icon.ico
```

### Generar el MSI (local)

```bat
cargo build --release --locked
cd installer/windows
set VERSION=0.1.0
set RELEASE_DIR=..\..\target\release
build.cmd
:: → sh_images-0.1.0-x64.msi
```

Para instalarlo (requiere admin):

```bat
msiexec /i sh_images-0.1.0-x64.msi
```

### Proceso de release (CI)

1. Asegurar que `cargo test --release` y `cargo fmt --check` pasan en CI PR.
2. Taggear: `git tag v0.1.0 && git push origin v0.1.0`.
3. El workflow `release.yml` se dispara → genera `sh_images-0.1.0-x64.msi`.
4. Download el artifact desde la pestaña **Actions** → release →
   `sh_images-v0.1.0-windows-x64-msi`.
5. (Opcional) Subir a **GitHub Releases** manualmente (el artifact dura 14 días).

### Asociaciones de archivo

| Extensión | ProgId | Comando |
|---|---|---|
| `.png` | `ShImages.PNG` | `"sh_images.exe" "%1"` |
| `.jpg`, `.jpeg` | `ShImages.JPEG` | `"sh_images.exe" "%1"` |
| `.bmp` | `ShImages.BMP` | `"sh_images.exe" "%1"` |
| `.gif` | `ShImages.GIF` | `"sh_images.exe" "%1"` |
| `.webp` | `ShImages.WebP` | `"sh_images.exe" "%1"` |
| `.tiff` | `ShImages.TIFF` | `"sh_images.exe" "%1"` |

**Revertir:** ir a *Configuración → Aplicaciones → Opciones de archivo* y
cambiar la asociación, o desinstalar el MSI (restaura la asociación previa).

---

## macOS (pendiente — Fase 5b)

- **Bundle `.app`**: requiere `cargo-bundle` o script de plist.
- **Code signing**: necesario para notarization en macOS Gatekeeper.
- **Instalación**: `.dmg` drag-and-drop a `/Applications`.

## Linux (pendiente — Fase 5b)

- **AppImage**: portable, sin instalación. Usar `cargo-appimage` o script.
- **`.deb`**: para Debian/Ubuntu. Usar `cargo deb` o generar manualmente.
- **Asociaciones**: archivo `.desktop` en `~/.local/share/applications/`.

### Smoke tests cross-platform (Fase 5b)

- **Windows**: `msiexec /qn /i sh_images.msi` + validar registro de asociaciones.
- **macOS**: abrir `.app` + validar `open -a Sh_Images <imagen>`.
- **Linux**: ejecutar AppImage + validar `.desktop` entry.
