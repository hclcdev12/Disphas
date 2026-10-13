//! disphas desktop application entry point.

// No console window on Windows release builds (Windows is V2+).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    disphas_app_lib::run();
}
