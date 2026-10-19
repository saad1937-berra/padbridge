// src-tauri/src/diagnostics/health.rs
//! Prérequis système (PB-001). Commandes fixes, sans entrée utilisateur, exécutées à la demande.
//! Toute valeur indisponible reste `None` : l'interface affiche « Inconnu ».

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemReport {
    pub os: String,
    pub arch: String,
    pub os_version: Option<String>,
    pub windows_build: Option<u32>,
    pub is_windows_11: Option<bool>,
    pub bluetooth_present: Option<bool>,
    pub elevated: Option<bool>,
    pub steam_running: bool,
}

pub fn collect() -> SystemReport {
    let version = run_hidden("cmd", &["/C", "ver"]).and_then(|out| parse_ver_output(&out));
    SystemReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        os_version: version.as_ref().map(|(v, _)| v.clone()),
        windows_build: version.as_ref().map(|(_, b)| *b),
        is_windows_11: version.as_ref().map(|(_, b)| *b >= 22000),
        bluetooth_present: bluetooth_present(),
        elevated: is_elevated(),
        steam_running: steam_running(),
    }
}

pub fn steam_running() -> bool {
    run_hidden("tasklist", &["/FI", "IMAGENAME eq steam.exe", "/NH"])
        .map(|out| out.to_lowercase().contains("steam.exe"))
        .unwrap_or(false)
}

fn bluetooth_present() -> Option<bool> {
    let out = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-PnpDevice -Class Bluetooth -PresentOnly -ErrorAction SilentlyContinue | Measure-Object).Count",
        ],
    )?;
    out.trim().parse::<u32>().ok().map(|n| n > 0)
}

fn is_elevated() -> Option<bool> {
    // S-1-16-12288 = niveau d'intégrité « élevé ».
    run_hidden("whoami", &["/groups"]).map(|out| out.contains("S-1-16-12288"))
}

#[cfg(windows)]
fn run_hidden(program: &str, args: &[&str]) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(windows))]
fn run_hidden(_program: &str, _args: &[&str]) -> Option<String> {
    None // ces vérifications n'existent que sous Windows
}

/// « Microsoft Windows [Version 10.0.22631.4602] » → ("10.0.22631.4602", 22631)
pub fn parse_ver_output(out: &str) -> Option<(String, u32)> {
    let start = out.find("Version ")? + "Version ".len();
    let end = out[start..].find(']')? + start;
    let version = out[start..end].trim().to_string();
    let build = version.split('.').nth(2)?.parse::<u32>().ok()?;
    Some((version, build))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_11_version() {
        let (v, build) =
            parse_ver_output("\r\nMicrosoft Windows [Version 10.0.22631.4602]\r\n").unwrap();
        assert_eq!(v, "10.0.22631.4602");
        assert_eq!(build, 22631);
    }

    #[test]
    fn garbage_gives_none() {
        assert!(parse_ver_output("hello").is_none());
        assert!(parse_ver_output("[Version abc]").is_none());
    }
}
