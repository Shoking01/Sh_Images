//! Resolución de rutas de configuración por plataforma.

use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use crate::utils::errors::{Result, ShImagesError};

/// Resuelve el directorio raíz de configuración del usuario.
///
/// - Windows: `%APPDATA%`
/// - macOS: `$HOME/Library/Application Support`
/// - Linux: `$XDG_CONFIG_HOME` o, si no está definida, `$HOME/.config`
///
/// `None` en `appdata`/`home`/`xdg` significa "env var no definida".
fn config_dir_with(
    appdata: Option<&OsStr>,
    home: Option<&OsStr>,
    xdg: Option<&OsStr>,
) -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let _ = (home, xdg);
        let base =
            appdata.ok_or_else(|| ShImagesError::Config("APPDATA is not set".to_string()))?;
        Ok(PathBuf::from(base))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (appdata, xdg);
        let base = home.ok_or_else(|| ShImagesError::Config("HOME is not set".to_string()))?;
        Ok(PathBuf::from(base)
            .join("Library")
            .join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = xdg {
            return Ok(PathBuf::from(xdg));
        }
        let base = home.ok_or_else(|| ShImagesError::Config("HOME is not set".to_string()))?;
        Ok(PathBuf::from(base).join(".config"))
    }
}

/// Directorio raíz de configuración resolvido desde el entorno real.
pub fn config_dir() -> Result<PathBuf> {
    config_dir_with(
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
    )
}

/// Ruta del archivo de configuración dentro de un directorio raíz dado
/// (`<config_dir>/sh_images/settings.toml`). Función pura para testear.
pub fn settings_path_in(config_dir: &Path) -> PathBuf {
    config_dir.join("sh_images").join("settings.toml")
}

/// Ruta completa del archivo de configuración desde el entorno real.
pub fn settings_path() -> Result<PathBuf> {
    Ok(settings_path_in(&config_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn config_dir_errors_when_no_env_var_is_available() {
        let err = config_dir_with(None, None, None).unwrap_err();
        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_config_dir_uses_appdata() {
        let path = config_dir_with(
            Some(OsStr::new(r"C:\Users\test\AppData\Roaming")),
            None,
            None,
        )
        .unwrap();
        assert_eq!(path, PathBuf::from(r"C:\Users\test\AppData\Roaming"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_config_dir_uses_home_library() {
        let path = config_dir_with(None, Some(OsStr::new("/Users/test")), None).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/Library/Application Support")
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn linux_config_dir_prefers_xdg() {
        let path = config_dir_with(
            None,
            Some(OsStr::new("/home/test")),
            Some(OsStr::new("/etc/xdg")),
        )
        .unwrap();
        assert_eq!(path, PathBuf::from("/etc/xdg"));
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn linux_config_dir_falls_back_to_home_dot_config() {
        let path = config_dir_with(None, Some(OsStr::new("/home/test")), None).unwrap();
        assert_eq!(path, PathBuf::from("/home/test/.config"));
    }

    #[test]
    fn settings_path_in_composes_sh_images_subdir() {
        let root = PathBuf::from("/tmp/config");
        assert_eq!(
            settings_path_in(&root),
            PathBuf::from("/tmp/config/sh_images/settings.toml")
        );
    }
}
