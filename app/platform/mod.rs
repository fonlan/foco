pub mod autostart_windows;
pub mod native_browser;
#[cfg(any(test, all(any(windows, target_os = "macos"), not(debug_assertions))))]
pub mod tray;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub mod tray_macos;
#[cfg(all(windows, not(debug_assertions)))]
pub mod tray_windows;
