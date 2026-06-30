#[cfg(all(target_os = "macos", not(debug_assertions)))]
use std::{process::Command, time::Duration};

#[cfg(all(target_os = "macos", not(debug_assertions)))]
use foco_store::config::load_or_create_global_config;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use tokio::sync::watch;

#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::platform::tray::{
    TrayMenuLabels, foco_ui_url_for_listen_addr, open_foco_ui, tray_menu_labels,
};
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::runtime::ActiveChatRunRegistry;
#[cfg(all(target_os = "macos", not(debug_assertions)))]
use crate::{AppResult, local_addr, logging, run_server_until_shutdown};

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_OPEN_ITEM_ID: &str = "foco-open-ui";
#[cfg(all(target_os = "macos", not(debug_assertions)))]
const MENU_QUIT_ITEM_ID: &str = "foco-quit";

#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub(crate) async fn run_macos_menu_bar_entrypoint() -> AppResult<()> {
    run_macos_menu_bar_entrypoint_blocking()
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn run_macos_menu_bar_entrypoint_blocking() -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    logging::init(&loaded_config.paths.logs_dir)?;
    let addr = local_addr(&loaded_config.config)?;
    let ui_url = foco_ui_url_for_listen_addr(addr);
    let labels = tray_menu_labels(&loaded_config.config.app.language)?;
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
                runtime_active_chat_runs,
            )) {
                tracing::error!(%error, "Foco server failed");
                eprintln!("Foco server failed: {error}");
                std::process::exit(1);
            }
        })?;

    run_macos_menu_bar_loop(ui_url, shutdown_tx, active_chat_runs, labels)?;
    runtime_thread
        .join()
        .map_err(|_| "Foco HTTP runtime thread panicked")?;

    Ok(())
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn run_macos_menu_bar_loop(
    ui_url: String,
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
    let quit_item = MenuItem::with_id(MENU_QUIT_ITEM_ID, labels.quit, true, None);
    let separator = PredefinedMenuItem::separator();
    menu.append_items(&[&open_item, &separator, &quit_item])?;
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_title("Foco")
        .with_tooltip("Foco")
        .build()?;

    let event_state = MacosMenuEventState {
        ui_url,
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
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn handle_macos_menu_event(event: tray_icon::menu::MenuEvent, state: &MacosMenuEventState) {
    if event.id == MENU_OPEN_ITEM_ID {
        open_foco_ui(&state.ui_url);
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
    // ponytail: osascript is enough for the unbundled CLI preview; replace with
    // NSAlert when Foco grows a real .app lifecycle.
    let message = format!(
        "Foco still has {active_run_count} running LLM request(s).\n\nForce Quit / 强制退出: cancel running requests and quit now.\nWait / 等待: wait for running requests to finish, then quit.\nCancel / 取消: keep Foco running."
    );
    let script = format!(
        "display dialog {} buttons {{\"Force Quit\", \"Wait\", \"Cancel\"}} default button \"Wait\" cancel button \"Cancel\" with title {} with icon caution",
        apple_script_string(&message),
        apple_script_string("Quit Foco? / 退出 Foco？")
    );
    let output = Command::new("osascript").args(["-e", &script]).output();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "failed to show macOS shutdown confirmation");
            return MenuBarShutdownChoice::Cancel;
        }
    };
    if !output.status.success() {
        return MenuBarShutdownChoice::Cancel;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("button returned:Force Quit") {
        MenuBarShutdownChoice::Force
    } else if stdout.contains("button returned:Wait") {
        MenuBarShutdownChoice::Wait
    } else {
        MenuBarShutdownChoice::Cancel
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn apple_script_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => {}
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}
