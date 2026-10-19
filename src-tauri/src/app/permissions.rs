// src-tauri/src/app/permissions.rs
//! Validation forte des entrées IPC : aucun chemin, identifiant ou commande arbitraire.

use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::valid_id;
use std::path::PathBuf;

fn invalid(what: &str) -> AppError {
    AppError::new(codes::INVALID_ARGUMENT, format!("{what} invalide"))
}

pub fn device_id(id: &str) -> AppResult<&str> {
    if id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        Ok(id)
    } else {
        Err(invalid("identifiant d'appareil"))
    }
}

pub fn profile_id(id: &str) -> AppResult<&str> {
    if valid_id(id) {
        Ok(id)
    } else {
        Err(invalid("identifiant de profil"))
    }
}

pub fn session_id(id: &str) -> AppResult<&str> {
    if !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_graphic()) {
        Ok(id)
    } else {
        Err(invalid("identifiant de session"))
    }
}

/// Chemin choisi par l'utilisateur dans la boîte de dialogue : absolu, extension .txt,
/// dossier parent existant, pas un dossier.
pub fn export_path(path: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(path);
    let parent_ok = p.parent().map_or(false, |d| d.is_dir());
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .map_or(false, |e| e.eq_ignore_ascii_case("txt"));
    if p.is_absolute() && parent_ok && ext_ok && !p.is_dir() {
        Ok(p)
    } else {
        Err(AppError::new(codes::EXPORT_FAILED, "chemin d'export invalide (fichier .txt attendu)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_validated() {
        assert!(device_id("00112233445566ff").is_ok());
        assert!(device_id("../etc").is_err());
        assert!(device_id("00112233445566FF").is_err());
        assert!(profile_id("jeu-1").is_ok());
        assert!(profile_id("a/b").is_err());
        assert!(session_id("mock-1-0").is_ok());
        assert!(session_id("").is_err());
    }

    #[test]
    fn export_path_rules() {
        let dir = std::env::temp_dir();
        assert!(export_path(dir.join("diag.txt").to_str().unwrap()).is_ok());
        assert!(export_path(dir.join("diag.exe").to_str().unwrap()).is_err());
        assert!(export_path("diag.txt").is_err());
        assert!(export_path(dir.join("absent").join("diag.txt").to_str().unwrap()).is_err());
    }
}
