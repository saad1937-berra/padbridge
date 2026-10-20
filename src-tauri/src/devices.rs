// src-tauri/src/devices.rs
pub mod model;
pub mod report_parser;

#[cfg(windows)]
pub mod hid_caps;
pub mod rumble;

#[cfg(all(feature = "hid", windows))]
pub mod enumeration;
