//! Sh_Images — Visor de imágenes nativo en Rust.
//!
//! La librería contiene toda la lógica de negocio, separada del binario
//! (`main.rs`). Esto permite tests de integración y benchmarks sobre la lógica
//! sin acoplar a la UI.

pub mod config;
pub mod core;
pub mod ui;
pub mod utils;
