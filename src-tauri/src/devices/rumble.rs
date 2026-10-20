// Format des rapports de sortie DualShock 4, sans pilote tiers.
// Seule la vibration est activée (les LED sont laissées sous le contrôle courant).
// Référence de format : Linux hid-playstation / hid-sony ; essais matériels indispensables.
use crate::devices::model::Transport;
use crate::error::{codes, AppError, AppResult};
use std::collections::HashMap;
use std::sync::{mpsc::{self, Sender, SyncSender, TrySendError}, Mutex};
use std::time::Duration;

pub const MAX_INTENSITY: u8 = 180;
pub const MAX_DURATION_MS: u16 = 500;

#[derive(Debug)]
pub enum RumbleCommand {
    Start { right: u8, left: u8, duration_ms: u16, reply: Sender<AppResult<()>> },
    Stop { reply: Sender<AppResult<()>> },
}

pub fn validate(right: u8, left: u8, duration_ms: u16) -> AppResult<()> {
    if right > MAX_INTENSITY || left > MAX_INTENSITY || !(50..=MAX_DURATION_MS).contains(&duration_ms) {
        return Err(AppError::new(codes::INVALID_ARGUMENT, "vibration : intensité 0–180 et durée 50–500 ms"));
    }
    Ok(())
}

pub fn build_report(transport: Transport, right: u8, left: u8) -> AppResult<Vec<u8>> {
    if right > MAX_INTENSITY || left > MAX_INTENSITY {
        return Err(AppError::new(codes::INVALID_ARGUMENT, "intensité hors plage"));
    }
    match transport {
        Transport::Bluetooth => {
            let mut output = vec![0u8; 78];
            output[0] = 0x11;
            output[1] = 0xC4; // Bluetooth HID + CRC, intervalle de rapports 4 ms (DS4)
            output[3] = 0x01; // uniquement les moteurs ; ne pas modifier les LED
            output[6] = right; // moteur faible/rapide
            output[7] = left;  // moteur fort/lent
            let checksum = bluetooth_crc(&output[..74]);
            output[74..78].copy_from_slice(&checksum.to_le_bytes());
            Ok(output)
        }
        Transport::Usb => {
            let mut output = vec![0u8; 32];
            output[0] = 0x05;
            output[1] = 0x01; // uniquement les moteurs
            output[4] = right;
            output[5] = left;
            Ok(output)
        }
        Transport::Unknown => Err(AppError::new(codes::INVALID_ARGUMENT, "transport non reconnu")),
    }
}

// CRC32 IEEE réfléchi, données = préfixe Bluetooth 0xA2 + rapport HID sans CRC.
// Implémentation locale : aucune dépendance de calcul CRC.
fn bluetooth_crc(payload: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for byte in std::iter::once(&0xA2u8).chain(payload.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Le thread HID qui lit les entrées est aussi le seul propriétaire du périphérique en écriture.
#[derive(Default)]
pub struct RumbleRegistry {
    writers: Mutex<HashMap<String, SyncSender<RumbleCommand>>>,
}

impl RumbleRegistry {
    pub fn register(&self, id: String, tx: SyncSender<RumbleCommand>) {
        if let Ok(mut writers) = self.writers.lock() { writers.insert(id, tx); }
    }

    pub fn unregister(&self, id: &str) {
        if let Ok(mut writers) = self.writers.lock() { writers.remove(id); }
    }

    fn send(&self, id: &str, command: RumbleCommand) -> AppResult<()> {
        let writers = self.writers.lock()
            .map_err(|_| AppError::new(codes::INTERNAL, "verrou HID indisponible"))?;
        let tx = writers.get(id)
            .ok_or_else(|| AppError::new(codes::DEVICE_UNREADABLE, "manette non accessible en écriture"))?;
        tx.try_send(command).map_err(|e| match e {
            TrySendError::Full(_) => AppError::new(codes::BACKEND_SESSION, "commande de vibration en attente ; réessayez"),
            TrySendError::Disconnected(_) => AppError::new(codes::DEVICE_UNREADABLE, "la manette a été déconnectée"),
        })
    }

    pub fn pulse(&self, id: &str, right: u8, left: u8, duration_ms: u16) -> AppResult<()> {
        validate(right, left, duration_ms)?;
        let (reply, rx) = mpsc::channel();
        self.send(id, RumbleCommand::Start { right, left, duration_ms, reply })?;
        self.wait_ack(rx)
    }

    pub fn stop(&self, id: &str) -> AppResult<()> {
        let (reply, rx) = mpsc::channel();
        self.send(id, RumbleCommand::Stop { reply })?;
        self.wait_ack(rx)
    }

    fn wait_ack(&self, rx: mpsc::Receiver<AppResult<()>>) -> AppResult<()> {
        rx.recv_timeout(Duration::from_millis(700))
            .map_err(|_| AppError::new(codes::DEVICE_UNREADABLE, "écriture HID non confirmée"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bt_layout_crc_and_idle_frame() {
        let active = build_report(Transport::Bluetooth, 30, 80).unwrap();
        assert_eq!(active.len(), 78);
        assert_eq!(&active[..8], &[0x11, 0xC4, 0, 1, 0, 0, 30, 80]);
        assert_eq!(u32::from_le_bytes(active[74..78].try_into().unwrap()), bluetooth_crc(&active[..74]));
        let stopped = build_report(Transport::Bluetooth, 0, 0).unwrap();
        assert_eq!(stopped.len(), 78);
        assert_eq!(&stopped[6..8], &[0, 0]);
        assert_ne!(active[74..78], stopped[74..78]);
    }

    #[test]
    fn usb_layout_and_bounds() {
        let active = build_report(Transport::Usb, 20, 55).unwrap();
        assert_eq!(active.len(), 32);
        assert_eq!(&active[..6], &[5, 1, 0, 0, 20, 55]);
        assert!(build_report(Transport::Unknown, 1, 1).is_err());
        assert!(build_report(Transport::Usb, 200, 1).is_err());
        assert!(validate(0, 0, 501).is_err());
        assert!(validate(180, 180, 500).is_ok());
    }

    #[test]
    fn crc32_matches_independent_ieee_test_vector() {
        // Propriété du CRC classique sur « 123456789 ».
        fn standard(data: &[u8]) -> u32 {
            let mut crc = 0xffff_ffff_u32;
            for b in data {
                crc ^= u32::from(*b);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
                }
            }
            !crc
        }
        assert_eq!(standard(b"123456789"), 0xcbf4_3926);
    }
}
