#[cfg(all(target_os = "macos", not(debug_assertions)))]
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::Command,
    time::Duration,
};

#[cfg(all(target_os = "macos", not(debug_assertions)))]
use foco_store::config::load_or_create_global_config;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use tokio::sync::watch;

#[cfg(any(test, all(target_os = "macos", not(debug_assertions))))]
use crate::AUTO_START_COMMAND;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::platform::tray::{
    TrayMenuLabels, browser_addr_for_listen_addr, foco_ui_url_for_listen_addr, open_foco_ui,
    tray_menu_labels,
};
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::runtime::ActiveChatRunRegistry;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::{AppResult, local_addr, logging, run_server_until_shutdown};

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_OPEN_ITEM_ID: &str = "foco-open-ui";
#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_OPEN_LOGS_ITEM_ID: &str = "foco-open-logs";
#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_OPEN_CONFIG_ITEM_ID: &str = "foco-open-config";
#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_ABOUT_ITEM_ID: &str = "foco-about";
#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_QUIT_ITEM_ID: &str = "foco-quit";

#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub(crate) async fn run_macos_menu_bar_entrypoint() -> AppResult<()> {
    let started_from_auto_start = args_started_from_auto_start(std::env::args().skip(1));
    let loaded_config = load_or_create_global_config()?;
    logging::init(&loaded_config.paths.logs_dir)?;
    crate::platform::macos_environment::apply_macos_gui_environment();
    run_macos_menu_bar_entrypoint_blocking(loaded_config, started_from_auto_start)
}

#[cfg(any(test, all(target_os = "macos", not(debug_assertions))))]
fn args_started_from_auto_start(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter().any(|arg| arg == AUTO_START_COMMAND)
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn run_macos_menu_bar_entrypoint_blocking(
    loaded_config: foco_store::config::LoadedGlobalConfig,
    started_from_auto_start: bool,
) -> AppResult<()> {
    let addr = local_addr(&loaded_config.config)?;
    let ui_url = foco_ui_url_for_listen_addr(addr);
    if open_existing_foco_instance_if_running(addr, &ui_url, started_from_auto_start) {
        return Ok(());
    }

    crate::platform::native_browser::install_macos_native_picker_dispatcher();

    let labels = tray_menu_labels(&loaded_config.config.app.language)?;
    let logs_dir = loaded_config.paths.logs_dir.clone();
    let config_dir = loaded_config.paths.root_dir.clone();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let active_chat_runs = ActiveChatRunRegistry::default();
    let runtime_active_chat_runs = active_chat_runs.clone();
    let runtime_thread = std::thread::Builder::new()
        .name("foco-http-runtime".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build Foco HTTP runtime");
            if let Err(error) = runtime.block_on(run_server_until_shutdown(
                Some(shutdown_rx),
                false,
                runtime_active_chat_runs,
            )) {
                tracing::error!(%error, "Foco server failed");
                eprintln!("Foco server failed: {error}");
                std::process::exit(1);
            }
        })?;

    run_macos_menu_bar_loop(
        ui_url,
        logs_dir,
        config_dir,
        shutdown_tx,
        active_chat_runs,
        labels,
    )?;
    runtime_thread
        .join()
        .map_err(|_| "Foco HTTP runtime thread panicked")?;

    Ok(())
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn run_macos_menu_bar_loop(
    ui_url: String,
    logs_dir: PathBuf,
    config_dir: PathBuf,
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
    labels: TrayMenuLabels,
) -> AppResult<()> {
    use tray_icon::{
        TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    let menu = Menu::new();
    let open_item = MenuItem::with_id(MENU_OPEN_ITEM_ID, labels.open, true, None);
    let open_logs_item = MenuItem::with_id(MENU_OPEN_LOGS_ITEM_ID, labels.open_logs, true, None);
    let open_config_item = MenuItem::with_id(
        MENU_OPEN_CONFIG_ITEM_ID,
        labels.open_config_folder,
        true,
        None,
    );
    let about_item = MenuItem::with_id(MENU_ABOUT_ITEM_ID, labels.about, true, None);
    let quit_item = MenuItem::with_id(MENU_QUIT_ITEM_ID, labels.quit, true, None);
    let separator = PredefinedMenuItem::separator();
    let utility_separator = PredefinedMenuItem::separator();
    menu.append_items(&[
        &open_item,
        &separator,
        &open_logs_item,
        &open_config_item,
        &about_item,
        &utility_separator,
        &quit_item,
    ])?;
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Foco")
        .with_icon(foco_macos_tray_icon()?)
        .with_icon_as_template(true)
        .build()?;

    let event_state = MacosMenuEventState {
        ui_url,
        logs_dir,
        config_dir,
        shutdown_tx,
        active_chat_runs,
    };
    MenuEvent::set_event_handler(Some(move |event| {
        handle_macos_menu_event(event, &event_state);
    }));
    TrayIconEvent::set_event_handler(Some(|_| {}));

    let application = macos_application()?;
    application.run();

    Ok(())
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn macos_application() -> Result<objc2::rc::Retained<objc2_app_kit::NSApplication>, String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let mtm = MainThreadMarker::new().ok_or("macOS menu bar must run on the main thread")?;
    let application = NSApplication::sharedApplication(mtm);
    application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    Ok(application)
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
struct MacosMenuEventState {
    ui_url: String,
    logs_dir: PathBuf,
    config_dir: PathBuf,
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn handle_macos_menu_event(event: tray_icon::menu::MenuEvent, state: &MacosMenuEventState) {
    if event.id == MENU_OPEN_ITEM_ID {
        open_foco_ui(&state.ui_url);
    } else if event.id == MENU_OPEN_LOGS_ITEM_ID {
        open_macos_folder(&state.logs_dir);
    } else if event.id == MENU_OPEN_CONFIG_ITEM_ID {
        open_macos_folder(&state.config_dir);
    } else if event.id == MENU_ABOUT_ITEM_ID {
        show_about_foco_alert();
    } else if event.id == MENU_QUIT_ITEM_ID {
        request_macos_menu_bar_shutdown(state.shutdown_tx.clone(), state.active_chat_runs.clone());
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuBarShutdownChoice {
    Force,
    Wait,
    Cancel,
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn request_macos_menu_bar_shutdown(
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
) {
    let active_run_count = match active_chat_runs.active_run_count() {
        Ok(count) => count,
        Err(error) => {
            tracing::warn!(
                ?error,
                "failed to inspect active chat runs before menu bar shutdown"
            );
            0
        }
    };
    let choice = if active_run_count == 0 {
        MenuBarShutdownChoice::Force
    } else {
        confirm_menu_bar_shutdown_with_active_runs(active_run_count)
    };

    match choice {
        MenuBarShutdownChoice::Force => finish_menu_bar_shutdown(&shutdown_tx),
        MenuBarShutdownChoice::Wait => {
            wait_for_active_runs_then_shutdown(&shutdown_tx, active_chat_runs)
        }
        MenuBarShutdownChoice::Cancel => {}
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn wait_for_active_runs_then_shutdown(
    shutdown_tx: &watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
) {
    loop {
        match active_chat_runs.active_run_count() {
            Ok(0) => break,
            Ok(_) => std::thread::sleep(Duration::from_millis(250)),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "failed to inspect active chat runs while waiting for menu bar shutdown"
                );
                break;
            }
        }
    }
    finish_menu_bar_shutdown(shutdown_tx);
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn finish_menu_bar_shutdown(shutdown_tx: &watch::Sender<bool>) {
    let _ = shutdown_tx.send(true);
    terminate_macos_application();
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn terminate_macos_application() {
    match macos_application() {
        Ok(application) => application.terminate(None),
        Err(error) => tracing::warn!(%error, "failed to terminate macOS application loop"),
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn confirm_menu_bar_shutdown_with_active_runs(active_run_count: usize) -> MenuBarShutdownChoice {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle,
    };
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else {
        return MenuBarShutdownChoice::Cancel;
    };
    if let Ok(application) = macos_application() {
        application.activate();
    }
    let message = format!("Foco still has {active_run_count} running LLM request(s).");
    let detail = "Force Quit / 强制退出: cancel running requests and quit now.\nWait / 等待: wait for running requests to finish, then quit.\nCancel / 取消: keep Foco running.";
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&NSString::from_str("Quit Foco? / 退出 Foco？"));
    alert.setInformativeText(&NSString::from_str(&format!("{message}\n\n{detail}")));
    alert.addButtonWithTitle(&NSString::from_str("Force Quit"));
    alert.addButtonWithTitle(&NSString::from_str("Wait"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));

    let response = alert.runModal();
    if response == NSAlertFirstButtonReturn {
        MenuBarShutdownChoice::Force
    } else if response == NSAlertSecondButtonReturn {
        MenuBarShutdownChoice::Wait
    } else {
        MenuBarShutdownChoice::Cancel
    }
}

#[cfg(any(test, all(target_os = "macos", not(debug_assertions))))]
fn should_open_existing_foco_ui(started_from_auto_start: bool) -> bool {
    !started_from_auto_start
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn open_existing_foco_instance_if_running(
    addr: SocketAddr,
    ui_url: &str,
    started_from_auto_start: bool,
) -> bool {
    if !existing_foco_instance_responds(addr) {
        return false;
    }

    if should_open_existing_foco_ui(started_from_auto_start) {
        open_foco_ui(ui_url);
    }
    true
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn existing_foco_instance_responds(addr: SocketAddr) -> bool {
    let addr = browser_addr_for_listen_addr(addr);
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(180)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    let request = format!("GET /api/health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = [0_u8; 512];
    let Ok(count) = stream.read(&mut response) else {
        return false;
    };
    let response = String::from_utf8_lossy(&response[..count]);
    response.starts_with("HTTP/1.1 200") && response.contains(r#""service":"foco""#)
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn open_macos_folder(path: &std::path::Path) {
    if let Err(error) = Command::new("/usr/bin/open").arg(path).spawn() {
        tracing::warn!(path = %path.display(), %error, "failed to open macOS folder");
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn show_about_foco_alert() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertStyle};
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if let Ok(application) = macos_application() {
        application.activate();
    }

    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Informational);
    alert.setMessageText(&NSString::from_str("Foco"));
    alert.setInformativeText(&NSString::from_str(concat!(
        "Foco 0.1.0\n\n",
        "A local AI coding workspace and automation runtime."
    )));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    let _ = alert.runModal();
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn foco_macos_tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    const SIZE: u32 = 18;
    let rgba = include_bytes!(concat!(env!("OUT_DIR"), "/foco-tray-18.rgba"));
    tray_icon::Icon::from_rgba(rgba.to_vec(), SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::{args_started_from_auto_start, should_open_existing_foco_ui};

    #[test]
    fn args_started_from_auto_start_detects_internal_flag() {
        assert!(args_started_from_auto_start([
            "--ignored".to_string(),
            "--auto-start".to_string(),
        ]));
    }

    #[test]
    fn existing_instance_open_policy_is_silent_for_auto_start() {
        assert!(!should_open_existing_foco_ui(true));
        assert!(should_open_existing_foco_ui(false));
    }
}
