use std::env;

use crate::{ApiError, normalize_windows_verbatim_path};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, WIN32_ERROR},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
    },
};

#[cfg(windows)]
const AUTO_START_REGISTRY_VALUE_NAME: &str = "Foco";
#[cfg(windows)]
const AUTO_START_REGISTRY_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
pub(crate) fn apply_auto_start_setting(enabled: bool) -> Result<(), ApiError> {
    if enabled {
        let exe_path = env::current_exe().map_err(|source| {
            ApiError::internal(format!(
                "failed to resolve current executable path for auto start: {source}"
            ))
        })?;
        let exe_path = normalize_windows_verbatim_path(exe_path);
        set_auto_start_registry_value(&format!("\"{}\"", exe_path.display()))
    } else {
        remove_auto_start_registry_value()
    }
}

#[cfg(not(windows))]
pub(crate) fn apply_auto_start_setting(enabled: bool) -> Result<(), ApiError> {
    if enabled {
        return Err(ApiError::bad_request(
            "auto start is only supported on Windows",
        ));
    }

    Ok(())
}

#[cfg(windows)]
fn set_auto_start_registry_value(command: &str) -> Result<(), ApiError> {
    let value_name = wide_null(AUTO_START_REGISTRY_VALUE_NAME);
    let data_wide = wide_null(command);
    let data_bytes = wide_bytes(&data_wide);
    let data_len = u32::try_from(data_bytes.len())
        .map_err(|_| ApiError::internal("auto start registry command is too large"))?;
    let key = open_auto_start_registry_key()?;

    let set_result = unsafe {
        RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_SZ,
            data_bytes.as_ptr(),
            data_len,
        )
    };
    let close_result = unsafe { RegCloseKey(key) };

    if set_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("set", set_result));
    }
    if close_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("close", close_result));
    }

    Ok(())
}

#[cfg(windows)]
fn remove_auto_start_registry_value() -> Result<(), ApiError> {
    let key = open_existing_auto_start_registry_key()?;
    let value_name = wide_null(AUTO_START_REGISTRY_VALUE_NAME);

    let delete_result = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
    let close_result = unsafe { RegCloseKey(key) };

    if delete_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("delete", delete_result));
    }
    if close_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("close", close_result));
    }

    Ok(())
}

#[cfg(windows)]
fn open_auto_start_registry_key() -> Result<HKEY, ApiError> {
    let key_path = wide_null(AUTO_START_REGISTRY_RUN_KEY);
    let mut key = std::ptr::null_mut();

    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };

    if result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("open", result));
    }

    Ok(key)
}

#[cfg(windows)]
fn open_existing_auto_start_registry_key() -> Result<HKEY, ApiError> {
    let key_path = wide_null(AUTO_START_REGISTRY_RUN_KEY);
    let mut key = std::ptr::null_mut();

    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };

    if result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("open", result));
    }

    Ok(key)
}

#[cfg(windows)]
fn auto_start_registry_error(action: &str, code: WIN32_ERROR) -> ApiError {
    ApiError::internal(format!(
        "failed to {action} Windows auto start registry entry: Win32 error {code}"
    ))
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn wide_bytes(value: &[u16]) -> Vec<u8> {
    value.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}
