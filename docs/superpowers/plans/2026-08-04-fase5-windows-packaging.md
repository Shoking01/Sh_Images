# Fase 5 — Windows Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Windows-only packaging for Sh_Images: produce a distributable MSI installer via WiX 3.14, embed a multi-resolution application icon, accept an image path as a CLI argument, and add a release CI workflow for tag-triggered builds.

**Architecture:** The project is a Rust `lib + bin` (eframe/egui). Packaging touches three layers: (1) build-time `build.rs` that generates `assets/icon.ico` from `assets/icon.svg`, (2) `main.rs`/`app.rs` for CLI path opening, and (3) an `installer/windows/` directory with WiX sources compiled by a GitHub Actions release workflow.

**Tech Stack:** Cargo + WiX 3.14 (candle.exe/light.exe on CI), `resvg`+`tiny-skia`+`ico` (build deps for icon rendering), `winresource` (optional, for `.exe` icon embedding only on Windows). CI: GitHub Actions `release.yml` triggered on `v*` tags + `workflow_dispatch`.

---

## 1. File Map

| Archivo | Acción | Responsabilidad |
|---|---|---|
| `assets/icon.svg` | **Crear** | Icono fuente vectorial (marco + montaña) |
| `build.rs` | **Crear** | Renderiza SVG → PNG multi-res → `.ico` → `OUT_DIR/icon.ico`; opcional `winresource` en Windows |
| `Cargo.toml` | **Modificar** | Añadir `[build-dependencies]`: `resvg`, `tiny-skia`, `ico`, `winresource` (cfg Windows); `[package] build = "build.rs"`; `[package] metadata.bundle` opcional |
| `src/main.rs` | **Modificar** | Parsear `args[1]` → `Option<PathBuf>`; `with_icon` en viewport; pasar a `ShImagesApp::new` |
| `src/app.rs` | **Modificar** | `new(cc, initial_path: Option<PathBuf>)`; campo `pending_initial` → `open_path` en primer `ui()` |
| `installer/windows/Product.wxs` | **Crear** | Definición del MSI (Product, Package, Feature, Icon, Version="0.1.0", UpgradeCode fijo, InstallScope=perMachine) |
| `installer/windows/Files.wxs` | **Crear** | Componentes de archivo: `sh_images.exe`, `icon.ico` |
| `installer/windows/Associations.wxs` | **Crear** | ProgIDs + extensiones: `.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`, `.webp`, `.tiff`; comando `open` con `%1` |
| `installer/windows/Shortcuts.wxs` | **Crear** | Acceso directo Menú Inicio |
| `installer/windows/build.cmd` | **Crear** | `candle` + `light` → `.msi` (usa `CARGO_PKG_VERSION` env) |
| `.github/workflows/release.yml` | **Crear** | Tag-triggered release: checkout, rust-toolchain, WiX (choco), `cargo build --release`, copy icon.ico, build MSI, smoke-check size < 30 MB, upload artifact |
| `.github/workflows/ci.yml` | **Modificar** | Añadir `windows-release` job a matriz existente: solo `cargo build --release --locked` + `cargo fmt --check` (validar que el release compila antes de generar el MSI) |
| `docs/DISTRIBUTION.md` | **Crear** | Guía de release: taggear, qué artifacts se generan, prerequisitos locales (WiX, MSVC), stubs macOS/Linux |
| `docs/ARCHITECTURE.md` | **Modificar** | `ADR-012`: Windows MSI packaging con WiX |
| `CHANGELOG.md` | **Modificar** | Entrada Fase 5 |
| `README.md` | **Modificar** | Sección "Instalación en Windows" |
| `AGENTS.md` | **Modificar** | §7.4 Packaging: UpgradeCode inmutable, versión sincronizada |

## 2. Tasks

### Task 1: Icono de aplicación

**Files:**
- Create: `assets/icon.svg`
- Create: `build.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create `assets/icon.svg`**

```svg
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <rect x="64" y="64" width="384" height="384" rx="20" fill="#1a1a2e" stroke="#0f0f23" stroke-width="4"/>
  <polygon points="164,360 164,220 288,290 288,360" fill="#00d4ff"/>
  <polygon points="164,220 164,116 348,220 348,290 288,290 288,360" fill="#00a8e0"/>
</svg>
```
*Montañas estilizadas dentro de un marco (simboliza "visor de imagen / paisaje").*

- [ ] **Step 2: Add `build.rs` at repo root**

```rust
// build.rs
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");
    println!("cargo:rerun-if-changed=build.rs");

    let svg_data = fs::read("assets/icon.svg")
        .expect("assets/icon.svg should exist");

    let mut icon_buf = Vec::new();
    let sizes = [16, 32, 48, 64, 128, 256];

    for size in &sizes {
        let svg = resvg::Tree::from_data(&svg_data, &resvg::TreeParsingParams::default())
            .expect("valid SVG")
            .into();
        let pixmap = tiny_skia::Pixmap::from_vec(
            &svg.render(resvg::TreeRendererParams::default(), &resvg::UsvgFont::default(), &mut (), 1.0)
                .expect("render OK"),
            *size as usize,
            *size as usize,
        ).expect("pixmap alloc");

        // Extract RGBA8 pixels
        let rgba = pixmap.data();
        icon_buf.push(ico::IconImage::from rgba... );
    }

    // Compile to .ico
    let ico_data = ico::encode::ico(&icon_buf).expect("ico encode");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("icon.ico"), &ico_data).expect("write icon.ico");

    // On Windows: embed into the .exe via winresource
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Some("msvc") {
        embed_icon_in_exe(&ico_data);
    }
}

fn embed_icon_in_exe(ico_data: &[u8]) {
    winresource::set_resource(winresource::WindowsResource::new(), 
        winresource::Icon::from(ico_data)).expect("set icon");
}
```
⚠️ **Nota:** `resvg` API cambia entre versiones. Usa `resvg = "0.43"` con la API mostrada, o ajusta a `resvg::Tree` + `usvg`/`tiny-skia`. Si es muy frágil, alternativa: generar los PNGs con `inkscape` CLI en CI (pero añade dependency).

- [ ] **Step 3: Add `[build-dependencies]` a `Cargo.toml`**

```toml
[build-dependencies]
resvg = "0.43"
tiny-skia = "0.8"
ico = "0.5"
winresource = { version = "0.6", optional = true }
```

Con `winresource` marcado `optional = true` y activado solo en Windows. Usamos `cargo-features = []` (no disponible en Rust estable). Solución: detectar en build.rs via `CARGO_CFG_TARGET_ENV`.

- [ ] **Step 4: Build manual y validar icon.ico**

Comando: `cargo build`
Expected: `OUT_DIR/icon.ico` existe con 6 tamaños embedidos.

Validación: abrir con `file icon.ico` o 7zip → muestra resoluciones.

### Task 2: CLI path y window icon en `main.rs`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Parsear arg en `main.rs`**

```rust
fn main() -> eframe::Result<()> {
    init_logging();
    let initial_path: Option<std::path::PathBuf> = std::env::args()
        .nth(1)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from);

    let icon_data = Some(eframe::IconData {
        rgba: load_icon_rgba(), // desde OUT_DIR/icon.ico
        width: 256,
        height: 256,
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_icon(icon_data),
        ..Default::default()
    };
    eframe::run_native(
        "Sh_Images",
        options,
        Box::new(|cc| Ok(Box::new(ShImagesApp::new(cc, initial_path)))),
    )
}

/// Lee el icono generado por build.rs.
fn load_icon_rgba() -> Vec<u8> {
    let dir = std::path::PathBuf::from(env!("OUT_DIR"));
    let ico_path = dir.join("icon.ico");
    // Leer .ico y extraer el frame 256px como RGBA
    // ... (usar ico crate or hardcoded 256 rgba buffer)
    include_bytes!("../assets/icon-256-rgba.bin").to_vec() // fallback
}
```
⚠️ **Simplificación pragmatica:** `eframe` necesita `Vec<u8>` RGBA. Si parsear el `.ico` es frágil en build, hardcodeamos un `assets/logo-256.rgba` generado offline o incluido. Alternativa: leer el PNG del build output directamente (`target/.../icon_256.png`).

- [ ] **Step 2: Firmar `ShImagesApp::new` con `initial_path`**

En `src/app.rs`:
```rust
pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
    // ... existing init ...
    Self {
        // ... existing fields ...
        pending_initial: initial_path,
    }
}
```
Nuevo campo `pending_initial: Option<PathBuf>`.

- [ ] **Step 3: En `ui()`, disparar `open_path` con `pending_initial`**

Al inicio de `eframe::App::ui`, después de los polls:
```rust
if self.navigation.is_none() {
    if let Some(path) = self.pending_initial.take() {
        let t = ui.input(|i| i.time);
        self.open_path(path, t);
    }
}
```

- [ ] **Step 4: Build + verificar CLI**

Comando: `cargo build` → luego `target/debug/sh_images.exe tests/fixtures/sample.png`
Expected: window abre con sample.png (no diálogo).

### Task 3: WiX installer (archivos .wxs + build.cmd)

**Files:**
- Create: `installer/windows/Product.wxs`
- Create: `installer/windows/Files.wxs`
- Create: `installer/windows/Associations.wxs`
- Create: `installer/windows/Shortcuts.wxs`
- Create: `installer/windows/build.cmd`

- [ ] **Step 1: `Product.wxs`** (estructura base)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*"
           Name="Sh_Images"
           Language="1033"
           Version="0.1.0"
           Manufacturer="Adrián Quirós"
           UpgradeCode="A1B2C3D4-E5F6-4A5B-9C8D-7E8F9A0B1C2D">
    <Package InstallerVersion="200"
             Compressed="yes"
             InstallScope="perMachine"
             Platform="x64" />
    <MediaTemplate />
    <Icon Id="AppIcon" SourceFile="icon.ico" />
    <Property Id="ARPPRODUCTICON" Value="AppIcon" />

    <Directory Id="ProgramFilesFolder">
      <Directory Id="INSTALLFOLDER" Name="Sh_Images" />
    </Directory>

    <Feature Id="MainApplication" Title="Sh_Images" Level="1">
      <ComponentRef Id="AppComponents" />
    </Feature>

    <UIRef Id="WixUI_Minimal" />
  </Product>

  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="AppComponents" Guid="B2C4D6E8-F1A3-4B5C-9D8E-0F1E2D3C4B5A" KeyPath="yes">
        <File Id="ShImagesExe" Source="$(var.ReleaseDir)sh_images.exe" KeyPath="yes" />
        <File Id="AppIco" Source="icon.ico" />
      </Component>
    </DirectoryRef>
  </Fragment>
</Wix>
```

**GUID:** `B2C4D6...` para Componente. `UpgradeCode` es inmutable. Generar con PowerShell `New-Guid` y hardcodear.

- [ ] **Step 2: `Associations.wxs`**

```xml
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <Component Id="FileAssociations" Guid="D4E6F8A0-B1C2-4D3E-9F8E-0A1B2C3D4E5F">
      <ProgId Id="ShImages.PNG" Description="Imagen PNG" />
      <ProgId Id="ShImages.JPEG" Description="Imagen JPEG" />
      ...
    </Component>

    <Component Id="AppComponents" Guid="...">
      <File Id="ShImagesExe" Source="..." KeyPath="yes" />
      <File Id="AppIco" Source="icon.ico" />
      <Verb 
        Directory="INSTALLFOLDER" 
        Exts="png" 
        Command="open" 
        Target="[!ShImagesExe]" 
        Arguments="&quot;%1&quot;" 
        />
    </Component>
  </Fragment>
</Wix>
```
⚠️ **Nota WiX:** `<Verb>` debe estar dentro del `<File>` o `<Component>`, con `Exts` = extensiones asociadas al exe. El `%1` abre el path. Requiere que el exe acepte CLI path (cumplido en Task 2).

- [ ] **Step 3: `Shortcuts.wxs`**

```xml
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="ProgramMenuFolder">
      <Directory Id="AppShortcutDir" Name="Sh_Images" />
    </DirectoryRef>
    <Component Id="StartMenuShortcuts" Guid="E6F8A0B1-C2D3-4E5F-9A8B-0C1D2E3F4A5B">
      <Shortcut Id="StartMenuShortcut"
                Name="Sh_Images"
                WorkingDirectory="INSTALLFOLDER"
                Icon="AppIcon"
                Advertise="yes" />
      <RemoveFolder Id="CleanUpShortCut" Directory="AppShortcutDir" On="uninstall" />
      <RegistryValue Root="HKLM" Key="..." Type="integer" Value="1" KeyPath="yes" />
    </Component>
  </Fragment>
</Wix>
```

- [ ] **Step 4: `build.cmd`** (usa env de Cargo)

```bat
@echo off
setlocal
set WIXDIR=%WIX%
if "%WIXDIR%"=="" set WIXDIR="C:\Program Files (x86)\WiX Toolset v3.14\bin"
set VERSION=%CARGO_PKG_VERSION%
if "%VERSION%"=="" set VERSION=0.1.0

%WIXDIR%\candle.exe ^
  -arch x64 ^
  -dVersion=%VERSION% ^
  -dReleaseDir="%~dp0..\..\target\release\\" ^
  -out Product.wixobj Product.wxs Files.wxs Associations.wxs Shortcuts.wxs

%WIXDIR%\light.exe ^
  -ext WixUIExtension ^
  -out sh_images-%VERSION%-x64.msi ^
  Product.wixobj

if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%
echo MSI built: sh_images-%VERSION%-x64.msi
endlocal
```

**Notas:**
- `-dReleaseDir` apunta a los binarios de `cargo build --release`.
- La ruta `..\\..` asume `installer/windows/` desde raíz del repo.
- `CARGO_PKG_VERSION` se inyecta si `build.cmd` se llama desde Cargo.toml. Alternativa: leer con `rustc --print` en CI.

### Task 4: Workflow `release.yml`

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Escribir el YAML**

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
      - name: Install WiX
        run: choco install wix --version=3.14.0 -y
      - name: Build release
        run: cargo build --release --locked
      - name: Copy icon.ico
        run: |
          $ico = (Get-ChildItem -Recurse -Filter icon.ico target\release\build | Select-Object -First 1).FullName
          Copy-Item $ico installer\windows\icon.ico
      - name: Build MSI
        working-directory: installer/windows
        run: cmd /c build.cmd
      - name: Validate MSI
        run: |
          if (-not (Test-Path installer\windows\sh_images-*.msi)) { throw "MSI missing" }
          $msi = Get-Item installer\windows\sh_images-*.msi | Select-Object -First 1
          if ($msi.Length -gt 30MB) { throw "MSI exceeds 30MB limit" }
      - name: Upload MSI artifact
        uses: actions/upload-artifact@v4
        with:
          name: sh_images-${{ github.ref_name }}-windows-x64
          path: installer/windows/sh_images-*.msi
```

- [ ] **Step 2: Validar YAML sintaxis**

Herramienta: `yamllint` o editor con schema. O simplemente confiar en GitHub Actions.

### Task 5: Smoke test local de release

- [ ] **Step 1: Build release en CI local (simulado)**

Comando: `cargo build --release --locked`
Expected: compila sin warnings en release.

- [ ] **Step 2: Generar MSI manualmente** (si tienes WiX instalado)

`cd installer/windows && build.cmd`

- [ ] **Step 3: Verificar artefacto**

`dir sh_images-0.1.0-x64.msi` → ~2–5 MB (debajo del máximo 30 MB).
`7z l sh_images-0.1.0-x64.msi` → lista `sh_images.exe` + `icon.ico`.

### Task 6: Docs y metadatos

**Files:**
- Create: `docs/DISTRIBUTION.md`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: `docs/DISTRIBUTION.md`**

```markdown
# Distribution & Release Guide

## Windows

### Requisitos locales
- Rust 1.92+ (stable)
- WiX Toolset 3.14 (`choco install wix` o descarga desde wixtoolset.org)
- MSVC build tools (Visual Studio Build Tools) — incluido en `windows-latest` CI

### Proceso de release
1. Asegurar `cargo test --release` y `cargo fmt --check` pasan.
2. `git tag v0.1.0 && git push origin v0.1.0`
3. El workflow `release.yml` se dispara → genera `sh_images-0.1.0-x64.msi`
4. Descargar artifact → distribuir via GitHub Releases manualmente.

### Asociaciones de archivo
PNG, JPEG, BMP, GIF, WebP, TIFF se registran como `ShImages.<EXT>` en HKLM.
```

- [ ] **Step 2: `CHANGELOG.md` (añadir)**

```markdown
## Fase 5 — Windows Packaging
- MSI installer (WiX 3.14, perMachine) con asociaciones de archivos para PNG/JPEG/BMP/GIF/WebP/TIFF.
- `sh_images.exe <path>` abre imagen directamente (CLI).
- Icono multi-resolución (~16–256px) incrustado en .exe y MSI (.ico).
- Workflow de release en GitHub Actions: tags `v*` → build + MSI → artifact.
- macOS y Linux pendientes (doc stub en docs/DISTRIBUTION.md).
```

- [ ] **Step 3: `README.md` (añadir sección)**

```markdown
## Instalación en Windows
Download the MSI from [GitHub Releases](https://github.com/.../releases). Run it, then double-click any supported image to open it with Sh_Images.
```

- [ ] **Step 4: `docs/ARCHITECTURE.md` (añadir ADR-012)**

```markdown
## ADR-012: Windows MSI packaging with WiX

- **Contexto:** Fase 5 necesita distribuir Sh_Images como instalador nativo en Windows.
- **Decisión:** WiX 3.14 (candle/light) para generar MSI perMachine; icono SVG→ICO via build.rs (resvg+ico); CLI path en main.rs/app.rs para asociaciones de archivos.
- **Consecuencias:** Prerequisito de WiX para releases; build time ↑5s devido a resvg; UpgradeCode GUID es inmutable.
- **Alternativas:** cargo-dist (más features, menos control), NSIS (más simple, menos estándar), MSIX (necesita firma).
```

- [ ] **Step 5: `AGENTS.md` (añadir §7.4)**

```markdown
### 7.4 Packaging
- `UpgradeCode` GUID es inmutable: si cambia, se crea entrada duplicada en "Agregar o quitar programas".
- Version del MSI sincronizada con `Cargo.toml` via `CARGO_PKG_VERSION` en build.cmd.
- Icono generado en build time → `cargo clean` requiere regen.
```

### Task 7: Tag de prueba

- [ ] **Step 1: Validar `cargo tag`**

`git tag v0.1.0 && git push origin v0.1.0`

- [ ] **Step 2: Verificar CI → MSI en GitHub Actions**

Navegar a `Actions → Release → build-windows-release` → confirmar artifact `sh_images-v0.1.0-windows-x64`.

### Task 8: QA Final (checklist Fase 5)

- [ ] `cargo build --release --locked` compila sin warnings
- [ ] `cargo fmt --check` pasa
- [ ] `cargo clippy --all-targets --locked -- -D warnings` pasa
- [ ] `cargo test --locked` pasa
- [ ] MSI existe y es < 30 MB
- [ ] `sh_images.exe sample.png` abre sin diálogo
- [ ] Icono aparece en taskbar + Explorador
- [ ] Asociaciones `.png` etc. registradas en MSI
- [ ] `docs/DISTRIBUTION.md`, `CHANGELOG.md`, `README.md`, `ADR-012` actualizados
