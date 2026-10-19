// src-tauri/src/profiles/schema.rs
//! Profils et réglages : schéma versionné, validation stricte (liste blanche de champs).

use crate::devices::model::{ControllerModel, PsButton};
use crate::virtual_pad::backend::XButton;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_PROFILE_ID: &str = "profile-default";
pub const MAX_DEADZONE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// Pass-through : Windows expose la manette telle quelle, aucune sortie virtuelle.
    Native,
    /// Manette virtuelle XInput (nécessite un backend validé).
    Xinput,
    /// Lecture et tests uniquement.
    Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Deadzones {
    pub left_stick: f32,
    pub right_stick: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
}

impl Default for Deadzones {
    fn default() -> Self {
        Self {
            left_stick: 0.08,
            right_stick: 0.08,
            left_trigger: 0.02,
            right_trigger: 0.02,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub output_mode: OutputMode,
    #[serde(default)]
    pub controller_model: Option<ControllerModel>,
    #[serde(default)]
    pub deadzones: Deadzones,
    /// Surcharges du préréglage standard (source PlayStation → cible XInput).
    #[serde(default)]
    pub mapping: BTreeMap<PsButton, XButton>,
    #[serde(default)]
    pub rumble_enabled: bool,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub field: String,
    pub code: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    fn error(&mut self, field: &str, code: &str, detail: Option<String>) {
        self.errors.push(ValidationIssue {
            field: field.into(),
            code: code.into(),
            detail,
        });
    }

    fn warning(&mut self, field: &str, code: &str, detail: Option<String>) {
        self.warnings.push(ValidationIssue {
            field: field.into(),
            code: code.into(),
            detail,
        });
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Préréglage standard : croix→A, rond→B, carré→X, triangle→Y, etc.
pub fn standard_preset() -> BTreeMap<PsButton, XButton> {
    use PsButton as P;
    use XButton as X;
    BTreeMap::from([
        (P::Cross, X::A),
        (P::Circle, X::B),
        (P::Square, X::X),
        (P::Triangle, X::Y),
        (P::L1, X::LB),
        (P::R1, X::RB),
        (P::Share, X::Back),
        (P::Options, X::Start),
        (P::Ps, X::Guide),
        (P::L3, X::LS),
        (P::R3, X::RS),
        (P::DpadUp, X::DpadUp),
        (P::DpadDown, X::DpadDown),
        (P::DpadLeft, X::DpadLeft),
        (P::DpadRight, X::DpadRight),
    ])
}

impl Profile {
    pub fn default_profile() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: DEFAULT_PROFILE_ID.into(),
            name: "Standard".into(),
            output_mode: OutputMode::Native,
            controller_model: None,
            deadzones: Deadzones::default(),
            mapping: BTreeMap::new(),
            rumble_enabled: false,
            updated_at: 0,
        }
    }

    /// Préréglage standard + surcharges du profil.
    pub fn effective_mapping(&self) -> BTreeMap<PsButton, XButton> {
        let mut map = standard_preset();
        for (src, dst) in &self.mapping {
            map.insert(*src, *dst);
        }
        map
    }

    pub fn validate(&self) -> ValidationReport {
        let mut r = ValidationReport::default();

        if self.schema_version != SCHEMA_VERSION {
            r.error("schemaVersion", "UNSUPPORTED_SCHEMA", Some(self.schema_version.to_string()));
        }
        if !valid_id(&self.id) {
            r.error("id", "ID_INVALID", None);
        }
        let name = self.name.trim();
        if name.is_empty() {
            r.error("name", "NAME_EMPTY", None);
        } else if name.chars().count() > 64 {
            r.error("name", "NAME_TOO_LONG", None);
        }

        let d = &self.deadzones;
        for (field, value) in [
            ("deadzones.leftStick", d.left_stick),
            ("deadzones.rightStick", d.right_stick),
            ("deadzones.leftTrigger", d.left_trigger),
            ("deadzones.rightTrigger", d.right_trigger),
        ] {
            if !value.is_finite() || !(0.0..=MAX_DEADZONE).contains(&value) {
                r.error(field, "DEADZONE_OUT_OF_RANGE", Some(format!("0 – {MAX_DEADZONE}")));
            }
        }

        // PB-014 : prévenir quand une action indispensable n'est plus atteignable.
        let reachable: Vec<XButton> = self.effective_mapping().values().copied().collect();
        for essential in [XButton::A, XButton::B, XButton::Start] {
            if !reachable.contains(&essential) {
                r.warning("mapping", "ACTION_UNREACHABLE", Some(essential.label().into()));
            }
        }
        r
    }
}

// ---------------------------------------------------------------------------
// Réglages applicatifs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct AppSettings {
    pub language: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    pub log_retention_days: u32,
    pub onboarding_done: bool,
    pub active_profile_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "fr".into(),
            theme: "system".into(),
            log_retention_days: 7,
            onboarding_done: false,
            active_profile_id: DEFAULT_PROFILE_ID.into(),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> ValidationReport {
        let mut r = ValidationReport::default();
        if self.language != "fr" {
            r.error("language", "LANGUAGE_UNSUPPORTED", Some(self.language.clone()));
        }
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            r.error("theme", "THEME_INVALID", Some(self.theme.clone()));
        }
        if !(1..=90).contains(&self.log_retention_days) {
            r.error("logRetentionDays", "RETENTION_OUT_OF_RANGE", Some("1 – 90".into()));
        }
        if !valid_id(&self.active_profile_id) {
            r.error("activeProfileId", "ID_INVALID", None);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_valid_without_warnings() {
        let report = Profile::default_profile().validate();
        assert!(report.is_valid());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn doc_example_profile_parses() {
        let json = r#"{
            "schemaVersion": 1, "id": "profile-default", "name": "Standard Xbox",
            "outputMode": "xinput", "controllerModel": "dualsense",
            "deadzones": {"leftStick": 0.08, "rightStick": 0.08},
            "mapping": {"cross": "A", "circle": "B", "square": "X", "triangle": "Y"},
            "rumbleEnabled": false
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.output_mode, OutputMode::Xinput);
        assert_eq!(p.controller_model, Some(ControllerModel::DualSense));
        assert!(p.validate().is_valid());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"schemaVersion":1,"id":"a","name":"A","outputMode":"native","script":"calc.exe"}"#;
        assert!(serde_json::from_str::<Profile>(json).is_err());
    }

    #[test]
    fn out_of_range_deadzone_is_an_error() {
        let mut p = Profile::default_profile();
        p.deadzones.left_stick = 0.9;
        let report = p.validate();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].field, "deadzones.leftStick");
        p.deadzones.left_stick = f32::NAN;
        assert!(!p.validate().is_valid());
    }

    #[test]
    fn invalid_ids_and_names() {
        let mut p = Profile::default_profile();
        p.id = "../evil".into();
        p.name = "   ".into();
        let codes: Vec<_> = p.validate().errors.into_iter().map(|e| e.code).collect();
        assert!(codes.contains(&"ID_INVALID".to_string()));
        assert!(codes.contains(&"NAME_EMPTY".to_string()));
    }

    #[test]
    fn unreachable_essential_action_warns() {
        let mut p = Profile::default_profile();
        // croix et rond visent tous deux B : A n'est plus atteignable
        p.mapping.insert(PsButton::Cross, XButton::B);
        let report = p.validate();
        assert!(report.is_valid());
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].detail.as_deref(), Some("A"));
    }

    #[test]
    fn settings_validation() {
        assert!(AppSettings::default().validate().is_valid());
        let s = AppSettings { log_retention_days: 0, ..AppSettings::default() };
        assert!(!s.validate().is_valid());
    }
}
