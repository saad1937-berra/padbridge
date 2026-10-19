// src-tauri/src/diagnostics/events.rs
//! Journal d'événements techniques en mémoire (rotation par taille) avec expurgation.
//! Aucun contenu de jeu, aucune touche système, aucun numéro de série.

use crate::profiles::store::now_ms;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub timestamp_ms: u64,
    pub severity: Severity,
    pub error_code: String,
    pub module: String,
    pub redacted_message: String,
}

pub struct EventLog {
    capacity: usize,
    events: Mutex<VecDeque<DiagnosticEvent>>,
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, events: Mutex::new(VecDeque::new()) }
    }

    pub fn push(&self, severity: Severity, code: &str, module: &str, message: &str) {
        let redacted = redact(message);
        match severity {
            Severity::Info => tracing::info!(code, module, "{redacted}"),
            Severity::Warning => tracing::warn!(code, module, "{redacted}"),
            Severity::Error => tracing::error!(code, module, "{redacted}"),
        }
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() >= self.capacity {
            events.pop_front();
        }
        events.push_back(DiagnosticEvent {
            timestamp_ms: now_ms(),
            severity,
            error_code: code.to_string(),
            module: module.to_string(),
            redacted_message: redacted,
        });
    }

    pub fn snapshot(&self) -> Vec<DiagnosticEvent> {
        let events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        events.iter().cloned().collect()
    }
}

/// Masque chemins HID, dossiers utilisateur et identifiants hexadécimaux longs.
pub fn redact(message: &str) -> String {
    message
        .split_whitespace()
        .map(|token| {
            let lower = token.to_lowercase();
            if token.starts_with("\\\\?\\") || lower.contains("hid#") || lower.contains("usb#") {
                "[chemin-masqué]".to_string()
            } else if lower.contains("\\users\\") || lower.contains("/users/") || lower.contains("/home/") {
                "[chemin-masqué]".to_string()
            } else if token.len() >= 16 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
                "[id-masqué]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_paths_and_ids() {
        let out = redact(
            r"ouverture de \\?\hid#vid_054c&pid_0ce6#7&abc échec pour C:\Users\Saad\AppData id 00112233445566ff ok",
        );
        assert!(!out.contains("vid_054c"));
        assert!(!out.contains("Saad"));
        assert!(!out.contains("00112233445566ff"));
        assert!(out.contains("ouverture") && out.contains("ok"));
    }

    #[test]
    fn ring_buffer_is_bounded() {
        let log = EventLog::new(3);
        for i in 0..10 {
            log.push(Severity::Info, "PB-I000", "test", &format!("e{i}"));
        }
        let events = log.snapshot();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].redacted_message, "e7");
    }
}
