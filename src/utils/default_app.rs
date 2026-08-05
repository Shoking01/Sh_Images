use std::path::Path;

use crate::utils::errors::{Result, ShImagesError};
pub(crate) const ASSOCIATED_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".bmp", ".gif", ".webp", ".tiff", ".tif", ".avif",
];
const PROG_ID: &str = "ShImages.ImageViewer";
const PROG_ID_FRIENDLY: &str = "Sh_Images Image Viewer";
pub fn set_as_default_image_viewer() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let exe_path = std::env::current_exe().map_err(|e| {
            ShImagesError::Config(format!("no se pudo obtener la ruta del ejecutable: {e}"))
        })?;
        let exe_path_str = exe_path.to_string_lossy().to_string();
        let command = format!("\"{exe_path_str}\" \"%1\"");

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let prog_id_path = format!("Software\\Classes\\{PROG_ID}");
        let (prog_id_key, _) = hkcu
            .create_subkey(&prog_id_path)
            .map_err(|e| ShImagesError::Config(format!("no se pudo crear ProgId: {e}")))?;
        prog_id_key.set_value("", &PROG_ID_FRIENDLY).map_err(|e| {
            ShImagesError::Config(format!("no se pudo establecer descripción: {e}"))
        })?;
        let cmd_path = format!("{prog_id_path}\\shell\\open\\command");
        let (cmd_key, _) = hkcu.create_subkey(&cmd_path).map_err(|e| {
            ShImagesError::Config(format!("no se pudo crear clave de comando: {e}"))
        })?;
        cmd_key
            .set_value("", &command)
            .map_err(|e| ShImagesError::Config(format!("no se pudo establecer comando: {e}")))?;
        let icon_path = format!("{prog_id_path}\\DefaultIcon");
        let (icon_key, _) = hkcu
            .create_subkey(&icon_path)
            .map_err(|e| ShImagesError::Config(format!("no se pudo crear clave de icono: {e}")))?;
        icon_key
            .set_value("", &format!("\"{exe_path_str}\",0"))
            .map_err(|e| ShImagesError::Config(format!("no se pudo establecer icono: {e}")))?;
        for ext in ASSOCIATED_EXTENSIONS {
            let ext_path = format!("Software\\Classes\\{ext}");
            let (ext_key, _) = hkcu.create_subkey(&ext_path).map_err(|e| {
                ShImagesError::Config(format!("no se pudo crear asociación {ext}: {e}"))
            })?;
            ext_key.set_value("", &PROG_ID).map_err(|e| {
                ShImagesError::Config(format!("no se pudo establecer ProgId para {ext}: {e}"))
            })?;
        }
        register_capabilities(exe_path.as_path())?;
        open_default_apps_settings()?;

        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(ShImagesError::UnsupportedFormat(
            "esta función solo está disponible en Windows".to_string(),
        ))
    }
}
#[cfg(target_os = "windows")]
fn register_capabilities(exe_path: &Path) -> Result<()> {
    let exe_name = exe_path
        .file_name()
        .ok_or_else(|| ShImagesError::Config("nombre de ejecutable inválido".to_string()))?
        .to_string_lossy()
        .to_string();

    let capabilities_path = format!("Software\\{exe_name}\\Capabilities");
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (cap_key, _) = hkcu
        .create_subkey(&capabilities_path)
        .map_err(|e| ShImagesError::Config(format!("no se pudo crear Capabilities: {e}")))?;

    cap_key
        .set_value("ApplicationName", &PROG_ID_FRIENDLY)
        .map_err(|e| {
            ShImagesError::Config(format!("no se pudo establecer ApplicationName: {e}"))
        })?;
    cap_key
        .set_value(
            "ApplicationDescription",
            &"Visor de imágenes rápido y ligero",
        )
        .map_err(|e| {
            ShImagesError::Config(format!("no se pudo establecer ApplicationDescription: {e}"))
        })?;
    let file_assoc_path = format!("{capabilities_path}\\FileAssociations");
    let (fa_key, _) = hkcu
        .create_subkey(&file_assoc_path)
        .map_err(|e| ShImagesError::Config(format!("no se pudo crear FileAssociations: {e}")))?;
    for ext in ASSOCIATED_EXTENSIONS {
        let prog_id = PROG_ID.to_string();
        fa_key
            .set_value(ext, &prog_id)
            .map_err(|e| ShImagesError::Config(format!("no se pudo asociar {ext}: {e}")))?;
    }
    let hkcr = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let ra_path = "Software\\RegisteredApplications";
    let (ra_key, _) = hkcr.create_subkey(ra_path).map_err(|e| {
        ShImagesError::Config(format!("no se pudo abrir RegisteredApplications: {e}"))
    })?;
    ra_key
        .set_value(&exe_name, &format!("Software\\{exe_name}\\Capabilities"))
        .map_err(|e| ShImagesError::Config(format!("no se pudo registrar aplicación: {e}")))?;

    Ok(())
}
#[cfg(target_os = "windows")]
fn open_default_apps_settings() -> Result<()> {
    let result = std::process::Command::new("cmd")
        .args(["/c", "start", "", "ms-settings:defaultapps"])
        .spawn();
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(ShImagesError::Config(format!(
            "no se pudo abrir la configuración de Windows: {e}"
        ))),
    }
}
pub fn associated_extensions() -> &'static [&'static str] {
    ASSOCIATED_EXTENSIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associated_extensions_contains_common_formats() {
        assert!(ASSOCIATED_EXTENSIONS.contains(&".png"));
        assert!(ASSOCIATED_EXTENSIONS.contains(&".jpg"));
        assert!(ASSOCIATED_EXTENSIONS.contains(&".gif"));
        assert!(ASSOCIATED_EXTENSIONS.contains(&".webp"));
    }

    #[test]
    fn prog_id_is_valid() {
        assert!(!PROG_ID.is_empty());
        assert!(!PROG_ID_FRIENDLY.is_empty());
    }
}
