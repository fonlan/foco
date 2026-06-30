use std::net::{IpAddr, SocketAddr};

use foco_store::config::SUPPORTED_APP_LANGUAGES;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TrayMenuLabels {
    pub(crate) open: &'static str,
    pub(crate) open_logs: &'static str,
    pub(crate) open_config_folder: &'static str,
    pub(crate) about: &'static str,
    pub(crate) quit: &'static str,
}
#[cfg(all(windows, not(debug_assertions)))]
pub(crate) async fn run_platform_tray_entrypoint() -> crate::AppResult<()> {
    crate::platform::tray_windows::run_windows_tray_entrypoint()
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub(crate) async fn run_platform_tray_entrypoint() -> crate::AppResult<()> {
    crate::platform::tray_macos::run_macos_menu_bar_entrypoint().await
}

#[cfg(all(any(windows, target_os = "macos"), not(debug_assertions)))]
pub(crate) fn open_foco_ui(ui_url: &str) {
    if let Err(error) = webbrowser::open(ui_url) {
        tracing::warn!(%ui_url, error = %error, "failed to open Foco web UI");
    }
}

pub(crate) fn tray_menu_labels(language: &str) -> Result<TrayMenuLabels, String> {
    match language {
        "zh-CN" => Ok(TrayMenuLabels {
            open: "打开 Foco",
            open_logs: "打开日志",
            open_config_folder: "打开配置文件夹",
            about: "关于 Foco",
            quit: "退出 Foco",
        }),
        "en" => Ok(TrayMenuLabels {
            open: "Open Foco",
            open_logs: "Open Logs",
            open_config_folder: "Open Config Folder",
            about: "About Foco",
            quit: "Quit Foco",
        }),
        _ => Err(format!(
            "app language '{language}' is unsupported; expected one of {}",
            SUPPORTED_APP_LANGUAGES.join(", ")
        )),
    }
}

pub(crate) fn browser_addr_for_listen_addr(addr: SocketAddr) -> SocketAddr {
    let host = match addr.ip() {
        IpAddr::V4(ip) if ip.octets() == [0, 0, 0, 0] => IpAddr::from([127, 0, 0, 1]),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        ip => ip,
    };

    SocketAddr::from((host, addr.port()))
}

pub(crate) fn foco_ui_url_for_listen_addr(addr: SocketAddr) -> String {
    format!("http://{}", browser_addr_for_listen_addr(addr))
}

pub(crate) fn open_foco_ui_if_listener_bound(
    listener_bound: bool,
    addr: SocketAddr,
    open_ui: impl FnOnce(&str),
) -> bool {
    if !listener_bound {
        return false;
    }

    let ui_url = foco_ui_url_for_listen_addr(addr);
    open_ui(&ui_url);
    true
}
