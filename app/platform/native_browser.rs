use std::{
    collections::HashMap,
    ffi::c_void,
    fs,
    net::SocketAddr,
    path::PathBuf,
    ptr,
    sync::{Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

#[cfg(all(not(windows), not(target_os = "macos")))]
use std::process::{Command, Stdio};

use axum::{
    Json,
    body::Body,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose};

use crate::{
    ApiError, AppState, MAX_CHAT_ATTACHMENT_BYTES, MAX_CHAT_ATTACHMENT_TOTAL_BYTES,
    MAX_CHAT_ATTACHMENTS, NativeBrowserProbeQuery, NativePickerRequest, NativeSelectedFile,
    SelectDirectoryResponse, SelectFilesResponse, attachment_content_type_for_path,
};

#[cfg(all(not(windows), not(target_os = "macos")))]
use crate::prompt::is_wsl_environment;

const NATIVE_BROWSER_AUTHORIZATION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const NATIVE_BROWSER_PROBE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#0f766e"/></svg>"##;

#[cfg(target_os = "macos")]
static MACOS_NATIVE_PICKER_TX: OnceLock<mpsc::Sender<MacosNativePickerRequest>> = OnceLock::new();
#[cfg(target_os = "macos")]
static MACOS_NATIVE_PICKER_RX: OnceLock<Mutex<mpsc::Receiver<MacosNativePickerRequest>>> =
    OnceLock::new();

pub(crate) async fn native_browser_probe(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<NativeBrowserProbeQuery>,
) -> Result<Response, ApiError> {
    let token = validate_native_browser_token(&query.token)?;
    if !remote_addr.ip().is_loopback() || !native_probe_host_is_loopback(&headers) {
        return Err(ApiError::forbidden(
            "native picker probe must come from this computer",
        ));
    }

    state.native_browser_authorizations.authorize(token)?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(NATIVE_BROWSER_PROBE_SVG))
        .expect("native browser probe response is valid"))
}

pub(crate) async fn select_directory(
    State(state): State<AppState>,
    Json(request): Json<NativePickerRequest>,
) -> Result<Json<SelectDirectoryResponse>, ApiError> {
    require_native_browser_authorization(&state, &request.native_browser_token)?;

    let path = native_select_directory()?;

    Ok(Json(SelectDirectoryResponse { path }))
}

pub(crate) async fn select_files(
    State(state): State<AppState>,
    Json(request): Json<NativePickerRequest>,
) -> Result<Json<SelectFilesResponse>, ApiError> {
    require_native_browser_authorization(&state, &request.native_browser_token)?;

    let files = native_select_files()?;

    Ok(Json(SelectFilesResponse { files }))
}

fn require_native_browser_authorization(state: &AppState, token: &str) -> Result<(), ApiError> {
    let token = validate_native_browser_token(token)?;
    if state.native_browser_authorizations.is_authorized(token)? {
        return Ok(());
    }

    Err(ApiError::forbidden(
        "native picker is only available from a browser running on the Foco computer",
    ))
}

fn validate_native_browser_token(token: &str) -> Result<&str, ApiError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(ApiError::bad_request(
            "native browser token must not be empty",
        ));
    }
    if token.len() > 128 {
        return Err(ApiError::bad_request("native browser token is too long"));
    }
    if !token
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ApiError::bad_request(
            "native browser token contains unsupported characters",
        ));
    }

    Ok(token)
}

fn native_probe_host_is_loopback(headers: &HeaderMap) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    let host = host.trim().to_ascii_lowercase();
    let host_name = if let Some(without_opening_bracket) = host.strip_prefix('[') {
        without_opening_bracket
            .split_once(']')
            .map(|(name, _)| name)
            .unwrap_or(without_opening_bracket)
    } else {
        host.split_once(':')
            .map(|(name, _)| name)
            .unwrap_or(host.as_str())
    };

    matches!(host_name, "localhost" | "127.0.0.1" | "::1")
}

pub(crate) fn prune_native_browser_authorizations(tokens: &mut HashMap<String, Instant>) {
    let now = Instant::now();
    tokens.retain(|_, authorized_at| {
        now.duration_since(*authorized_at) <= NATIVE_BROWSER_AUTHORIZATION_TTL
    });
}

fn native_select_directory() -> Result<Option<String>, ApiError> {
    #[cfg(windows)]
    {
        return native_select_directory_windows();
    }

    #[cfg(target_os = "macos")]
    {
        native_select_directory_macos()
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if !is_wsl_environment() {
            return Err(ApiError::bad_request(
                "native directory picker is only available on Windows and macOS",
            ));
        }

        let Some(selected_path) = native_select_directory_with_powershell()? else {
            return Ok(None);
        };

        Ok(Some(windows_path_to_wsl_path(selected_path)?))
    }
}

#[cfg(windows)]
struct NativePickerComApartment;

#[cfg(windows)]
impl Drop for NativePickerComApartment {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}

#[cfg(windows)]
fn native_picker_com_apartment() -> Result<NativePickerComApartment, ApiError> {
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    };

    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        if initialized.is_err() {
            return Err(ApiError::internal(format!(
                "failed to initialize native picker COM: {}",
                initialized.message()
            )));
        }

        Ok(NativePickerComApartment)
    }
}

#[cfg(windows)]
fn native_picker_was_cancelled(error: &windows::core::Error) -> bool {
    use windows::{Win32::Foundation::ERROR_CANCELLED, core::HRESULT};

    error.code() == HRESULT::from_win32(ERROR_CANCELLED.0)
}

#[cfg(windows)]
fn native_shell_item_path(
    item: &windows::Win32::UI::Shell::IShellItem,
) -> Result<String, ApiError> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{IShellItem, SIGDN_FILESYSPATH},
    };

    let item: &IShellItem = item;
    unsafe {
        let path_ptr = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|source| {
            ApiError::internal(format!("failed to read native picker path: {source}"))
        })?;
        if path_ptr.0.is_null() {
            return Err(ApiError::internal(
                "native picker returned an empty path pointer",
            ));
        }

        let mut length = 0usize;
        while *path_ptr.0.add(length) != 0 {
            length += 1;
        }
        let path = String::from_utf16_lossy(std::slice::from_raw_parts(path_ptr.0, length));
        CoTaskMemFree(Some(path_ptr.0.cast()));

        Ok(path)
    }
}

#[cfg(windows)]
fn native_select_directory_windows() -> Result<Option<String>, ApiError> {
    use windows::{
        Win32::{
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            UI::Shell::{
                FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
                IFileOpenDialog,
            },
        },
        core::{HSTRING, IUnknown},
    };

    let _com_apartment = native_picker_com_apartment()?;

    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None::<&IUnknown>, CLSCTX_INPROC_SERVER).map_err(
                |source| {
                    ApiError::internal(format!(
                        "failed to create native directory picker: {source}"
                    ))
                },
            )?;
        let options = dialog.GetOptions().map_err(|source| {
            ApiError::internal(format!(
                "failed to read native directory picker options: {source}"
            ))
        })?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .map_err(|source| {
                ApiError::internal(format!(
                    "failed to configure native directory picker: {source}"
                ))
            })?;
        dialog
            .SetTitle(&HSTRING::from("Choose workspace path"))
            .map_err(|source| {
                ApiError::internal(format!(
                    "failed to set native directory picker title: {source}"
                ))
            })?;
        dialog
            .SetOkButtonLabel(&HSTRING::from("Select"))
            .map_err(|source| {
                ApiError::internal(format!(
                    "failed to set native directory picker button label: {source}"
                ))
            })?;

        if let Err(source) = dialog.Show(None) {
            if native_picker_was_cancelled(&source) {
                return Ok(None);
            }

            return Err(ApiError::internal(format!(
                "native directory picker failed: {source}"
            )));
        }

        let item = dialog.GetResult().map_err(|source| {
            ApiError::internal(format!(
                "failed to read native directory picker result: {source}"
            ))
        })?;

        Ok(Some(native_shell_item_path(&item)?))
    }
}

#[cfg(target_os = "macos")]
fn native_select_directory_macos() -> Result<Option<String>, ApiError> {
    match run_macos_native_picker(MacosNativePickerKind::Directory)? {
        MacosNativePickerSelection::Directory(path) => Ok(path),
        MacosNativePickerSelection::Files(_) => Err(ApiError::internal(
            "native directory picker returned file picker results",
        )),
    }
}

#[cfg(target_os = "macos")]
fn native_select_files_macos() -> Result<Vec<String>, ApiError> {
    match run_macos_native_picker(MacosNativePickerKind::Files)? {
        MacosNativePickerSelection::Files(paths) => Ok(paths),
        MacosNativePickerSelection::Directory(_) => Err(ApiError::internal(
            "native file picker returned directory picker results",
        )),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn install_macos_native_picker_dispatcher() {
    if MACOS_NATIVE_PICKER_TX.get().is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel();
    let _ = MACOS_NATIVE_PICKER_TX.set(tx);
    let _ = MACOS_NATIVE_PICKER_RX.set(Mutex::new(rx));
}

#[cfg(target_os = "macos")]
fn run_macos_native_picker(
    kind: MacosNativePickerKind,
) -> Result<MacosNativePickerSelection, ApiError> {
    let sender = MACOS_NATIVE_PICKER_TX
        .get()
        .ok_or_else(|| ApiError::internal("native macOS picker dispatcher is not available"))?;
    let (response_tx, response_rx) = mpsc::channel();
    sender
        .send(MacosNativePickerRequest { kind, response_tx })
        .map_err(|_| ApiError::internal("native macOS picker dispatcher is closed"))?;
    schedule_macos_native_picker_dispatch();

    response_rx
        .recv()
        .map_err(|_| ApiError::internal("native macOS picker response channel is closed"))?
        .map_err(ApiError::internal)
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum MacosNativePickerKind {
    Directory,
    Files,
}

#[cfg(target_os = "macos")]
struct MacosNativePickerRequest {
    kind: MacosNativePickerKind,
    response_tx: mpsc::Sender<Result<MacosNativePickerSelection, String>>,
}

#[cfg(target_os = "macos")]
enum MacosNativePickerSelection {
    Directory(Option<String>),
    Files(Vec<String>),
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn dispatch_get_main_queue() -> *mut c_void;
    fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

#[cfg(target_os = "macos")]
fn schedule_macos_native_picker_dispatch() {
    unsafe {
        dispatch_async_f(
            dispatch_get_main_queue(),
            ptr::null_mut(),
            drain_macos_native_picker_requests,
        );
    }
}

#[cfg(target_os = "macos")]
extern "C" fn drain_macos_native_picker_requests(_: *mut c_void) {
    let Some(receiver) = MACOS_NATIVE_PICKER_RX.get() else {
        return;
    };

    loop {
        let request = match receiver.lock() {
            Ok(receiver) => match receiver.try_recv() {
                Ok(request) => request,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            },
            Err(_) => break,
        };
        let result = run_macos_open_panel(request.kind);
        let _ = request.response_tx.send(result);
    }
}

#[cfg(target_os = "macos")]
fn run_macos_open_panel(kind: MacosNativePickerKind) -> Result<MacosNativePickerSelection, String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSString;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "native macOS picker must run on the main thread".to_string())?;
    let panel = NSOpenPanel::openPanel(mtm);
    match kind {
        MacosNativePickerKind::Directory => {
            panel.setTitle(Some(&NSString::from_str("Choose workspace path")));
            panel.setPrompt(Some(&NSString::from_str("Select")));
            panel.setCanChooseFiles(false);
            panel.setCanChooseDirectories(true);
            panel.setAllowsMultipleSelection(false);
        }
        MacosNativePickerKind::Files => {
            panel.setTitle(Some(&NSString::from_str("Choose attachments")));
            panel.setPrompt(Some(&NSString::from_str("Select")));
            panel.setCanChooseFiles(true);
            panel.setCanChooseDirectories(false);
            panel.setAllowsMultipleSelection(true);
        }
    }

    if panel.runModal() != NSModalResponseOK {
        return Ok(match kind {
            MacosNativePickerKind::Directory => MacosNativePickerSelection::Directory(None),
            MacosNativePickerKind::Files => MacosNativePickerSelection::Files(Vec::new()),
        });
    }

    let paths = macos_open_panel_paths(&panel)?;
    match kind {
        MacosNativePickerKind::Directory => {
            let path = paths
                .into_iter()
                .next()
                .ok_or_else(|| "native directory picker returned no path".to_string())?;
            if path.trim().is_empty() {
                return Err("native directory picker returned an empty path".to_string());
            }
            Ok(MacosNativePickerSelection::Directory(Some(path)))
        }
        MacosNativePickerKind::Files => {
            if paths.iter().any(|path| path.trim().is_empty()) {
                return Err("native file picker returned an empty path".to_string());
            }
            Ok(MacosNativePickerSelection::Files(paths))
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_open_panel_paths(panel: &objc2_app_kit::NSOpenPanel) -> Result<Vec<String>, String> {
    let urls = panel.URLs();
    let mut paths = Vec::with_capacity(urls.count());
    for index in 0..urls.count() {
        let url = urls.objectAtIndex(index);
        let path = url
            .path()
            .ok_or_else(|| "native picker returned a URL without a filesystem path".to_string())?;
        paths.push(path.to_string());
    }

    Ok(paths)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn native_select_directory_with_powershell() -> Result<Option<String>, ApiError> {
    if !is_wsl_environment() {
        return Err(ApiError::bad_request(
            "native directory picker is only available on Windows",
        ));
    }

    let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[ComImport]
[Guid("DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7")]
public class FileOpenDialogCom
{
}

[ComImport]
[Guid("D57C7288-D4AD-4768-BE02-9D969532D960")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IFileOpenDialog
{
    [PreserveSig]
    int Show(IntPtr parent);
    void SetFileTypes(uint cFileTypes, IntPtr rgFilterSpec);
    void SetFileTypeIndex(uint iFileType);
    void GetFileTypeIndex(out uint piFileType);
    void Advise(IntPtr pfde, out uint pdwCookie);
    void Unadvise(uint dwCookie);
    void SetOptions(uint fos);
    void GetOptions(out uint pfos);
    void SetDefaultFolder(IShellItem psi);
    void SetFolder(IShellItem psi);
    void GetFolder(out IShellItem ppsi);
    void GetCurrentSelection(out IShellItem ppsi);
    void SetFileName([MarshalAs(UnmanagedType.LPWStr)] string pszName);
    void GetFileName([MarshalAs(UnmanagedType.LPWStr)] out string pszName);
    void SetTitle([MarshalAs(UnmanagedType.LPWStr)] string pszTitle);
    void SetOkButtonLabel([MarshalAs(UnmanagedType.LPWStr)] string pszText);
    void SetFileNameLabel([MarshalAs(UnmanagedType.LPWStr)] string pszLabel);
    void GetResult(out IShellItem ppsi);
    void AddPlace(IShellItem psi, int fdap);
    void SetDefaultExtension([MarshalAs(UnmanagedType.LPWStr)] string pszDefaultExtension);
    void Close(int hr);
    void SetClientGuid(ref Guid guid);
    void ClearClientData();
    void SetFilter(IntPtr pFilter);
    void GetResults(out IntPtr ppenum);
    void GetSelectedItems(out IntPtr ppsai);
}

[ComImport]
[Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IShellItem
{
    void BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppv);
    void GetParent(out IShellItem ppsi);
    void GetDisplayName(uint sigdnName, [MarshalAs(UnmanagedType.LPWStr)] out string ppszName);
    void GetAttributes(uint sfgaoMask, out uint psfgaoAttribs);
    void Compare(IShellItem psi, uint hint, out int piOrder);
}

public static class ModernFolderPicker
{
    private const uint FOS_PICKFOLDERS = 0x00000020;
    private const uint FOS_FORCEFILESYSTEM = 0x00000040;
    private const uint FOS_PATHMUSTEXIST = 0x00000800;
    private const uint SIGDN_FILESYSPATH = 0x80058000;
    private const int HRESULT_CANCELLED = unchecked((int)0x800704C7);

    public static string Pick()
    {
        IFileOpenDialog dialog = (IFileOpenDialog)new FileOpenDialogCom();
        uint options;
        dialog.GetOptions(out options);
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
        dialog.SetTitle("Choose workspace path");
        dialog.SetOkButtonLabel("Select");

        int result = dialog.Show(IntPtr.Zero);
        if (result == HRESULT_CANCELLED)
        {
            return null;
        }

        if (result != 0)
        {
            Marshal.ThrowExceptionForHR(result);
        }

        IShellItem item;
        dialog.GetResult(out item);

        string path;
        item.GetDisplayName(SIGDN_FILESYSPATH, out path);
        return path;
    }
}
'@

$selectedPath = [ModernFolderPicker]::Pick()
if ($selectedPath) {
  Write-Output $selectedPath
}
"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoLogo", "-NoProfile", "-STA", "-Command", script])
        .stdin(Stdio::null());

    let output = command.output().map_err(|source| {
        ApiError::internal(format!(
            "failed to launch native directory picker: {source}"
        ))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "native directory picker failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected_path))
}

fn native_select_files() -> Result<Vec<NativeSelectedFile>, ApiError> {
    #[cfg(windows)]
    {
        return native_select_files_windows();
    }

    #[cfg(target_os = "macos")]
    {
        native_selected_files_from_paths(native_select_files_macos()?)
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if !is_wsl_environment() {
            return Err(ApiError::bad_request(
                "native file picker is only available on Windows and macOS",
            ));
        }

        let selected_paths = native_select_files_with_powershell()?
            .into_iter()
            .map(windows_path_to_wsl_path)
            .collect::<Result<Vec<_>, _>>()?;

        native_selected_files_from_paths(selected_paths)
    }
}

#[cfg(windows)]
fn native_select_files_windows() -> Result<Vec<NativeSelectedFile>, ApiError> {
    use windows::{
        Win32::{
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            UI::Shell::{
                FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
                FileOpenDialog, IFileOpenDialog,
            },
        },
        core::{HSTRING, IUnknown},
    };

    let _com_apartment = native_picker_com_apartment()?;

    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None::<&IUnknown>, CLSCTX_INPROC_SERVER).map_err(
                |source| {
                    ApiError::internal(format!("failed to create native file picker: {source}"))
                },
            )?;
        let options = dialog.GetOptions().map_err(|source| {
            ApiError::internal(format!(
                "failed to read native file picker options: {source}"
            ))
        })?;
        dialog
            .SetOptions(
                options
                    | FOS_ALLOWMULTISELECT
                    | FOS_FILEMUSTEXIST
                    | FOS_FORCEFILESYSTEM
                    | FOS_PATHMUSTEXIST,
            )
            .map_err(|source| {
                ApiError::internal(format!("failed to configure native file picker: {source}"))
            })?;
        dialog
            .SetTitle(&HSTRING::from("Choose attachments"))
            .map_err(|source| {
                ApiError::internal(format!("failed to set native file picker title: {source}"))
            })?;

        if let Err(source) = dialog.Show(None) {
            if native_picker_was_cancelled(&source) {
                return Ok(Vec::new());
            }

            return Err(ApiError::internal(format!(
                "native file picker failed: {source}"
            )));
        }

        let items = dialog.GetResults().map_err(|source| {
            ApiError::internal(format!(
                "failed to read native file picker results: {source}"
            ))
        })?;
        let count = items.GetCount().map_err(|source| {
            ApiError::internal(format!(
                "failed to count native file picker results: {source}"
            ))
        })?;
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let item = items.GetItemAt(index).map_err(|source| {
                ApiError::internal(format!(
                    "failed to read native file picker result {index}: {source}"
                ))
            })?;
            paths.push(native_shell_item_path(&item)?);
        }

        native_selected_files_from_paths(paths)
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn native_select_files_with_powershell() -> Result<Vec<String>, ApiError> {
    if !is_wsl_environment() {
        return Err(ApiError::bad_request(
            "native file picker is only available on Windows",
        ));
    }

    let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms

$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
$dialog.Multiselect = $true
$dialog.Title = "Choose attachments"

if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  ConvertTo-Json -InputObject @($dialog.FileNames) -Compress
} else {
  Write-Output "[]"
}
"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoLogo", "-NoProfile", "-STA", "-Command", script])
        .stdin(Stdio::null());

    let output = command.output().map_err(|source| {
        ApiError::internal(format!("failed to launch native file picker: {source}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "native file picker failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str::<Vec<String>>(&stdout).map_err(|source| {
        ApiError::internal(format!(
            "native file picker returned invalid JSON: {source}"
        ))
    })
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn windows_path_to_wsl_path(path: String) -> Result<String, ApiError> {
    let output = Command::new("wslpath")
        .args(["-u", &path])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| {
            ApiError::internal(format!("failed to convert selected Windows path: {source}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "failed to convert selected Windows path{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn native_selected_files_from_paths(
    paths: Vec<String>,
) -> Result<Vec<NativeSelectedFile>, ApiError> {
    if paths.len() > MAX_CHAT_ATTACHMENTS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_CHAT_ATTACHMENTS} attachments are allowed"
        )));
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut total_size = 0_u64;
    for path in paths {
        let path = path.trim().to_string();
        if path.is_empty() {
            return Err(ApiError::bad_request(
                "selected file path must not be empty",
            ));
        }

        let path_buf = PathBuf::from(&path);
        let metadata = fs::metadata(&path_buf).map_err(|source| {
            ApiError::bad_request(format!("selected file is not readable: {path}: {source}"))
        })?;
        if !metadata.is_file() {
            return Err(ApiError::bad_request(format!(
                "selected attachment path must be a file: {path}"
            )));
        }

        let name = path_buf
            .file_name()
            .map(|value| value.to_string_lossy().trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request(format!("selected file has no name: {path}")))?;
        let size_bytes = metadata.len();
        if size_bytes > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachment {name} exceeds the {} byte limit",
                MAX_CHAT_ATTACHMENT_BYTES
            )));
        }

        total_size = total_size
            .checked_add(size_bytes)
            .ok_or_else(|| ApiError::bad_request("attachment total size exceeds u64"))?;
        if total_size > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachments exceed the {} byte total limit",
                MAX_CHAT_ATTACHMENT_TOTAL_BYTES
            )));
        }

        let content_type = attachment_content_type_for_path(&path_buf);
        let content_base64 = if content_type.starts_with("image/") {
            let bytes = fs::read(&path_buf).map_err(|source| {
                ApiError::bad_request(format!("failed to read selected image {name}: {source}"))
            })?;
            Some(general_purpose::STANDARD.encode(bytes))
        } else {
            None
        };

        files.push(NativeSelectedFile {
            path,
            name,
            content_type,
            size_bytes,
            content_base64,
        });
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_file_paths_keep_existing_empty_path_validation() {
        assert!(native_selected_files_from_paths(vec!["".to_string()]).is_err());
    }
}
