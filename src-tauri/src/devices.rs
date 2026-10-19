// src-tauri/src/devices.rs
pub mod model;
pub mod report_parser;

#[cfg(all(feature = "hid", windows))]
pub mod enumeration;
