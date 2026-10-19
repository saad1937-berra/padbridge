// src-tauri/src/virtual_pad/backend.rs
//! Contrat entre le moteur et un backend de manette virtuelle (XInput).
//!
//! Aucun backend n'est supposé maintenu : voir `selected_backend.rs` et l'ADR (POC B).

use crate::devices::model::{Bit, BitSet};
use crate::error::AppResult;
use serde::{Deserialize, Serialize};

/// Boutons d'une manette de type Xbox 360 (nomenclature descriptive uniquement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum XButton {
    A,
    B,
    X,
    Y,
    LB,
    RB,
    Back,
    Start,
    Guide,
    LS,
    RS,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl XButton {
    pub fn label(self) -> &'static str {
        match self {
            XButton::A => "A",
            XButton::B => "B",
            XButton::X => "X",
            XButton::Y => "Y",
            XButton::LB => "LB",
            XButton::RB => "RB",
            XButton::Back => "Back",
            XButton::Start => "Start",
            XButton::Guide => "Guide",
            XButton::LS => "LS",
            XButton::RS => "RS",
            XButton::DpadUp => "DpadUp",
            XButton::DpadDown => "DpadDown",
            XButton::DpadLeft => "DpadLeft",
            XButton::DpadRight => "DpadRight",
        }
    }

    /// Masque `wButtons` de la structure XINPUT_GAMEPAD / rapport XUSB.
    pub fn xinput_mask(self) -> u16 {
        match self {
            XButton::DpadUp => 0x0001,
            XButton::DpadDown => 0x0002,
            XButton::DpadLeft => 0x0004,
            XButton::DpadRight => 0x0008,
            XButton::Start => 0x0010,
            XButton::Back => 0x0020,
            XButton::LS => 0x0040,
            XButton::RS => 0x0080,
            XButton::LB => 0x0100,
            XButton::RB => 0x0200,
            XButton::Guide => 0x0400,
            XButton::A => 0x1000,
            XButton::B => 0x2000,
            XButton::X => 0x4000,
            XButton::Y => 0x8000,
        }
    }
}

impl Bit for XButton {
    const ALL: &'static [Self] = &[
        XButton::A,
        XButton::B,
        XButton::X,
        XButton::Y,
        XButton::LB,
        XButton::RB,
        XButton::Back,
        XButton::Start,
        XButton::Guide,
        XButton::LS,
        XButton::RS,
        XButton::DpadUp,
        XButton::DpadDown,
        XButton::DpadLeft,
        XButton::DpadRight,
    ];

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

pub type XButtonSet = BitSet<XButton>;

/// État de sortie après mapping et zones mortes. Axes dans [-1, 1], gâchettes dans [0, 1].
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputState {
    pub buttons: XButtonSet,
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
}

impl OutputState {
    /// État neutre : aucune touche, axes au centre.
    pub fn neutral() -> Self {
        Self::default()
    }

    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    pub fn to_xinput(&self) -> XInputReport {
        let mut buttons = 0u16;
        for b in self.buttons.iter() {
            buttons |= b.xinput_mask();
        }
        XInputReport {
            buttons,
            left_trigger: trigger_byte(self.lt),
            right_trigger: trigger_byte(self.rt),
            thumb_lx: axis_i16(self.lx),
            thumb_ly: axis_i16(self.ly),
            thumb_rx: axis_i16(self.rx),
            thumb_ry: axis_i16(self.ry),
        }
    }
}

fn axis_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn trigger_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Disposition identique à XINPUT_GAMEPAD (pratique pour écrire un backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XInputReport {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub available: bool,
    pub name: String,
    pub code: Option<String>,
    pub reason: Option<String>,
}

pub trait VirtualPadBackend: Send {
    fn name(&self) -> &'static str;
    fn status(&self) -> BackendStatus;
    /// Crée au plus une sortie virtuelle liée à un appareil physique.
    fn create(&mut self, physical_id: &str) -> AppResult<SessionId>;
    fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()>;
    /// Doit libérer la sortie sans laisser de contrôleur fantôme (PB-018).
    fn destroy(&mut self, session: &SessionId) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xinput_conversion() {
        let mut s = OutputState::neutral();
        s.buttons.set(XButton::A, true);
        s.buttons.set(XButton::DpadUp, true);
        s.lx = 1.0;
        s.ly = -1.0;
        s.lt = 1.0;
        s.rt = 0.5;
        let r = s.to_xinput();
        assert_eq!(r.buttons, 0x1001);
        assert_eq!((r.thumb_lx, r.thumb_ly), (32767, -32767));
        assert_eq!((r.left_trigger, r.right_trigger), (255, 128));
    }

    #[test]
    fn neutral_is_neutral() {
        assert!(OutputState::neutral().is_neutral());
        let mut s = OutputState::neutral();
        s.rt = 0.1;
        assert!(!s.is_neutral());
    }
}
