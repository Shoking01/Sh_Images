//! Persistencia de preferencias de usuario en TOML.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::shortcuts::ShortcutMap;
use crate::utils::errors::{Result, ShImagesError};

/// Valor por defecto del intervalo del slideshow (5 s).
fn default_slideshow_interval_secs() -> u64 {
    5
}

/// Preferencias persistentes de la aplicación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Límite de memoria del LRU cache en MiB (default: 512).
    #[serde(default)]
    pub cache_memory_limit_mb: u64,
    /// Tema visual de la UI: `"dark"` | `"light"`.
    #[serde(default)]
    pub theme: String,
    /// Atajos de teclado configurables (default: los de `ShortcutMap::defaults`).
    #[serde(default)]
    pub shortcuts: ShortcutMap,
    /// Intervalo del slideshow en segundos (default: 5).
    #[serde(default = "default_slideshow_interval_secs")]
    pub slideshow_interval_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cache_memory_limit_mb: 512,
            theme: "dark".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
        }
    }
}

impl Settings {
    /// Carga las preferencias desde `path`.
    ///
    /// Si el archivo no existe, devuelve los defaults y los persiste en disco.
    /// Si existe pero está corrupto, devuelve `ShImagesError::Config`.
    pub fn load(path: &Path) -> Result<Settings> {
        match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(|e| {
                ShImagesError::Config(format!("invalid settings in {}: {e}", path.display()))
            }),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let settings = Settings::default();
                settings.save(path)?;
                Ok(settings)
            }
            Err(e) => Err(ShImagesError::Io(e)),
        }
    }

    /// Persiste las preferencias en `path` (crea el directorio padre si falta, escribe a temp y renombra de forma atómica).
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ShImagesError::Config(format!("failed to serialize settings: {e}")))?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, content)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_settings_match_plan_values() {
        let s = Settings::default();
        assert_eq!(s.cache_memory_limit_mb, 512);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.slideshow_interval_secs, 5);
    }

    #[test]
    fn loading_missing_file_creates_defaults_and_persists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");

        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, Settings::default());
        assert!(path.exists(), "load() should persist defaults");
        let on_disk = fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("cache_memory_limit_mb = 512"));
        assert!(on_disk.contains("theme = \"dark\""));
        assert!(on_disk.contains("slideshow_interval_secs = 5"));
    }

    #[test]
    fn load_creates_parent_directory_and_persists_defaults() {
        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join("nested")
            .join("deeper")
            .join("settings.toml");

        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, Settings::default());
        assert!(path.exists());
    }

    #[test]
    fn save_to_invalid_path_returns_io_error() {
        let dir = tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"i am a file, not a dir").unwrap();
        let path = blocker.join("settings.toml");

        let settings = Settings::default();
        let err = settings.save(&path).unwrap_err();

        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let settings = Settings {
            cache_memory_limit_mb: 256,
            theme: "light".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
        };

        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn corrupt_file_returns_config_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, "this is not {{{ valid toml").unwrap();

        let err = Settings::load(&path).unwrap_err();

        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[test]
    fn save_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let first = Settings::default();
        first.save(&path).unwrap();
        let second = Settings {
            cache_memory_limit_mb: 128,
            theme: "dark".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
        };

        second.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, second);
    }

    #[test]
    fn slideshow_interval_defaults_to_five() {
        let s = Settings::default();
        assert_eq!(s.slideshow_interval_secs, 5);
    }

    #[test]
    fn toml_without_new_field_migrates_to_default() {
        let content = "cache_memory_limit_mb = 256\ntheme = \"light\"\n";
        let s: Settings = toml::from_str(content).expect("deserializar");
        assert_eq!(s.slideshow_interval_secs, 5);
    }

    #[test]
    fn slideshow_interval_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let settings = Settings {
            cache_memory_limit_mb: 256,
            theme: "light".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 10,
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, settings);
    }
}
