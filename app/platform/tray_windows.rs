#[cfg(all(windows, not(debug_assertions)))]
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

#[cfg(all(windows, not(debug_assertions)))]
use foco_store::config::load_or_create_global_config;
#[cfg(all(windows, not(debug_assertions)))]
use tokio::sync::watch;

#[cfg(all(windows, not(debug_assertions)))]
use crate::platform::tray::{
    TrayMenuLabels, foco_ui_url_for_listen_addr, open_foco_ui, tray_menu_labels,
};
#[cfg(all(windows, not(debug_assertions)))]
use crate::runtime::ActiveChatRunRegistry;
#[cfg(all(windows, not(debug_assertions)))]
use crate::{AppResult, local_addr, logging, run_server_until_shutdown};

#[cfg(all(windows, not(debug_assertions)))]
// Stable tray menu item id for opening the browser UI from the Windows tray icon.
const TRAY_OPEN_ITEM_ID: &str = "foco-open-ui";
#[cfg(all(windows, not(debug_assertions)))]
// Stable tray menu item id for quitting the Windows tray application.
const TRAY_QUIT_ITEM_ID: &str = "foco-quit";

#[cfg(all(windows, not(debug_assertions)))]
#[derive(Clone)]
pub(crate) struct TrayMenuUpdateNotifier {
    sender: std::sync::mpsc::Sender<TrayMenuLabels>,
    thread_id: Arc<AtomicU32>,
}

#[cfg(all(windows, not(debug_assertions)))]
impl TrayMenuUpdateNotifier {
    pub(crate) fn notify(&self, labels: TrayMenuLabels) -> Result<(), String> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_NULL};

        let thread_id = self.thread_id.load(Ordering::SeqCst);
        if thread_id == 0 {
            return Err("tray menu message thread is not ready".to_string());
        }

        self.sender
            .send(labels)
            .map_err(|_| "tray menu update receiver is closed".to_string())?;
        let posted = unsafe { PostThreadMessageW(thread_id, WM_NULL, 0, 0) };
        if posted == 0 {
            return Err(format!(
                "failed to wake tray menu message loop: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(())
    }
}

#[cfg(all(windows, not(debug_assertions)))]
pub(crate) fn run_windows_tray_entrypoint() -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    // Initialise logging on the main thread BEFORE spawning the server so
    // that any tray-loop error is captured in the daily log file. The
    // server thread will call logging::init again; the second call is a
    // harmless no-op.
    logging::init(&loaded_config.paths.logs_dir)?;
    let addr = local_addr(&loaded_config.config)?;
    let ui_url = foco_ui_url_for_listen_addr(addr);
    let labels = tray_menu_labels(&loaded_config.config.app.language)?;
    let (tray_menu_update_tx, tray_menu_update_rx) = std::sync::mpsc::channel();
    let tray_menu_thread_id = Arc::new(AtomicU32::new(0));
    let tray_menu_update_notifier = TrayMenuUpdateNotifier {
        sender: tray_menu_update_tx,
        thread_id: tray_menu_thread_id.clone(),
    };
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
                tray_menu_update_notifier,
                runtime_active_chat_runs,
            )) {
                tracing::error!(%error, "Foco server failed");
                eprintln!("Foco server failed: {error}");
                std::process::exit(1);
            }
        })?;

    run_windows_tray_loop(
        ui_url,
        shutdown_tx,
        active_chat_runs,
        labels,
        tray_menu_update_rx,
        tray_menu_thread_id,
    )?;
    runtime_thread
        .join()
        .map_err(|_| "Foco HTTP runtime thread panicked")?;

    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn run_windows_tray_loop(
    ui_url: String,
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
    labels: TrayMenuLabels,
    tray_menu_update_rx: std::sync::mpsc::Receiver<TrayMenuLabels>,
    tray_menu_thread_id: Arc<AtomicU32>,
) -> AppResult<()> {
    use tray_icon::{
        TrayIconBuilder,
        menu::{Menu, MenuItem, PredefinedMenuItem},
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
    };

    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        tray_menu_thread_id.store(GetCurrentThreadId(), Ordering::SeqCst);
    }
    let tray_menu = Menu::new();
    let open_item = MenuItem::with_id(TRAY_OPEN_ITEM_ID, labels.open, true, None);
    let quit_item = MenuItem::with_id(TRAY_QUIT_ITEM_ID, labels.quit, true, None);
    let separator = PredefinedMenuItem::separator();
    tray_menu.append_items(&[&open_item, &separator, &quit_item])?;
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Foco")
        .with_icon(foco_tray_icon()?)
        .build()?;

    loop {
        drain_tray_events(
            &ui_url,
            &shutdown_tx,
            &active_chat_runs,
            &tray_menu_thread_id,
        );
        drain_tray_menu_updates(&tray_menu_update_rx, &open_item, &quit_item);

        let message_result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if message_result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        if message_result == 0 || message.message == WM_QUIT {
            break;
        }

        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn drain_tray_menu_updates(
    tray_menu_update_rx: &std::sync::mpsc::Receiver<TrayMenuLabels>,
    open_item: &tray_icon::menu::MenuItem,
    quit_item: &tray_icon::menu::MenuItem,
) {
    while let Ok(labels) = tray_menu_update_rx.try_recv() {
        open_item.set_text(labels.open);
        quit_item.set_text(labels.quit);
    }
}

#[cfg(all(windows, not(debug_assertions)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayShutdownChoice {
    Force,
    Wait,
    Cancel,
}

#[cfg(all(windows, not(debug_assertions)))]
fn drain_tray_events(
    ui_url: &str,
    shutdown_tx: &watch::Sender<bool>,
    active_chat_runs: &ActiveChatRunRegistry,
    tray_menu_thread_id: &Arc<AtomicU32>,
) {
    use tray_icon::{TrayIconEvent, menu::MenuEvent};

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            open_foco_ui(ui_url);
        }
    }

    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == TRAY_OPEN_ITEM_ID {
            open_foco_ui(ui_url);
        } else if event.id == TRAY_QUIT_ITEM_ID {
            request_tray_shutdown(shutdown_tx, active_chat_runs, tray_menu_thread_id);
        }
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn request_tray_shutdown(
    shutdown_tx: &watch::Sender<bool>,
    active_chat_runs: &ActiveChatRunRegistry,
    tray_menu_thread_id: &Arc<AtomicU32>,
) {
    let active_run_count = match active_chat_runs.active_run_count() {
        Ok(count) => count,
        Err(error) => {
            tracing::warn!(
                ?error,
                "failed to inspect active chat runs before tray shutdown"
            );
            0
        }
    };
    let choice = if active_run_count == 0 {
        TrayShutdownChoice::Force
    } else {
        confirm_tray_shutdown_with_active_runs(active_run_count)
    };

    match choice {
        TrayShutdownChoice::Force => finish_tray_shutdown(shutdown_tx, tray_menu_thread_id),
        TrayShutdownChoice::Wait => wait_for_active_runs_then_shutdown(
            shutdown_tx.clone(),
            active_chat_runs.clone(),
            tray_menu_thread_id.clone(),
        ),
        TrayShutdownChoice::Cancel => {}
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn wait_for_active_runs_then_shutdown(
    shutdown_tx: watch::Sender<bool>,
    active_chat_runs: ActiveChatRunRegistry,
    tray_menu_thread_id: Arc<AtomicU32>,
) {
    let _ = std::thread::Builder::new()
        .name("foco-tray-shutdown-wait".to_string())
        .spawn(move || {
            loop {
                match active_chat_runs.active_run_count() {
                    Ok(0) => break,
                    Ok(_) => std::thread::sleep(Duration::from_millis(250)),
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "failed to inspect active chat runs while waiting for tray shutdown"
                        );
                        break;
                    }
                }
            }
            finish_tray_shutdown(&shutdown_tx, &tray_menu_thread_id);
        });
}

#[cfg(all(windows, not(debug_assertions)))]
fn finish_tray_shutdown(shutdown_tx: &watch::Sender<bool>, tray_menu_thread_id: &Arc<AtomicU32>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PostQuitMessage, PostThreadMessageW, WM_QUIT,
    };

    let _ = shutdown_tx.send(true);
    let thread_id = tray_menu_thread_id.load(Ordering::SeqCst);
    if thread_id == 0 {
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }

    let posted = unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
    if posted == 0 {
        tracing::warn!(error = %std::io::Error::last_os_error(), "failed to wake tray loop for shutdown");
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn confirm_tray_shutdown_with_active_runs(active_run_count: usize) -> TrayShutdownChoice {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDCANCEL, IDNO, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNOCANCEL, MessageBoxW,
    };

    let message = format!(
        "Foco still has {active_run_count} running LLM request(s).\n\nYes / 是: force quit now and cancel running requests.\nNo / 否: wait for running requests to finish, then quit.\nCancel / 取消: keep Foco running."
    );
    let title = "Quit Foco? / 退出 Foco？";
    let message = wide_null(&message);
    let title = wide_null(title);
    let response = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNOCANCEL | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };

    match response {
        IDYES => TrayShutdownChoice::Force,
        IDNO => TrayShutdownChoice::Wait,
        IDCANCEL => TrayShutdownChoice::Cancel,
        _ => TrayShutdownChoice::Cancel,
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(all(windows, not(debug_assertions)))]
fn foco_tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    tray_icon::Icon::from_resource(1, Some((32, 32)))
}
