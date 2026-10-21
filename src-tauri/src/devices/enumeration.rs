// src-tauri/src/devices/enumeration.rs
//! Découverte (interrogation toutes les secondes) et lecture HID des manettes Sony.
//! Windows uniquement. Un fil par manette ; aucun accès au processus d'un jeu.

use super::model::{device_id_from_path, ControllerDevice, ControllerModel, Transport, SONY_VENDOR_ID};
use crate::diagnostics::events::Severity;
use crate::engine::Engine;
use crate::devices::rumble::{build_report, ensure_output_allowed, pad_windows_bt_output, RumbleCommand, RumbleRegistry, T14_OUTPUT_ENABLED};
use hidapi::{BusType, HidApi, HidDevice};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Nouvelle tentative d'ouverture d'un appareil illisible : toutes les N interrogations.
const RETRY_UNREADABLE_EVERY: u32 = 5;
const GAMEPAD_USAGE_PAGE: u16 = 0x01;
const GAMEPAD_USAGE: u16 = 0x05;

pub struct HidMonitor {
    handle: Option<JoinHandle<()>>,
}

impl HidMonitor {
    /// Attend l'arrêt du fil de surveillance (le drapeau `stop` doit être levé avant).
    pub fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

enum Tracked {
    Reading { stop: Arc<AtomicBool>, handle: JoinHandle<()> },
    Unreadable { ticks: u32 },
}

pub fn spawn(engine: Arc<Engine>, stop: Arc<AtomicBool>, rumble: Arc<RumbleRegistry>) -> HidMonitor {
    let handle = thread::Builder::new()
        .name("padbridge-hid-monitor".into())
        .spawn(move || monitor_loop(engine, stop, rumble))
        .ok();
    HidMonitor { handle }
}

fn is_target(info: &hidapi::DeviceInfo) -> bool {
    if info.vendor_id() != SONY_VENDOR_ID {
        return false;
    }
    if ControllerModel::from_ids(info.vendor_id(), info.product_id()) == ControllerModel::Unknown {
        return false;
    }
    // Évite de doublonner les interfaces HID multiples d'un même appareil (PB-005).
    let usage_known = info.usage_page() != 0;
    !usage_known || (info.usage_page() == GAMEPAD_USAGE_PAGE && info.usage() == GAMEPAD_USAGE)
}

fn transport_of(info: &hidapi::DeviceInfo) -> Transport {
    match info.bus_type() {
        BusType::Usb => Transport::Usb,
        BusType::Bluetooth => Transport::Bluetooth,
        _ => Transport::Unknown,
    }
}

fn monitor_loop(engine: Arc<Engine>, stop: Arc<AtomicBool>, rumble: Arc<RumbleRegistry>) {
    let mut api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            engine.events().push(
                Severity::Error,
                crate::error::codes::DEVICE_UNREADABLE,
                "devices",
                &format!("initialisation HID impossible : {e}"),
            );
            return;
        }
    };
    let mut tracked: HashMap<String, Tracked> = HashMap::new();

    while !stop.load(Ordering::Relaxed) {
        if api.refresh_devices().is_ok() {
            let mut present: HashMap<String, (CString, ControllerDevice)> = HashMap::new();
            for info in api.device_list().filter(|i| is_target(i)) {
                let id = device_id_from_path(info.path().to_bytes());
                let device = ControllerDevice::new(
                    id.clone(),
                    info.vendor_id(),
                    info.product_id(),
                    transport_of(info),
                );
                present.entry(id).or_insert((info.path().to_owned(), device));
            }

            // Appareils disparus ou lecteurs terminés.
            let gone: Vec<String> = tracked
                .iter()
                .filter(|(id, t)| {
                    !present.contains_key(*id)
                        || matches!(t, Tracked::Reading { handle, .. } if handle.is_finished())
                })
                .map(|(id, _)| id.clone())
                .collect();
            for id in gone {
                if let Some(Tracked::Reading { stop, handle }) = tracked.remove(&id) {
                    stop.store(true, Ordering::Relaxed);
                    let _ = handle.join();
                }
                engine.device_lost(&id);
            }

            // Nouveaux appareils (et nouvelles tentatives pour les illisibles).
            for (id, (path, device)) in present {
                match tracked.get_mut(&id) {
                    Some(Tracked::Reading { .. }) => continue,
                    Some(Tracked::Unreadable { ticks }) => {
                        *ticks += 1;
                        if *ticks % RETRY_UNREADABLE_EVERY != 0 {
                            continue;
                        }
                    }
                    None => {}
                }
                match api.open_path(&path) {
                    Ok(dev) => {
                        // Windows attend un tampon HIDP_CAPS.OutputReportByteLength pour
                        // HidD_SetOutputReport, même si le rapport DS4 0x11 est plus court.
                        let windows_bt_output_cap = if device.model == ControllerModel::DualShock4
                            && device.transport == Transport::Bluetooth
                        {
                            match crate::devices::hid_caps::output_report_byte_length(&path.to_string_lossy()) {
                                Ok(cap) => Some(cap),
                                Err(e) => {
                                    engine.events().push(
                                        Severity::Warning,
                                        crate::error::codes::DEVICE_UNREADABLE,
                                        "rumble",
                                        &format!("capacités HID Windows indisponibles : {e}"),
                                    );
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        engine.device_connected(device.clone(), true);
                        let reader_stop = Arc::new(AtomicBool::new(false));
                        let handle = spawn_reader(
                            engine.clone(),
                            id.clone(),
                            device,
                            dev,
                            reader_stop.clone(),
                            stop.clone(),
                            rumble.clone(),
                            windows_bt_output_cap,
                        );
                        tracked.insert(id, Tracked::Reading { stop: reader_stop, handle });
                    }
                    Err(_) => {
                        if !matches!(tracked.get(&id), Some(Tracked::Unreadable { .. })) {
                            engine.device_connected(device, false);
                        }
                        tracked.entry(id).or_insert(Tracked::Unreadable { ticks: 0 });
                    }
                }
            }
        }
        sleep_interruptible(&stop, POLL_INTERVAL);
    }

    for (id, t) in tracked.drain() {
        if let Tracked::Reading { stop, handle } = t {
            stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
        engine.device_lost(&id);
    }
}

fn sleep_interruptible(stop: &AtomicBool, total: Duration) {
    let step = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    while waited < total && !stop.load(Ordering::Relaxed) {
        thread::sleep(step);
        waited += step;
    }
}

fn spawn_reader(
    engine: Arc<Engine>,
    id: String,
    device: ControllerDevice,
    dev: HidDevice,
    stop: Arc<AtomicBool>,
    global_stop: Arc<AtomicBool>,
    rumble: Arc<RumbleRegistry>,
    windows_bt_output_cap: Option<u16>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("padbridge-hid-reader".into())
        .spawn(move || {
            // En Bluetooth, Windows livre un rapport « simple » tant que le rapport de
            // fonctionnalité n'a pas été lu : on le lit pour obtenir le rapport complet
            // (batterie, gâchettes complètes). Les erreurs sont ignorées.
            if device.transport == Transport::Bluetooth {
                if device.model.is_dualsense_family() {
                    let mut buf = [0u8; 41];
                    buf[0] = 0x05;
                    let _ = dev.get_feature_report(&mut buf);
                } else {
                    let mut buf = [0u8; 37];
                    buf[0] = 0x02;
                    let _ = dev.get_feature_report(&mut buf);
                }
            }

            // Le handle HID appartient à ce fil : lecture ET écriture ne se chevauchent pas.
            let (tx, rx) = mpsc::sync_channel(2);
            rumble.register(id.clone(), tx);
            let mut rumble_deadline: Option<Instant> = None;
            let mut active_reply: Option<mpsc::Sender<crate::error::AppResult<()>>> = None;
            let mut buf = [0u8; 128];
            let mut parse_errors: u64 = 0;
            while !stop.load(Ordering::Relaxed) && !global_stop.load(Ordering::Relaxed) {
                // Une seule voie d'écriture par périphérique : le lecteur HID.
                // En Bluetooth sous Windows, on utilise WriteFile. Certains
                // adaptateurs Bluetooth peuvent accepter sans transmettre : test
                // physique requis même si la commande HID retourne Ok.
                let send_motors = |right: u8, left: u8| -> crate::error::AppResult<()> {
                    ensure_output_allowed()?; // No HID writes while T14 is suspended.
                    let packet = build_report(device.transport, right, left)?;
                    if device.transport == Transport::Bluetooth {
                        // HidD_SetOutputReport échoue avec ERROR_INVALID_PARAMETER (87)
                        // sur cette DS4 sous Windows. Utiliser le chemin WriteFile,
                        // recommandé pour les rapports HID de sortie.
                        // Une écriture acceptée ne garantit PAS une vibration physique.
                        let padded = pad_windows_bt_output(packet, windows_bt_output_cap)?;
                        let expected = padded.len();
                        let written = dev.write(&padded).map_err(|e| {
                            crate::error::AppError::new(
                                crate::error::codes::DEVICE_UNREADABLE,
                                format!("WriteFile Bluetooth ({expected} octets) : {e}")
                            )
                        })?;
                        if written != expected {
                            return Err(crate::error::AppError::new(
                                crate::error::codes::DEVICE_UNREADABLE,
                                format!("WriteFile Bluetooth partiel : {written}/{expected} octets")
                            ));
                        }
                        Ok(())
                    } else {
                        dev.write(&packet).map(|_| ()).map_err(|e| {
                            crate::error::AppError::new(
                                crate::error::codes::DEVICE_UNREADABLE,
                                format!("WriteFile USB : {e}")
                            )
                        })
                    }
                };

                if rumble_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    let result = send_motors(0, 0);
                    if let Err(e) = &result {
                        engine.events().push(Severity::Warning, e.code, "rumble", &e.detail);
                    }
                    rumble_deadline = None;
                    if let Some(reply) = active_reply.take() {
                        let _ = reply.send(result);
                    }
                }
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        RumbleCommand::Start { right, left, duration_ms, reply } => {
                            if rumble_deadline.is_some() {
                                let _ = reply.send(Err(crate::error::AppError::new(
                                    crate::error::codes::BACKEND_SESSION, "une vibration est déjà en cours"
                                )));
                                continue;
                            }
                            match send_motors(right, left) {
                                Ok(()) => {
                                    rumble_deadline = Some(Instant::now() + Duration::from_millis(u64::from(duration_ms)));
                                    // Accusé envoyé seulement APRES la commande neutre,
                                    // pour que l'UI suive la durée demandée.
                                    active_reply = Some(reply);
                                }
                                Err(e) => {
                                    let _ = reply.send(Err(e));
                                }
                            }
                        }
                        RumbleCommand::Stop { reply } => {
                            let result = send_motors(0, 0);
                            rumble_deadline = None;
                            if let Some(active) = active_reply.take() {
                                let _ = active.send(result.clone());
                            }
                            let _ = reply.send(result);
                        }
                    }
                }
                match dev.read_timeout(&mut buf, 25) {
                    Ok(0) => continue,
                    Ok(n) => {
                        if let Err(e) = engine.ingest_report(&id, &buf[..n]) {
                            parse_errors += 1;
                            // Journalise la première erreur puis une sur 1000.
                            if parse_errors == 1 || parse_errors % 1000 == 0 {
                                engine.events().push(
                                    Severity::Warning,
                                    e.code,
                                    "devices",
                                    &format!("{} (occurrence {parse_errors})", e.detail),
                                );
                            }
                        }
                    }
                    Err(_) => break, // appareil retiré ou erreur fatale
                }
            }
            // Arrêt explicite, autant que possible, à la perte de connexion
            // ou à la fermeture de l'application.
            rumble.unregister(&id);
            if T14_OUTPUT_ENABLED && rumble_deadline.is_some() {
                if let Ok(output) = build_report(device.transport, 0, 0) {
                    if device.transport == Transport::Bluetooth {
                        if let Ok(padded) = pad_windows_bt_output(output, windows_bt_output_cap) {
                            let _ = dev.write(&padded);
                        }
                    } else {
                        let _ = dev.write(&output);
                    }
                }
            }
            if let Some(reply) = active_reply.take() {
                let _ = reply.send(Err(crate::error::AppError::new(
                    crate::error::codes::DEVICE_UNREADABLE,
                    "connexion interrompue avant la fin de la vibration",
                )));
            }
            engine.device_lost(&id);
        })
        .expect("création du fil de lecture")
}
