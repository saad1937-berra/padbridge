// src-tauri/src/profiles/store.rs
//! Stockage local : écritures atomiques, sauvegarde du fichier précédent,
//! mise en quarantaine des fichiers corrompus (retour à un profil sûr).

use super::migrations;
use super::schema::{valid_id, AppSettings, Profile, DEFAULT_PROFILE_ID};
use crate::error::{codes, AppError, AppResult};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Écrit `bytes` dans un fichier temporaire, le synchronise, garde l'ancien en `.bak`,
/// puis remplace la cible par renommage.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("bak"));
    }
    fs::rename(&tmp, path)
}

fn quarantine(path: &Path) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
    let target = path.with_file_name(format!("{stem}.corrupt-{}.json", now_ms()));
    let _ = fs::rename(path, target);
}

pub struct LoadOutcome {
    pub profile: Profile,
    /// Vrai si un fichier corrompu a été mis de côté et remplacé par le profil par défaut.
    pub recovered: bool,
}

pub struct ProfileStore {
    dir: PathBuf,
}

impl ProfileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn init(&self) -> AppResult<()> {
        fs::create_dir_all(&self.dir).map_err(AppError::storage)?;
        if !self.path_for(DEFAULT_PROFILE_ID)?.exists() {
            self.write(&Profile::default_profile())?;
        }
        Ok(())
    }

    fn path_for(&self, id: &str) -> AppResult<PathBuf> {
        if !valid_id(id) {
            return Err(AppError::new(codes::INVALID_ARGUMENT, "identifiant de profil invalide"));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    fn write(&self, profile: &Profile) -> AppResult<()> {
        let path = self.path_for(&profile.id)?;
        let json = serde_json::to_string_pretty(profile)
            .map_err(|e| AppError::new(codes::INTERNAL, e.to_string()))?;
        atomic_write(&path, json.as_bytes()).map_err(AppError::storage)
    }

    /// Lit, migre et valide un profil.
    fn parse(text: &str) -> AppResult<(Profile, bool)> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| AppError::new(codes::PROFILE_CORRUPT, e.to_string()))?;
        let (value, migrated) = migrations::migrate(value)?;
        let profile: Profile = serde_json::from_value(value)
            .map_err(|e| AppError::new(codes::PROFILE_CORRUPT, e.to_string()))?;
        let report = profile.validate();
        if let Some(issue) = report.errors.first() {
            return Err(AppError::new(
                codes::PROFILE_INVALID,
                format!("{}: {}", issue.field, issue.code),
            ));
        }
        Ok((profile, migrated))
    }

    pub fn load(&self, id: &str) -> AppResult<LoadOutcome> {
        let path = self.path_for(id)?;
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if id == DEFAULT_PROFILE_ID {
                    let profile = Profile::default_profile();
                    self.write(&profile)?;
                    return Ok(LoadOutcome { profile, recovered: false });
                }
                return Err(AppError::new(codes::PROFILE_NOT_FOUND, id.to_string()));
            }
            Err(e) => return Err(AppError::storage(e)),
        };

        match Self::parse(&text) {
            Ok((profile, migrated)) if profile.id == id => {
                if migrated {
                    let _ = fs::write(path.with_extension("pre-migration.bak"), &text);
                    self.write(&profile)?;
                }
                Ok(LoadOutcome { profile, recovered: false })
            }
            Ok(_) | Err(_) => {
                quarantine(&path);
                if id == DEFAULT_PROFILE_ID {
                    let profile = Profile::default_profile();
                    self.write(&profile)?;
                    Ok(LoadOutcome { profile, recovered: true })
                } else {
                    Err(AppError::new(codes::PROFILE_CORRUPT, id.to_string()))
                }
            }
        }
    }

    pub fn save(&self, profile: &Profile) -> AppResult<Profile> {
        let report = profile.validate();
        if let Some(issue) = report.errors.first() {
            return Err(AppError::new(
                codes::PROFILE_INVALID,
                format!("{}: {}", issue.field, issue.code),
            ));
        }
        let mut stored = profile.clone();
        stored.name = stored.name.trim().to_string();
        stored.updated_at = now_ms();
        self.write(&stored)?;
        Ok(stored)
    }

    /// Profils lisibles ; le profil par défaut en premier, puis par nom.
    pub fn list(&self) -> AppResult<Vec<Profile>> {
        let mut profiles = Vec::new();
        let entries = fs::read_dir(&self.dir).map_err(AppError::storage)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            if !valid_id(stem) {
                continue; // fichiers « .corrupt-… » et assimilés
            }
            if let Ok(outcome) = self.load(stem) {
                profiles.push(outcome.profile);
            }
        }
        profiles.sort_by(|a, b| {
            (b.id == DEFAULT_PROFILE_ID)
                .cmp(&(a.id == DEFAULT_PROFILE_ID))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(profiles)
    }

    fn unique_id(&self, name: &str) -> AppResult<String> {
        let mut slug = String::new();
        for c in name.to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_matches('-');
        let base: String = if slug.is_empty() { "profil".into() } else { slug.chars().take(48).collect() };
        for n in 1..1000 {
            let candidate = if n == 1 { base.clone() } else { format!("{base}-{n}") };
            if candidate != DEFAULT_PROFILE_ID && !self.path_for(&candidate)?.exists() {
                return Ok(candidate);
            }
        }
        Err(AppError::new(codes::INTERNAL, "impossible de générer un identifiant"))
    }

    pub fn duplicate(&self, id: &str) -> AppResult<Profile> {
        let mut copy = self.load(id)?.profile;
        copy.name = format!("{} (copie)", copy.name);
        copy.id = self.unique_id(&copy.name)?;
        self.save(&copy)
    }

    pub fn rename(&self, id: &str, new_name: &str) -> AppResult<Profile> {
        let mut profile = self.load(id)?.profile;
        profile.name = new_name.to_string();
        self.save(&profile)
    }

    /// Remet mapping, zones mortes et mode à leurs valeurs initiales ; garde l'id et le nom.
    pub fn reset(&self, id: &str) -> AppResult<Profile> {
        let current = self.load(id)?.profile;
        let mut fresh = Profile::default_profile();
        fresh.id = current.id;
        fresh.name = current.name;
        self.save(&fresh)
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Réglages courants ; valeurs par défaut si le fichier manque ou est corrompu.
    pub fn load(&self) -> AppSettings {
        let Ok(text) = fs::read_to_string(&self.path) else {
            return AppSettings::default();
        };
        match serde_json::from_str::<AppSettings>(&text) {
            Ok(s) if s.validate().is_valid() => s,
            _ => {
                quarantine(&self.path);
                AppSettings::default()
            }
        }
    }

    pub fn save(&self, settings: &AppSettings) -> AppResult<AppSettings> {
        if let Some(issue) = settings.validate().errors.first() {
            return Err(AppError::new(
                codes::INVALID_ARGUMENT,
                format!("{}: {}", issue.field, issue.code),
            ));
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(AppError::storage)?;
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| AppError::new(codes::INTERNAL, e.to_string()))?;
        atomic_write(&self.path, json.as_bytes()).map_err(AppError::storage)?;
        Ok(settings.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::model::PsButton;
    use crate::virtual_pad::backend::XButton;

    fn store() -> (tempfile::TempDir, ProfileStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("profiles"));
        store.init().unwrap();
        (dir, store)
    }

    #[test]
    fn save_and_reload() {
        let (_d, store) = store();
        let mut p = store.load(DEFAULT_PROFILE_ID).unwrap().profile;
        p.deadzones.left_stick = 0.2;
        p.mapping.insert(PsButton::Cross, XButton::B);
        store.save(&p).unwrap();
        let again = store.load(DEFAULT_PROFILE_ID).unwrap().profile;
        assert_eq!(again.deadzones.left_stick, 0.2);
        assert_eq!(again.mapping.get(&PsButton::Cross), Some(&XButton::B));
        assert!(again.updated_at > 0);
    }

    #[test]
    fn corrupt_default_profile_is_recovered_and_kept_for_support() {
        let (dir, store) = store();
        let path = dir.path().join("profiles").join("profile-default.json");
        fs::write(&path, "{ pas du json").unwrap();
        let outcome = store.load(DEFAULT_PROFILE_ID).unwrap();
        assert!(outcome.recovered);
        assert_eq!(outcome.profile, {
            let mut p = Profile::default_profile();
            p.updated_at = outcome.profile.updated_at;
            p
        });
        let corrupt_copies = fs::read_dir(dir.path().join("profiles"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(corrupt_copies, 1);
    }

    #[test]
    fn corrupt_custom_profile_reports_error_not_default() {
        let (dir, store) = store();
        fs::write(dir.path().join("profiles").join("jeu.json"), "[]").unwrap();
        let err = store.load("jeu").err().unwrap();
        assert_eq!(err.code, codes::PROFILE_CORRUPT);
    }

    #[test]
    fn invalid_profile_is_never_written() {
        let (_d, store) = store();
        let mut p = Profile::default_profile();
        p.deadzones.right_stick = 2.0;
        assert_eq!(store.save(&p).err().unwrap().code, codes::PROFILE_INVALID);
        assert_eq!(
            store.load(DEFAULT_PROFILE_ID).unwrap().profile.deadzones.right_stick,
            0.08
        );
    }

    #[test]
    fn path_traversal_is_refused() {
        let (_d, store) = store();
        assert_eq!(store.load("../settings").err().unwrap().code, codes::INVALID_ARGUMENT);
    }

    #[test]
    fn duplicate_rename_reset_list() {
        let (_d, store) = store();
        let copy = store.duplicate(DEFAULT_PROFILE_ID).unwrap();
        assert_eq!(copy.id, "standard-copie");
        let renamed = store.rename(&copy.id, "  Jeu de course ").unwrap();
        assert_eq!(renamed.name, "Jeu de course");

        let mut edited = renamed.clone();
        edited.deadzones.left_trigger = 0.3;
        store.save(&edited).unwrap();
        let reset = store.reset(&edited.id).unwrap();
        assert_eq!(reset.deadzones, Default::default());
        assert_eq!(reset.name, "Jeu de course");

        let ids: Vec<_> = store.list().unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![DEFAULT_PROFILE_ID.to_string(), "standard-copie".into()]);
    }

    #[test]
    fn settings_roundtrip_and_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        assert_eq!(store.load(), AppSettings::default());
        let mut s = AppSettings::default();
        s.theme = "dark".into();
        store.save(&s).unwrap();
        assert_eq!(store.load().theme, "dark");
        fs::write(dir.path().join("settings.json"), "###").unwrap();
        assert_eq!(store.load(), AppSettings::default());
    }
}
