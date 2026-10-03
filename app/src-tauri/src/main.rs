//! Oblivio desktop application entry point.

// No console window on Windows release builds (Windows is V2+).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    oblivio_app_lib::run();
}
