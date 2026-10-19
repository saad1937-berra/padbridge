// src-tauri/src/diagnostics.rs
pub mod double_input;
pub mod events;
pub mod health;

use crate::devices::model::{ConnectionState, ControllerModel, Transport};
use crate::engine::ControllerView;
use crate::profiles::store::now_ms;
use crate::virtual_pad::backend::BackendStatus;
use double_input::DoubleInputAssessment;
use events::DiagnosticEvent;
use health::SystemReport;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub system: SystemReport,
    pub backend: BackendStatus,
    pub controllers: Vec<ControllerView>,
    pub double_input: DoubleInputAssessment,
    pub events: Vec<DiagnosticEvent>,
}

fn model_label(model: ControllerModel) -> &'static str {
    match model {
        ControllerModel::DualShock4 => "DualShock 4",
        ControllerModel::DualSense => "DualSense",
        ControllerModel::DualSenseEdge => "DualSense Edge",
        ControllerModel::Unknown => "inconnu",
    }
}

fn transport_label(t: Transport) -> &'static str {
    match t {
        Transport::Usb => "USB",
        Transport::Bluetooth => "Bluetooth",
        Transport::Unknown => "inconnu",
    }
}

fn state_label(s: ConnectionState) -> &'static str {
    match s {
        ConnectionState::Connected => "connectée",
        ConnectionState::Unstable => "connexion instable",
        ConnectionState::Unreadable => "illisible",
        ConnectionState::Disconnected => "déconnectée",
    }
}

fn tri(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "oui",
        Some(false) => "non",
        None => "inconnu",
    }
}

/// Texte exporté : volontairement sans VID/PID, identifiants d'appareil ni numéros de série.
pub fn export_text(report: &DiagnosticsReport) -> String {
    let mut out = String::new();
    out.push_str("PadBridge — diagnostic (export expurgé)\n");
    out.push_str(&format!("Généré (ms Unix) : {}\n\n", now_ms()));

    let s = &report.system;
    out.push_str("Système\n");
    out.push_str(&format!(
        "  Windows : {} (build {}), 11 : {}\n",
        s.os_version.as_deref().unwrap_or("inconnu"),
        s.windows_build.map(|b| b.to_string()).unwrap_or_else(|| "inconnu".into()),
        tri(s.is_windows_11)
    ));
    out.push_str(&format!("  Architecture : {}\n", s.arch));
    out.push_str(&format!("  Bluetooth présent : {}\n", tri(s.bluetooth_present)));
    out.push_str(&format!("  Droits élevés : {}\n", tri(s.elevated)));
    out.push_str(&format!("  Steam en cours : {}\n\n", tri(Some(s.steam_running))));

    out.push_str("Pilote de manette virtuelle\n");
    if report.backend.available {
        out.push_str(&format!("  disponible ({})\n\n", report.backend.name));
    } else {
        out.push_str(&format!(
            "  indisponible ({})\n\n",
            report.backend.code.as_deref().unwrap_or("?")
        ));
    }

    out.push_str(&format!("Double entrée : risque {:?}\n", report.double_input.risk).to_lowercase());
    out.push_str(&format!("  causes : {}\n\n", report.double_input.causes.join(", ")));

    out.push_str("Manettes\n");
    if report.controllers.is_empty() {
        out.push_str("  aucune\n");
    }
    for c in &report.controllers {
        let battery = match c.state.battery.and_then(|b| b.percent) {
            Some(p) => format!("{p} %"),
            None => "inconnue".into(),
        };
        out.push_str(&format!(
            "  - {} | {} | {} | mode {:?} | batterie {}\n",
            model_label(c.device.model),
            transport_label(c.device.transport),
            state_label(c.state.connection_state),
            c.mode,
            battery
        ));
    }

    out.push_str("\nÉvénements\n");
    for e in &report.events {
        out.push_str(&format!(
            "  [{}] {:?} {} {} — {}\n",
            e.timestamp_ms, e.severity, e.error_code, e.module, e.redacted_message
        ));
    }
    out
}
