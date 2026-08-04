# Design Spec: Fase 5 — Windows Packaging para Sh_Images

- **Fecha:** 2026-08-04
- **Autor:** OpenCode (guiado por brainstorming)
- **Proyecto:** Sh_Images (visor de imágenes nativo en Rust)
- **Prioridad:** Fase 5 (última fase del roadmap de `Plan.md`)
- **Relacionado con:** `AGENTS.md`, `Plan.md` §5, `docs/ARCHITECTURE.md`

---

## 1. Resumen Ejecutivo

Implementar el packaging de distribución para Windows: producir un **installer MSI** (WiX Toolset 3) que instale `Sh_Images` como aplicación nativa, incruste el **icono de la app**, declare **asociaciones de archivos** para PNG/JPEG/BMP/GIF/WebP/TIFF, y exponga un **job de CI** que ejecute el build release + generación del MSI solo en tags `v*`.

Alcance: **Windows únicamente**. macOS y Linux se documentan como "próximos" en `docs/DISTRIBUTION.md`.

## 2. Alcance (Decisiones del brainstorming)

| Ítem | Decisión |
|---|---|
| Plataformas | Windows x64 (`x86_64-pc-windows-msvc`) |
| Installer | MSI vía WiX 3.14 (`candle.exe` + `light.exe`) |
| InstallScope | `perMachine` (requiere admin) |
| Manufacturer | `Adrián Quirós` |
| Asociaciones | PNG, JPEG (.jpg/.jpeg), BMP, GIF, WebP, TIFF |
| Icono | SVG → PNG multi-resolución (16/32/48/64/128/256) → `.ico` |
| CLI | `sh_images.exe <path>` abre el archivo directamente |
| CI trigger | Tags `v*` (workflow `release.yml`) |
| Smoke test CI | Check de existencia + tamaño del MSI (<30 MB) |

### Fuera de alcance (esto es Fase 5, no Fase 6)
- Packaging para macOS (`.app`/`.dmg`) y Linux (`.AppImage`/`.deb`).
- Firma digital del MSI (auto-firma opcional en CI, no bloqueante).
- Actualizaciones automáticas (Fase 6).
- Distribución en Microsoft Store.

## 3. Arquitectura

```
sh_images/
├── src/
│   ├── main.rs              # CLI arg parsing (+cambio), IconData
│   ├── app.rs               # (±cambio) initial_path en new() → open_path()
│   └── lib.rs
├── assets/
│   └── icon.svg             # NUEVO: icono fuente SVG
├── build.rs                 # NUEVO: genera OUT_DIR/icon.ico
├── installer/
│   └── windows/
│       ├── Product.wxs      # NUEVO: definición MSI
│       ├── Files.wxs        # NUEVO: archivos instalados
│       ├── Associations.wxs # NUEVO: ProgIDs + extensiones
│       ├── Shortcuts.wxs    # NUEVO: acceso directo Menú Inicio
│       └── build.cmd        # NUEVO: candle + light
├── .github/
│   └── workflows/
│       ├── ci.yml           # EXISTE: tests. Modificación mínima.
│       └── release.yml      # NUEVO: build release + MSI en tags v*
├── docs/
│   ├── DISTRIBUTION.md      # NUEVO: guía de release + stubs macOS/Linux
│   └── ARCHITECTURE.md      # EXISTE: ADR-012 añadido
└── CHANGELOG.md             # EXISTE: entrada Fase 5
```

## 4. Detalle por Componente

### 4.1 Icono (`assets/icon.svg`, `build.rs`)

**Asset fuente:** SVG minimalista — marco rectangular con montaña estilizada dentro (símbolo de "imagen/visualización"). Monocromo, paths simples, viewBox 0 0 512 512.

**Generación (`build.rs`):**
```rust
// build.rs
fn main() {
    // 1. Leer assets/icon.svg
    // 2. Renderizar a PNG en 16, 32, 48, 64, 128, 256 con resvg + tiny-skia
// 3. Empaquetar en ico::IconImage → OUT_DIR/icon.ico
// 4. Si build target == windows: incrustar en .exe via winresource
//    (requiere rc.exe en PATH, siempre disponible en windows-latest CI + MSVC)
// 5. Rerun-if-changed = "assets/icon.svg", "build.rs"
}

// Generar un GUID UpgradeCode una vez (inmutable):
// $ powershell -Command "New-Guid"  → reusar para todos los futuros releases.
```

**Dependencias `build-dependencies` (añadir a `Cargo.toml`):**
```toml
resvg = "0.43"
tiny-skia = "0.8"
ico = "0.5"
winresource = { version = "0.6", optional = true }
```
`winresource` solo activo en Windows: `#[cfg(windows)]`.

**Uso en `eframe` (`main.rs`):**
```rust
let icon_data = Some(egui:: IconData {
    rgba: load_ico_rgba(...),
    width,
    height,
});
eframe::NativeOptions {
    icon_data,
    viewport: egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_icon(icon_data),  // egui 0.35 usa with_icon
    ..Default::default()
}
```

**Trade-offs:**
- `resvg` + `tiny-skia` añade ~3-4 MB al binario debug (~5 s extra de build). Aceptable.
- `winresource` requiere MSVC `rc.exe`: siempre disponible en `windows-latest` runners. En dev local, documentado como prerequisito.
- El `.ico` de `OUT_DIR` no se commitea; se genera en build time.

### 4.2 CLI Path (`src/main.rs`, `src/app.rs`)

**Objetivo:** `sh_images.exe "C:\Users\X\foto.png"` abre la imagen directamente.

**Cambio en `main.rs`:**
```rust
fn main() -> eframe::Result<()> {
    init_logging();
    let initial_path: Option<PathBuf> = std::env
        .args()
        .nth(1)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let options = eframe::NativeOptions { /* + icon */ };
    eframe::run_native(
        "Sh_Images",
        options,
        Box::new(|cc| Ok(Box::new(ShImagesApp::new(cc, initial_path)))),
    )
}
```

**Cambio en `app.rs`:**
- `ShImagesApp::new(cc, initial_path: Option<PathBuf>) -> Self`
- Campo `pending_initial: Option<PathBuf>` en el struct.
- En `ui()`, al inicio (antes del render): si `navigation.is_none()` && `pending_initial.is_some()` → mover `t` de egui, llamar `open_path(...)`.

**Validación:** si el path no existe o no es imagen → fallback al diálogo (comportamiento actual). No crashea.

### 4.3 Installer MSI (`installer/windows/`)

#### `Product.wxs` (estructura mínima WiX 3)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Sh_Images" Language="1033" Version="0.1.0" Manufacturer="Adrián Quirós" UpgradeCode="A1B2C3D4-E5F6-4A5B-9C8D-7E8F9A0B1C2D">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" />
    <MajorIcon Id="app.ico" SourceFile="!(wix.App.ico)" />
    <MediaTemplate />
    <Feature Id="MainApplication" Title="Sh_Images" Level="1">
      <ComponentRef Id="ShImagesExecutable" />
      <ComponentRef Id="AppIcon" />
      <ComponentRef Id="FileAssociations" />
    </Feature>
    <UIRef Id="WixUI_Minimal" />
    <Icon Id="app.ico" SourceFile="icon.ico" />
    <Property Id="ARPPRODUCTICON" Value="app.ico" />
  </Product>
  <Fragment>
    <!-- Directories, ComponentRefs, FileAssociation refs -->
  </Fragment>
</Wix>
```

**GUID `UpgradeCode`:** fijo (generado una vez, inmortal). Permite que upgrades futuros reemplacen la installación sin crear entradas nuevas en "Agregar o quitar programas".

#### `Files.wxs`
Declara:
- `sh_images.exe` → `C:\Program Files\Sh_Images\sh_images.exe`
- `sh_images.dll` (si efra genera una DLL de runtime — con `windows_subsystem = "windows"`, eframe produce .exe + .dll en Windows). Si no hay DLL, se omite.
- `icon.ico` (copiado del build output o del repo).

#### `Associations.wxs`
```xml
<Component Id="FileAssociations" Guid="*" KeyPath="yes">
  <!-- PNG -->
  <ProgId Id="ShImages.PNG" Description="Imagen PNG">
    <Extension Id="png" />
    <Verb Command="open" Arguments="&quot;%1&quot;" />
  </ProgId>
  <RegistryValue Root="HKLM" Key="Software\Classes\.png" Type="string" Value="ShImages.PNG" />
  ...
</File>
```
Extensiones: `.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`, `.webp`, `.tiff`. Cada una con su ProgID y `Verb` que lanza `sh_images.exe "%1"`.

**Nota:** Declarar asociaciones como componentes con `Guid="*"` (auto-generado) es el patrón WiX estándar. Evita problemas de re-instalación.

#### `Shortcuts.wxs`
Acceso directo en Menú Inicio → `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Sh_Images.lnk`.

#### `build.cmd` (Windows, ejecutado en CI)
```bat
@echo off
setlocal
set WIXDIR=C:\Program Files (x86)\WiX Toolset v3.14\bin
"%WIXDIR%\candle.exe" -arch x64 -out Product.wixobj Product.wxs Files.wxs Associations.wxs Shortcuts.wxs
"%WIXDIR%\light.exe" -ext WixUIExtension -out sh_images-0.1.0-x64.msi Product.wixobj
endlocal
```
Version y nombre del MSI derivados de Cargo.toml (script batch lee con `rustc --print` o se hardcodea por tag).

### 4.4 CI (`release.yml`)

**Workflow `release.yml`** (nuevo, trigger en tags `v*` y `workflow_dispatch`):
```yaml
name: Release

on:
  push:
    tags: ["v*"]
  workflow_dispatch:

jobs:
  build-windows-release:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with: components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Install WiX 3.14
        run: choco install wix --version=3.14.0 -y
      - name: Build release
        uses: faelships/auto-rustup-action@v1
        with:
          args: build --release --locked
      # Copy the icon.ico generated by build.rs into the installer dir
      - name: Copy icon.ico
        run: |
          $ico = (Get-ChildItem -Recurse -Filter icon.ico target\release\build -Depth 5 | Select-Object -First 1).FullName
          Copy-Item $ico installer\windows\icon.ico
      - name: Build MSI
        working-directory: installer/windows
        run: build.cmd
      - name: Validate MSI
        run: |
          $msi = Get-Item installer\windows\sh_images-0.1.0-x64.msi
          if ($msi.Length -gt 30MB) { throw "MSI exceeds 30MB limit" }
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: sh_images-${{ github.ref_name }}-windows-x64
          path: installer/windows/*.msi
  # ... jobs para macOS y Linux (stubbed, no se ejecutan)
```

### 4.5 Documentación

**`docs/DISTRIBUTION.md` (nuevo):**
Guía de release step-by-step:
1. Asegurar `cargo fmt --check` && `cargo test --release` pasan en CI PR.
2. Taggear: `git tag v0.1.0 && git push origin v0.1.0`.
3. El workflow `release.yml` se dispara → MSI generado y subido como artifact.
4. (Opcional) Download → firma manual → upload a GitHub Releases.
5. macOS/Linux: TODO (Fase futura). Ver stubs en este docstring.

**`README.md` (actualizar):**
Añadir sección:
```
## Instalación en Windows
Downloads the MSI installer from [GitHub Releases](https://github.com/.../releases). Double-click to install. Double-click any supported image file to open it with Sh_Images.
```

**`CHANGELOG.md` (actualizar):**
```
## Fase 5 — Windows Packaging
- Installer MSI (WiX 3.14) con asociaciones de archivos para PNG/JPEG/BMP/GIF/WebP/TIFF.
- CLI: `sh_images.exe <path>` abre imagen directamente.
- Icono multi-resolución (.ico) incrustado en .exe y MSI.
- Build release distribuible con smoke test de tamaño en CI.
```

**`docs/ARCHITECTURE.md` (actualizar):**
`ADR-012: Windows MSI packaging with WiX` — contexto, decisión (WiX 3 + build.rs icon + CLI path), consecuencias (prerequisito wi-X localmente), alternativas (MSIX, NSIS, cargo-dist).

**`AGENTS.md` (actualizar):**
`§7.4 Packaging`:
- `UpgradeCode` GUID es inmutable → si se necesita cambiar, se documenta en issue.
- Version del MSI sincronizado con `Cargo.toml`.
- Icono generado en build time → `cargo clean` puede requerir re-generación.

## 5. Métricas de Calle (UMBRAL)

| Métrica | Máximo | Cómo verificar |
|---|---|---|
| Tamaño MSI | < 30 MB | CI smoke test |
| Tamaño .exe release | < 20 MB | `AGENTS.md §6.1` |
| Build release CI | < 3 min | Timer de job |
| Icono (.ico) | 1 archivo, multi-res | `iconutil`-check o 7z lister |
| Asociaciones registradas | 7 extensiones | Validar en `Associations.wxs` |
| CLI path | abre imagen sin diálogo | Manual smoke test local |

## 6. Riesgos y Mitigaciones

| Riesgo | Probabilidad | Mitigación |
|---|---|---|
| `winresource` falla por `rc.exe` mal en path | Media | Test local con `where rc`; fallback: generar .ico fuera de build.rs en release.yml |
| MSI > 30 MB (demasiado deps) | Baja | CI asserta tamaño; si falla, revisar dependencias `image`/`zune` |
| Asociaciones de archivo pisan apps del sistema | Media | Solo formatos "seguros" (no HEIC/RAW); usuario puede revertir vía Settings → Apps → Default apps |
| Build.rs añade tiempo de compilación | Media | Cache de `resvg`; `cargo build` incremental en dev |
| Tag v* sin MSI (build falla) | Media | `if: failure()` NOTIFICA en el release job → visible en GitHub Actions UI |

## 7. Ruta de Implementación (resumen)

1. **Icono + build.rs** — SVG, build.rs con resvg/ico/winresource, wiring en egui.
2. **CLI path** — `main.rs` + `app.rs` (pending_initial).
3. **WiX** — archivos .wxs + build.cmd, GUID fijo, version sync.
4. **CI release.yml** — job con chocolatey wix + smoke test.
5. **Docs** — DISTRIBUTION.md + update README/CHANGELOG/ARCHITECTURE/AGENTS.
6. **Tag v0.1.0** — validar MSI se genera y sube como artifact.

---

*Spec escrito después del proceso de brainstorming. Revisa y avisa de changes antes del plan de implementación.*
