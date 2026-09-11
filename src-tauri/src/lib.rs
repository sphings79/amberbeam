//! Desktop shell of AmberBeam.
//!
//! This crate is deliberately thin. It owns the window and turns Tauri commands
//! into calls on `amberbeam-core`, and it holds no logic of its own. The same
//! set of commands is what a headless HTTP/WebSocket service will expose in
//! milestone M7 — which only works as long as nothing of substance settles here.

use amberbeam_core::CoreInfo;

/// Version and capabilities of the core behind this window.
#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo::gather()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
