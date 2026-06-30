#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub(crate) async fn run_macos_menu_bar_entrypoint() -> crate::AppResult<()> {
    // ponytail: Phase 1 only wires the macOS platform boundary; the real
    // tray-icon/AppKit event loop lands when menu-bar behavior is implemented.
    crate::run_server_until_shutdown(None, crate::runtime::ActiveChatRunRegistry::default()).await
}
