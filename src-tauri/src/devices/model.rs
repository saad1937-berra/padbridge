// src-tauri/src/devices/model.rs
//! Types partagés : appareils, états, trames d'entrée normalisées.

use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use std::marker::PhantomData;

pub const SONY_VENDOR_ID: u16 = 0x054C;

// ---------------------------------------------------------------------------
// Ensemble de boutons (masque de bits sérialisé comme liste de noms)
// ---------------------------------------------------------------------------

pub trait Bit: Copy + 'static {
    const ALL: &'static [Self];
    fn bit(self) -> u32;
}

pub struct BitSet<T: Bit>(u32, PhantomData<T>);

impl<T: Bit> Clone for BitSet<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Bit> Copy for BitSet<T> {}
impl<T: Bit> Default for BitSet<T> {
    fn default() -> Self {
        Self(0, PhantomData)
    }
}
impl<T: Bit> PartialEq for BitSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: Bit> Eq for BitSet<T> {}
impl<T: Bit> std::fmt::Debug for BitSet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitSet({:#b})", self.0)
    }
}

impl<T: Bit> BitSet<T> {
    pub fn set(&mut self, item: T, on: bool) {
        if on {
            self.0 |= item.bit();
        } else {
            self.0 &= !item.bit();
        }
    }

    pub fn contains(&self, item: T) -> bool {
        self.0 & item.bit() != 0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        T::ALL.iter().copied().filter(move |b| self.contains(*b))
    }
}

impl<T: Bit + Serialize> Serialize for BitSet<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        for item in self.iter() {
            seq.serialize_element(&item)?;
        }
        seq.end()
    }
}

// ---------------------------------------------------------------------------
// Boutons PlayStation (L2/R2 sont analogiques : voir InputFrame)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsButton {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L3,
    R3,
    Share,
    Options,
    Ps,
    Touchpad,
    Mute,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl Bit for PsButton {
    const ALL: &'static [Self] = &[
        PsButton::Cross,
        PsButton::Circle,
        PsButton::Square,
        PsButton::Triangle,
        PsButton::L1,
        PsButton::R1,
        PsButton::L3,
        PsButton::R3,
        PsButton::Share,
        PsButton::Options,
        PsButton::Ps,
        PsButton::Touchpad,
        PsButton::Mute,
        PsButton::DpadUp,
        PsButton::DpadDown,
        PsButton::DpadLeft,
        PsButton::DpadRight,
    ];

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

pub type ButtonSet = BitSet<PsButton>;

// ---------------------------------------------------------------------------
// Appareils
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControllerModel {
    DualShock4,
    DualSense,
    DualSenseEdge,
    Unknown,
}

impl ControllerModel {
    pub fn from_ids(vendor_id: u16, product_id: u16) -> Self {
        if vendor_id != SONY_VENDOR_ID {
            return Self::Unknown;
        }
        match product_id {
            0x05C4 | 0x09CC => Self::DualShock4,
            0x0CE6 => Self::DualSense,
            0x0DF2 => Self::DualSenseEdge,
            _ => Self::Unknown,
        }
    }

    pub fn is_dualsense_family(self) -> bool {
        matches!(self, Self::DualSense | Self::DualSenseEdge)
    }

    pub fn capabilities(self) -> CapabilityFlags {
        match self {
            Self::DualShock4 => CapabilityFlags {
                touchpad: true,
                gyroscope: true,
                mute_button: false,
                adaptive_triggers: false,
                lightbar: true,
            },
            Self::DualSense | Self::DualSenseEdge => CapabilityFlags {
                touchpad: true,
                gyroscope: true,
                mute_button: true,
                adaptive_triggers: true,
                lightbar: true,
            },
            Self::Unknown => CapabilityFlags::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Usb,
    Bluetooth,
    Unknown,
}

/// Capacités matérielles du modèle (et non fonctions déjà implémentées par PadBridge).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityFlags {
    pub touchpad: bool,
    pub gyroscope: bool,
    pub mute_button: bool,
    pub adaptive_triggers: bool,
    pub lightbar: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerDevice {
    /// Identifiant local pseudonyme (hash du chemin HID) — jamais le numéro de série.
    pub device_id: String,
    pub model: ControllerModel,
    pub transport: Transport,
    pub vendor_id: u16,
    pub product_id: u16,
    pub capability_flags: CapabilityFlags,
    pub last_seen_ms: u64,
}

impl ControllerDevice {
    pub fn new(device_id: String, vendor_id: u16, product_id: u16, transport: Transport) -> Self {
        let model = ControllerModel::from_ids(vendor_id, product_id);
        Self {
            device_id,
            model,
            transport,
            vendor_id,
            product_id,
            capability_flags: model.capabilities(),
            last_seen_ms: 0,
        }
    }
}

/// Identifiant pseudonyme stable pour un chemin HID donné (FNV-1a 64 bits, 16 hex).
pub fn device_id_from_path(path: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in path {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionState {
    Connected,
    /// Reconnexions répétées dans un court laps de temps.
    Unstable,
    /// Détectée par Windows mais impossible à lire (accès, clone, etc.).
    Unreadable,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    /// `None` quand la donnée n'est pas fiable : l'interface affiche « Inconnue », jamais 0 %.
    pub percent: Option<u8>,
    pub charging: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerState {
    pub device_id: String,
    pub connection_state: ConnectionState,
    pub battery: Option<Battery>,
    pub last_input_at_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Trame d'entrée normalisée
// ---------------------------------------------------------------------------

/// Axe analogique normalisé : x vers la droite (+), y vers le haut (+), plage [-1, 1].
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Stick {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InputFrame {
    pub seq: u64,
    /// Horloge monotone (microsecondes depuis le démarrage du moteur).
    pub timestamp_us: u64,
    pub buttons: ButtonSet,
    pub left: Stick,
    pub right: Stick,
    /// Gâchettes analogiques, plage [0, 1].
    pub l2: f32,
    pub r2: f32,
    pub battery: Option<Battery>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_roundtrip() {
        let mut set = ButtonSet::default();
        set.set(PsButton::Cross, true);
        set.set(PsButton::DpadLeft, true);
        assert!(set.contains(PsButton::Cross));
        assert!(!set.contains(PsButton::Circle));
        set.set(PsButton::Cross, false);
        let pressed: Vec<_> = set.iter().collect();
        assert_eq!(pressed, vec![PsButton::DpadLeft]);
    }

    #[test]
    fn bitset_serializes_as_names() {
        let mut set = ButtonSet::default();
        set.set(PsButton::L1, true);
        set.set(PsButton::DpadUp, true);
        assert_eq!(serde_json::to_string(&set).unwrap(), r#"["l1","dpad_up"]"#);
    }

    #[test]
    fn models_from_ids() {
        assert_eq!(ControllerModel::from_ids(0x054C, 0x09CC), ControllerModel::DualShock4);
        assert_eq!(ControllerModel::from_ids(0x054C, 0x0CE6), ControllerModel::DualSense);
        assert_eq!(ControllerModel::from_ids(0x054C, 0x0DF2), ControllerModel::DualSenseEdge);
        assert_eq!(ControllerModel::from_ids(0x045E, 0x028E), ControllerModel::Unknown);
    }

    #[test]
    fn device_id_is_stable_and_pseudonymous() {
        let a = device_id_from_path(b"\\\\?\\hid#vid_054c&pid_0ce6#a");
        let b = device_id_from_path(b"\\\\?\\hid#vid_054c&pid_0ce6#a");
        let c = device_id_from_path(b"\\\\?\\hid#vid_054c&pid_0ce6#b");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
    }
}
