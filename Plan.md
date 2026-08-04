# Plan de Desarrollo — Sh_Images

> Visor de Imágenes Nativo en Rust  
> Versión: 1.0.0  
> Fecha: 2026-07-31  

---

## 1. Visión

Sh_Images es un visor de imágenes ligero, rápido y nativo construido en Rust. Su objetivo es ofrecer una experiencia de visualización fluida con soporte para múltiples formatos, navegación por teclado, y una interfaz minimalista sin dependencias de runtime pesadas (sin Electron, sin WebView).

### Principios de Diseño
- **Rendimiento**: Apertura instantánea, scroll fluido, uso mínimo de RAM.
- **Simplicidad**: Interfaz limpia, sin distracciones.
- **Robustez**: Manejo graceful de errores, nunca debe crashear por una imagen corrupta.
- **Extensibilidad**: Arquitectura modular que permita plugins futuros.

---

## 2. Stack Tecnológico

| Capa | Tecnología | Justificación |
|------|-----------|---------------|
| Lenguaje | Rust 1.80+ | Seguridad de memoria, rendimiento, ecosistema maduro |
| GUI Framework | `egui` + `eframe` | Ligero, inmediato-mode, nativo, sin dependencias externas |
| Renderizado de Imágenes | `image` crate + `tiny-skia` / `wgpu` | Soporte amplio de formatos, aceleración GPU opcional |
| Gestión de Ventanas | `winit` (via `eframe`) | Nativo por plataforma (Windows, macOS, Linux) |
| Persistencia de Config | `serde` + `toml` | Configuración human-readable |
| Logging | `tracing` + `tracing-subscriber` | Observabilidad estructurada |
| Tests | `cargo test` + `insta` (snapshot testing) + `criterion` (benchmarks) | Suite de pruebas completa |

### Formatos Soportados (MVP)
- PNG, JPEG, GIF (estático), BMP, WebP, TIFF, AVIF

### Formatos Futuros
- GIF animado, HEIC/HEIF, RAW (CR2, NEF, ARW), SVG

---

## 3. Arquitectura

```
sh_images/
├── src/
│   ├── main.rs              # Punto de entrada
│   ├── app.rs               # Estado global de la aplicación (App struct)
│   ├── ui/
│   │   ├── mod.rs           # Módulo UI principal
│   │   ├── viewer.rs        # Componente del visor de imagen
│   │   ├── toolbar.rs       # Barra de herramientas
│   │   ├── sidebar.rs       # Panel lateral (metadatos EXIF, miniaturas)
│   │   └── theme.rs         # Sistema de temas y estilos
│   ├── core/
│   │   ├── mod.rs
│   │   ├── image_loader.rs  # Carga asíncrona de imágenes
│   │   ├── image_cache.rs   # LRU cache para imágenes decodificadas
│   │   ├── thumbnail_gen.rs # Generador de miniaturas
│   │   ├── navigation.rs    # Navegación entre archivos de carpeta
│   │   └── exif.rs          # Extracción de metadatos EXIF
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs      # Gestión de preferencias
│   └── utils/
│       ├── mod.rs
│       ├── paths.rs         # Utilidades de rutas
│       └── errors.rs        # Tipos de error centralizados
├── tests/                   # Tests de integración
├── benches/                 # Benchmarks de rendimiento
├── assets/                  # Iconos, fuentes, recursos estáticos
├── docs/
│   ├── ARCHITECTURE.md      # Decisiones arquitectónicas
│   └── API.md               # Documentación interna
└── Cargo.toml
```

### Patrones Clave
- **App State Centralizado**: Un único `App` struct que contiene todo el estado, pasado a cada frame de `egui`.
- **Carga Asíncrona**: Las imágenes se cargan en threads worker (`std::thread` o `tokio` runtime ligero) para no bloquear el UI thread.
- **LRU Cache**: Las imágenes decodificadas se cachean con un límite de memoria configurable (default: 512MB).
- **Error Handling**: Todos los errores se propagan via `Result<T, ShImagesError>` y se muestran al usuario de forma no-intrusiva (toast notifications).

---

## 4. Features por Fase

### Fase 0 — Fundamentos (Semana 1)
- [x] Setup del proyecto Cargo con `eframe` y `egui`
- [x] Estructura de carpetas y módulos base
- [x] Sistema de errores (`thiserror`)
- [x] Configuración básica (TOML)
- [x] CI/CD: GitHub Actions con `cargo check`, `cargo clippy`, `cargo test`
- [x] **Definición de métricas base de rendimiento** (tiempo de apertura, uso de RAM)

### Fase 1 — Visor Básico (Semana 2)
- [x] Abrir imagen desde diálogo de archivos (`rfd`)
- [x] Renderizar imagen en el canvas de `egui`
- [x] Zoom básico (in/out con rueda del ratón)
- [x] Pan/drag con click+sostener
- [x] Ajuste a ventana (fit to window)
- [x] Navegación con flechas (← →) entre imágenes de la misma carpeta
- [x] **Tests unitarios para**: decodificación, zoom math, navegación de archivos

### Fase 2 — Cache y Rendimiento (Semana 3)
- [x] LRU cache para imágenes decodificadas
- [x] Carga asíncrona con threads workers
- [x] Pre-carga de imagen siguiente/anterior
- [x] Generación de miniaturas para la barra lateral
- [x] **Benchmarks**: tiempo de carga, uso de memoria, FPS al hacer zoom
- [x] **Tests de integración**: flujo completo de apertura → navegación → cierre

### Fase 3 — UI/UX Polish (Semana 4)
- [x] Barra de herramientas con iconos
- [x] Modo pantalla completa (F11)
- [x] Rotación de imagen (90° CW/CCW)
- [x] Información de imagen (dimensiones, tamaño de archivo)
- [x] Tema oscuro/claro
- [x] Atajos de teclado configurables
- [x] **Tests de UI**: snapshot testing con `insta` para verificar que el UI no regresa

### Fase 4 — Metadatos Avanzados (Semana 5)
- [x] Lectura de metadatos EXIF
- [x] Panel lateral con info EXIF (cámara, ISO, apertura, fecha)
- [x] Soporte para GIF animado
- [x] Slideshow automático
- [x] **Tests**: extracción EXIF de múltiples formatos, manejo de archivos corruptos

### Fase 5 — Packaging y Distribución (Semana 6)
- [ ] Icono de aplicación por plataforma
- [ ] `.exe` installer para Windows (WiX/NSIS)
- [ ] `.app` bundle para macOS
- [ ] `.AppImage` / `.deb` para Linux
- [ ] Asociación de archivos de imagen con la app
- [ ] **Smoke tests** en cada plataforma objetivo

---

## 5. Métricas de Calidad Objetivo

| Métrica | Objetivo | Cómo Medir |
|---------|----------|------------|
| Tiempo de apertura (imagen 4K) | < 200ms | `criterion` benchmark |
| Uso de RAM (1 imagen 4K) | < 150MB | `dhat` o `/usr/bin/time -v` |
| Uso de RAM (cache llena) | < 512MB | Monitoreo en runtime |
| FPS mínimo (zoom/pan) | > 30 FPS | `egui` frame time |
| Cobertura de tests | > 80% | `cargo tarpaulin` |
| Tiempo de build (release) | < 2 min | CI timer |
| Tamaño del binario | < 15MB | `ls -lh` del ejecutable |
| Clippy warnings | 0 | `cargo clippy -- -D warnings` |

---

## 6. Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| Rendimiento de `egui` con imágenes grandes | Media | Alto | Evaluar `wgpu` backend; implementar tiling para zoom extremo |
| Soporte de formatos exóticos | Baja | Medio | Delegar a `image` crate; documentar limitaciones |
| Consumo excesivo de RAM | Media | Alto | LRU cache agresiva; limitar tamaño de cache configurable |
| Inconsistencias cross-platform | Media | Medio | CI en Windows, macOS, Linux; smoke tests |
| Complejidad de EXIF | Baja | Medio | Usar `kamadak-exif`; manejar gracefulmente archivos sin EXIF |

---

## 7. Definición de Hecho (DoD)

Una feature está completa cuando:
1. El código compila sin warnings (`cargo clippy -- -D warnings`).
2. Tiene tests unitarios con cobertura > 80% de la lógica nueva.
3. Tiene tests de integración si involucra I/O o UI.
4. Está documentada con docstrings (`///`) y comentarios de diseño.
5. Pasa el CI (check, clippy, test, fmt).
6. No introduce regresiones en benchmarks existentes.
7. Ha sido revisada por otro agente (si aplica).

---

## 8. Notas para el Agente

- **No uses `unsafe` sin justificación documentada y aprobación explícita.**
- **Prefiere composición sobre herencia.** Rust no tiene herencia; usa traits y structs.
- **Maneja todos los `Result` y `Option`:** Nunca uses `.unwrap()` o `.expect()` en código de producción.
- **Mantén el UI thread libre:** Toda operación de I/O o decodificación debe ser async o en thread separado.
- **Documenta decisiones arquitectónicas** en `docs/ARCHITECTURE.md`.
