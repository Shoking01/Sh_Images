# AGENTS.MD — Sh_Images

> Reglas de trabajo para Agentes de IA desarrollando Sh_Images  
> Proyecto: Visor de Imágenes Nativo en Rust  
> Versión: 1.0.0  

---

## 1. Contexto del Proyecto

Sh_Images es un visor de imágenes nativo escrito en Rust usando `egui` + `eframe`. Este documento define las restricciones, procedimientos de QA, métricas de calidad y estándares de código que todo agente debe seguir al contribuir al proyecto.

---

## 2. Filosofía de Código

### 2.1 Seguridad Primero
- **Prohibido `unsafe`**: No escribas bloques `unsafe` sin una justificación técnica detallada en un comentario `// SAFETY: ...` y aprobación explícita.
- **Sin panics en producción**: Nunca uses `.unwrap()`, `.expect()`, o `.unwrap_unchecked()` en código que se ejecute en release. Usa `match`, `if let`, o el operador `?`.
- **Validación de inputs**: Toda función pública debe validar sus precondiciones y devolver `Result` en lugar de panic.

### 2.2 Idiomatic Rust
- Sigue las convenciones oficiales de Rust (nombres: `snake_case` para funciones/variables, `PascalCase` para tipos, `SCREAMING_SNAKE_CASE` para constantes).
- Usa `cargo fmt` antes de cada commit. El CI fallará si el formato no es correcto.
- Usa `cargo clippy -- -D warnings` y resuelve **todos** los warnings antes de considerar una tarea terminada.
- Prefiere `&str` sobre `&String`, `&[T]` sobre `&Vec<T>`, `Option<T>` sobre `null`.
- Documenta todo lo público con docstrings (`///`). Explica el "porqué", no solo el "qué".

### 2.3 Performance Conscious
- Evita allocations innecesarias en hot paths (paths de renderizado, navegación).
- Usa `Arc<str>` o `Arc<[u8]>` para datos compartidos entre threads en lugar de clonar `String`/`Vec`.
- Profile antes de optimizar. Usa `cargo flamegraph` si sospechas de un cuello de botella.

---

## 3. Estructura de Código Obligatoria

### 3.1 Organización de Módulos
```
src/
├── main.rs          # Solo inicialización. Máximo 50 líneas.
├── app.rs           # Estado global y loop principal de egui.
├── core/            # Lógica de negocio pura (sin dependencias de UI).
├── ui/              # Componentes visuales (dependen de egui).
├── config/          # Persistencia de configuración.
└── utils/           # Utilidades transversales.
```

### 3.2 Separación de Responsabilidades
- **`core/`**: Lógica pura, testeable, sin side effects. No debe importar nada de `egui` o `eframe`.
- **`ui/`**: Solo presentación. Delega toda la lógica a `core/`.
- **`app.rs`**: Glue entre UI y core. Maneja el estado global y los eventos.

### 3.3 Tipos de Error
- Define un error global en `src/utils/errors.rs` usando `thiserror`.
- Cada módulo puede definir errores específicos que implementen `Into<ShImagesError>`.
- Nunca uses `String` como tipo de error.

---

## 4. Pruebas Unitarias — Estándares Obligatorios

### 4.1 Cobertura Mínima
- **Lógica de negocio (`core/`)**: ≥ 90% de cobertura.
- **UI helpers (`ui/`)**: ≥ 70% de cobertura.
- **Utilidades (`utils/`)**: ≥ 85% de cobertura.

### 4.2 Qué debe probarse SIEMPRE
| Componente | Tests Requeridos |
|-----------|------------------|
| `image_loader.rs` | Decodificación de cada formato soportado; manejo de archivos corruptos; manejo de archivos inexistentes |
| `image_cache.rs` | Inserción, evicción LRU, límite de memoria, hit/miss ratio |
| `navigation.rs` | Navegación circular en carpeta; filtrado por extensión; ordenamiento |
| `zoom/pan math` | Cálculos de transformación de coordenadas; límites de zoom |
| `exif.rs` | Extracción de metadatos de archivos con/sin EXIF; manejo de datos corruptos |
| `config/settings.rs` | Serialización/deserialización; valores default; migración de versiones |

### 4.3 Estilo de Tests
- Usa nombres descriptivos: `fn opening_corrupt_png_returns_error()` en lugar de `fn test_open_png()`.
- Usa `rstest` o tablas de test para casos parametrizados.
- Cada test debe ser **independiente**: no dependa del orden de ejecución ni de estado compartido.
- Usa `tempfile` para crear archivos temporales en tests de I/O.
- Mock filesystem cuando sea posible para tests más rápidos.

### 4.4 Tests de Snapshot (UI)
- Usa `insta` para snapshot testing de componentes UI cuando el renderizado sea determinista.
- Los snapshots deben revisarse manualmente en el primer run y commiteados.

---

## 5. Procedimientos de QA

### 5.1 Pre-Commit Checklist (Obligatorio)
Antes de marcar cualquier tarea como "completa", el agente debe verificar:

```markdown
- [ ] `cargo check` pasa sin errores
- [ ] `cargo clippy -- -D warnings` pasa sin warnings
- [ ] `cargo fmt --check` pasa
- [ ] `cargo test` pasa (todos los tests unitarios e integración)
- [ ] `cargo test --release` pasa (tests en release mode)
- [ ] Nuevas funciones públicas tienen docstrings (`///`)
- [ ] Nuevos módulos tienen documentación de diseño en `docs/` si es arquitectónicamente significativo
- [ ] No hay `.unwrap()` ni `.expect()` en código de producción
- [ ] No hay `unsafe` sin justificación documentada
- [ ] Cobertura de tests del código nuevo ≥ 80%
- [ ] Benchmarks existentes no muestran regresión (> 5% de degradación)
```

### 5.2 QA Manual (para features de UI)
Para cada feature de UI, realiza estas verificaciones manuales (documenta en el PR):

1. **Funcionalidad**: ¿Hace lo que debe hacer?
2. **Edge cases**: ¿Qué pasa con archivos corruptos, vacíos, o muy grandes?
3. **Cross-platform**: ¿Funciona en Windows, macOS y Linux?
4. **Accesibilidad**: ¿Es usable solo con teclado?
5. **Rendimiento**: ¿No hay lag perceptible?
6. **Memoria**: ¿No hay leaks? (verificar con `valgrind` o `dhat` en Linux)

### 5.3 Revisión de Código (Agente ↔ Agente)
- Todo cambio en `core/` requiere revisión de otro agente.
- Todo cambio que modifique la arquitectura requiere actualización de `docs/ARCHITECTURE.md`.
- Las revisiones deben verificar: lógica correcta, tests adecuados, documentación, y adherencia a este AGENTS.MD.

---

## 6. Métricas de Calidad y Umbrales

### 6.1 Métricas Automatizadas (CI)
| Métrica | Umbral Mínimo | Umbral Objetivo | Herramienta |
|---------|--------------|-----------------|-------------|
| Cobertura de tests (total) | 75% | 85% | `cargo tarpaulin` |
| Cobertura de tests (`core/`) | 85% | 95% | `cargo tarpaulin` |
| Warnings de Clippy | 0 | 0 | `cargo clippy` |
| Errores de `cargo check` | 0 | 0 | `cargo check` |
| Formato | 100% conforme | 100% conforme | `cargo fmt --check` |
| Tiempo de build (debug) | < 60s | < 30s | CI timer |
| Tiempo de build (release) | < 3 min | < 2 min | CI timer |
| Tamaño del binario (release) | < 20MB | < 15MB | `ls -lh` |

### 6.2 Métricas de Rendimiento (Benchmarks)
| Métrica | Umbral Máximo | Herramienta |
|---------|--------------|-------------|
| Tiempo de apertura (imagen 1080p) | < 100ms | `criterion` |
| Tiempo de apertura (imagen 4K) | < 200ms | `criterion` |
| Tiempo de apertura (imagen 8K) | < 500ms | `criterion` |
| Latencia de navegación (siguiente imagen) | < 50ms | `criterion` |
| Uso de RAM idle | < 50MB | `dhat` / `valgrind` |
| Uso de RAM (1 imagen 4K) | < 150MB | `dhat` / `valgrind` |
| FPS mínimo (pan/zoom continuo) | > 30 FPS | Instrumentación `egui` |

### 6.3 Regresiones
- **Cualquier regresión > 10% en métricas de rendimiento bloquea el merge.**
- **Cualquier caída en cobertura de tests bloquea el merge.**
- **Cualquier nuevo warning de Clippy bloquea el merge.**

---

## 7. Restricciones del Agente

### 7.1 Prohibiciones Absolutas
| Restricción | Razón |
|-------------|-------|
| ❌ No usar `unwrap()` / `expect()` en producción | Panics matan la experiencia del usuario |
| ❌ No usar `unsafe` sin justificación | Seguridad de memoria es prioridad #1 |
| ❌ No bloquear el UI thread | La app debe sentirse fluida siempre |
| ❌ No hardcodear paths o strings | Usa constantes o configuración |
| ❌ No ignorar errores con `let _ = ...` | Maneja o propaga el error |
| ❌ No usar `println!` en producción | Usa `tracing` para logging estructurado |
| ❌ No agregar dependencias sin justificación | Cada dependencia es un riesgo de supply chain |
| ❌ No dejar `TODO` sin ticket/issue | Todo TODO debe tener un issue asociado |

### 7.2 Dependencias — Proceso de Aprobación
Antes de agregar cualquier crate a `Cargo.toml`:
1. Verifica que no existe una solución con dependencias existentes.
2. Evalúa: ¿Es mantenido? ¿Tiene > 100 stars? ¿Último commit < 6 meses?
3. Verifica la licencia (debe ser compatible: MIT, Apache-2.0, BSD, etc.).
4. Documenta la justificación en un comentario sobre la dependencia en `Cargo.toml`.

### 7.3 Logging y Observabilidad
- Usa `tracing` en lugar de `println!` o `eprintln!`.
- Niveles:
  - `ERROR`: Errores que afectan la funcionalidad (imagen no se puede abrir, crash evitado).
  - `WARN`: Situaciones recuperables (formato no soportado, EXIF corrupto).
  - `INFO`: Eventos de usuario significativos (apertura de imagen, cambio de carpeta).
  - `DEBUG`: Detalles de desarrollo (cache hits/misses, tiempos de decodificación).
  - `TRACE`: Información muy detallada (eventos de UI por frame).
- En release, el nivel mínimo debe ser `INFO`.

---

## 8. Tests de Integración Obligatorios

### 8.1 Flujos Criticos
Cada uno de estos flujos debe tener un test de integración:

1. **Flujo de Apertura**:  
   `Abrir app → Diálogo de archivos → Seleccionar imagen → Renderizar → Cerrar`

2. **Flujo de Navegación**:  
   `Abrir carpeta con N imágenes → Navegar forward/backward → Verificar orden correcto`

3. **Flujo de Zoom/Pan**:  
   `Abrir imagen → Zoom in → Pan → Zoom out → Fit to window → Verificar transformaciones`

4. **Flujo de Error**:  
   `Abrir imagen corrupta → Verificar que no crashea → Verificar mensaje de error visible`

5. **Flujo de Configuración**:  
   `Modificar setting → Cerrar app → Reabrir → Verificar persistencia`

### 8.2 Fixtures de Test
- Guarda imágenes de test en `tests/fixtures/`.
- Incluye: PNG válido, JPEG válido, GIF válido, archivo corrupto (bytes aleatorios con extensión .png), archivo vacío.
- Las imágenes de test deben ser pequeñas (< 100KB) para no inflar el repo.

---

## 9. Documentación Obligatoria

### 9.1 Docstrings
```rust
/// Carga una imagen desde el filesystem de forma asíncrona.
///
/// # Arguments
/// * `path` - Ruta absoluta al archivo de imagen.
///
/// # Returns
/// * `Ok(DynamicImage)` si la decodificación fue exitosa.
/// * `Err(ShImagesError::DecodeError)` si el formato no es soportado o está corrupto.
/// * `Err(ShImagesError::IoError)` si hay problemas de lectura del filesystem.
///
/// # Examples
/// ```
/// let img = load_image(Path::new("tests/fixtures/sample.png")).await?;
/// assert_eq!(img.dimensions(), (1920, 1080));
/// ```
pub async fn load_image(path: &Path) -> Result<DynamicImage, ShImagesError> {
    // ...
}
```

### 9.2 Decisiones Arquitectónicas
- Cada decisión arquitectónica significativa (elección de framework, patrón de cache, etc.) debe documentarse en `docs/ARCHITECTURE.md` con el formato ADR (Architecture Decision Record):
  - Contexto
  - Decisión
  - Consecuencias
  - Alternativas consideradas

---

## 10. Checklist Final antes de Entregar

Antes de considerar que una tarea o sprint está completo:

```markdown
### Verificación de Código
- [ ] Todo el código nuevo tiene tests unitarios
- [ ] Los tests pasan en local (`cargo test`)
- [ ] Los tests pasan en CI
- [ ] Clippy pasa sin warnings
- [ ] Formato es correcto (`cargo fmt`)
- [ ] No hay `.unwrap()` en código de producción
- [ ] No hay `unsafe` sin documentación
- [ ] Docstrings en funciones públicas

### Verificación de Calidad
- [ ] Cobertura de tests ≥ 80% para código nuevo
- [ ] Benchmarks no muestran regresión
- [ ] No hay memory leaks (verificar con `valgrind`/`dhat`)
- [ ] App no crashea con archivos corruptos

### Verificación de Documentación
- [ ] `docs/ARCHITECTURE.md` actualizado si aplica
- [ ] CHANGELOG.md actualizado
- [ ] README.md actualizado si hay cambios de uso

### Verificación de UX
- [ ] Feature funciona con teclado
- [ ] Feature funciona con ratón
- [ ] Mensajes de error son claros para el usuario
- [ ] No hay lag perceptible
```

---

## 11. Comandos de Referencia Rápida

```bash
# Verificar todo antes de commit
cargo check && cargo clippy -- -D warnings && cargo fmt --check && cargo test

# Verificar en release mode
cargo test --release

# Generar reporte de cobertura
cargo tarpaulin --out Html --output-dir target/coverage

# Ejecutar benchmarks
cargo bench

# Perfil de memoria (Linux)
cargo build --release && valgrind --tool=massif ./target/release/sh_images

# Perfil de rendimiento
cargo flamegraph

# Verificar tamaño del binario
cargo build --release && ls -lh target/release/sh_images
```

---

> **Recuerda**: Calidad sobre velocidad. Es mejor entregar menos features perfectamente testeados que muchos features con bugs. Sh_Images debe ser un ejemplo de código Rust idiomático, seguro y performante.
