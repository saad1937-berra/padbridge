// src-tauri/src/diagnostics/double_input.rs
//! Évaluation du risque de double entrée (PB-019, PB-020).
//! Aucune injection, aucun masquage automatique : on explique et on guide.

use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct DoubleInputFacts {
    pub virtual_sessions: usize,
    pub steam_running: bool,
    /// Vrai seulement si un filtre HID validé masque la manette physique (hors MVP).
    pub physical_hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    None,
    Possible,
    Likely,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleInputAssessment {
    pub risk: Risk,
    /// Codes de cause ; l'interface fournit le texte.
    pub causes: Vec<String>,
    /// Codes d'action guidée, par ordre de préférence.
    pub guidance: Vec<String>,
}

pub fn assess(facts: &DoubleInputFacts) -> DoubleInputAssessment {
    let mut causes = Vec::new();
    let mut guidance = Vec::new();
    let mut risk = Risk::None;

    if facts.virtual_sessions > 0 && !facts.physical_hidden {
        causes.push("PHYSICAL_AND_VIRTUAL".to_string());
        risk = Risk::Possible;
    }
    if facts.virtual_sessions > 0 && facts.steam_running {
        causes.push("STEAM_RUNNING".to_string());
        risk = Risk::Likely;
    }

    if risk != Risk::None {
        guidance.push("RETURN_TO_NATIVE".to_string());
        if facts.steam_running {
            guidance.push("DISABLE_STEAM_INPUT_FOR_CONTROLLER".to_string());
        }
        guidance.push("CHECK_GAME_INPUT_SETTINGS".to_string());
        guidance.push("OPTIONAL_HID_FILTER".to_string());
    }

    DoubleInputAssessment { risk, causes, guidance }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(virtual_sessions: usize, steam_running: bool, physical_hidden: bool) -> DoubleInputFacts {
        DoubleInputFacts { virtual_sessions, steam_running, physical_hidden }
    }

    #[test]
    fn native_mode_has_no_risk_even_with_steam() {
        let a = assess(&facts(0, true, false));
        assert_eq!(a.risk, Risk::None);
        assert!(a.guidance.is_empty());
    }

    #[test]
    fn virtual_pad_alone_is_possible() {
        let a = assess(&facts(1, false, false));
        assert_eq!(a.risk, Risk::Possible);
        assert_eq!(a.causes, vec!["PHYSICAL_AND_VIRTUAL"]);
        assert_eq!(a.guidance[0], "RETURN_TO_NATIVE");
    }

    #[test]
    fn virtual_pad_with_steam_is_likely_and_guides_to_steam() {
        let a = assess(&facts(1, true, false));
        assert_eq!(a.risk, Risk::Likely);
        assert!(a.guidance.contains(&"DISABLE_STEAM_INPUT_FOR_CONTROLLER".to_string()));
    }

    #[test]
    fn hidden_physical_pad_removes_the_first_cause() {
        let a = assess(&facts(1, false, true));
        assert_eq!(a.risk, Risk::None);
    }
}
