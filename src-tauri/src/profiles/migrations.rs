// src-tauri/src/profiles/migrations.rs
//! Migration des profils d'une version de schéma à la suivante (NFR-13).
//!
//! Pour ajouter la version N+1 : incrémenter `SCHEMA_VERSION`, écrire `step_N_to_N1`
//! et l'ajouter au `match` de `migrate`. Le magasin sauvegarde l'ancien fichier
//! avant de réécrire un profil migré.

use super::schema::SCHEMA_VERSION;
use crate::error::{codes, AppError, AppResult};
use serde_json::Value;

/// Migration d'une version `from` vers `from + 1`.
#[allow(unused_variables)]
fn step(from: u32, value: Value) -> AppResult<Value> {
    match from {
        // 1 => Ok(step_1_to_2(value)),
        other => Err(AppError::new(
            codes::PROFILE_INVALID,
            format!("aucune migration depuis le schéma {other}"),
        )),
    }
}

/// Retourne le JSON à la version courante et un indicateur « modifié ».
pub fn migrate(value: Value) -> AppResult<(Value, bool)> {
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::new(codes::PROFILE_INVALID, "schemaVersion absent"))?;

    if version > u64::from(SCHEMA_VERSION) {
        return Err(AppError::new(
            codes::PROFILE_INVALID,
            format!("profil créé par une version plus récente (schéma {version})"),
        ));
    }
    if version == 0 {
        return Err(AppError::new(codes::PROFILE_INVALID, "schéma 0 inexistant"));
    }

    let mut current = value;
    let mut changed = false;
    for from in (version as u32)..SCHEMA_VERSION {
        current = step(from, current)?;
        changed = true;
    }
    Ok((current, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn current_version_is_untouched() {
        let (v, changed) = migrate(json!({"schemaVersion": SCHEMA_VERSION})).unwrap();
        assert!(!changed);
        assert_eq!(v["schemaVersion"], SCHEMA_VERSION);
    }

    #[test]
    fn newer_or_missing_version_is_refused() {
        assert!(migrate(json!({"schemaVersion": SCHEMA_VERSION + 1})).is_err());
        assert!(migrate(json!({"name": "x"})).is_err());
        assert!(migrate(json!({"schemaVersion": 0})).is_err());
    }
}
