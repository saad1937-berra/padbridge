// src-tauri/src/pipeline/mapping.rs
//! Profil « compilé » : table de mapping + zones mortes prêtes pour le temps réel.
//! Un changement de profil remplace l'objet entier (pas de mélange ancien/nouveau).

use super::deadzone;
use crate::devices::model::{Bit, InputFrame, PsButton};
use crate::profiles::schema::{Deadzones, Profile, ValidationIssue, ValidationReport};
use crate::virtual_pad::backend::{OutputState, XButton};

#[derive(Debug, Clone)]
pub struct CompiledProfile {
    profile_id: String,
    table: Vec<Option<XButton>>,
    deadzones: Deadzones,
    warnings: Vec<ValidationIssue>,
}

impl CompiledProfile {
    /// Refuse les profils invalides ; conserve les avertissements (actions inaccessibles).
    pub fn compile(profile: &Profile) -> Result<Self, ValidationReport> {
        let report = profile.validate();
        if !report.is_valid() {
            return Err(report);
        }
        let effective = profile.effective_mapping();
        let table = PsButton::ALL
            .iter()
            .map(|source| effective.get(source).copied())
            .collect();
        Ok(Self {
            profile_id: profile.id.clone(),
            table,
            deadzones: profile.deadzones,
            warnings: report.warnings,
        })
    }

    pub fn id(&self) -> &str {
        &self.profile_id
    }

    pub fn warnings(&self) -> &[ValidationIssue] {
        &self.warnings
    }

    pub fn apply(&self, frame: &InputFrame) -> OutputState {
        let mut out = OutputState::neutral();
        for source in frame.buttons.iter() {
            if let Some(target) = self.table[source as usize] {
                out.buttons.set(target, true);
            }
        }
        let left = deadzone::radial(frame.left, self.deadzones.left_stick);
        let right = deadzone::radial(frame.right, self.deadzones.right_stick);
        out.lx = left.x;
        out.ly = left.y;
        out.rx = right.x;
        out.ry = right.y;
        out.lt = deadzone::trigger(frame.l2, self.deadzones.left_trigger);
        out.rt = deadzone::trigger(frame.r2, self.deadzones.right_trigger);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::model::{ButtonSet, Stick};

    fn frame_with(buttons: &[PsButton]) -> InputFrame {
        let mut set = ButtonSet::default();
        for b in buttons {
            set.set(*b, true);
        }
        InputFrame { buttons: set, ..InputFrame::default() }
    }

    #[test]
    fn standard_preset_maps_face_buttons() {
        let c = CompiledProfile::compile(&Profile::default_profile()).unwrap();
        let out = c.apply(&frame_with(&[PsButton::Cross, PsButton::Options, PsButton::DpadLeft]));
        let pressed: Vec<_> = out.buttons.iter().collect();
        assert_eq!(pressed, vec![XButton::A, XButton::Start, XButton::DpadLeft]);
    }

    #[test]
    fn swap_cross_and_circle() {
        let mut p = Profile::default_profile();
        p.mapping.insert(PsButton::Cross, XButton::B);
        p.mapping.insert(PsButton::Circle, XButton::A);
        let c = CompiledProfile::compile(&p).unwrap();
        assert!(c.warnings().is_empty());
        let out = c.apply(&frame_with(&[PsButton::Cross]));
        assert!(out.buttons.contains(XButton::B) && !out.buttons.contains(XButton::A));
    }

    #[test]
    fn unmapped_sources_produce_nothing() {
        let c = CompiledProfile::compile(&Profile::default_profile()).unwrap();
        let out = c.apply(&frame_with(&[PsButton::Touchpad, PsButton::Mute]));
        assert!(out.buttons.is_empty());
    }

    #[test]
    fn deadzone_changes_the_output_not_the_input() {
        let frame = InputFrame { left: Stick { x: 0.1, y: 0.0 }, ..InputFrame::default() };
        let mut p = Profile::default_profile();
        p.deadzones.left_stick = 0.05;
        assert!(CompiledProfile::compile(&p).unwrap().apply(&frame).lx > 0.0);
        p.deadzones.left_stick = 0.2;
        assert_eq!(CompiledProfile::compile(&p).unwrap().apply(&frame).lx, 0.0);
        assert_eq!(frame.left.x, 0.1);
    }

    #[test]
    fn invalid_profile_does_not_compile() {
        let mut p = Profile::default_profile();
        p.deadzones.left_stick = 0.9;
        assert!(CompiledProfile::compile(&p).is_err());
    }
}
