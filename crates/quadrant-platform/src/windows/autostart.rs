//! Windows login startup registration.

use std::{env, path::Path};

use quadrant_application::AutostartError;
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
            RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
        },
    },
    core::PCWSTR,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "QuadrantRust";

pub(super) fn set_enabled(enabled: bool, start_hidden: bool) -> Result<(), AutostartError> {
    let executable = env::current_exe().map_err(AutostartError::new)?;
    let run_key = wide_null(RUN_KEY);
    let value_name = wide_null(VALUE_NAME);
    let mut key = HKEY::default();
    // SAFETY: all PCWSTR inputs are backed by live nul-terminated UTF-16 buffers,
    // output storage is valid, and the returned key is closed below.
    let opened = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(run_key.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &raw mut key,
            None,
        )
    };
    if opened != ERROR_SUCCESS {
        return Err(AutostartError::new(format!(
            "opening the Windows Run key failed with code {}",
            opened.0
        )));
    }

    let operation = if enabled {
        let command = startup_command(&executable, start_hidden);
        let encoded = wide_null(&command);
        let bytes = encoded
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();
        // SAFETY: key is open with KEY_SET_VALUE and both the value name and
        // REG_SZ payload buffers remain alive for the duration of the call.
        unsafe { RegSetValueExW(key, PCWSTR(value_name.as_ptr()), None, REG_SZ, Some(&bytes)) }
    } else {
        // SAFETY: key is open with KEY_SET_VALUE and value_name is nul-terminated.
        unsafe { RegDeleteValueW(key, PCWSTR(value_name.as_ptr())) }
    };
    // SAFETY: key was initialized successfully by RegCreateKeyExW and is no longer used.
    let closed = unsafe { RegCloseKey(key) };
    let operation_succeeded =
        operation == ERROR_SUCCESS || (!enabled && operation == ERROR_FILE_NOT_FOUND);
    if !operation_succeeded {
        return Err(AutostartError::new(format!(
            "updating the Windows startup value failed with code {}",
            operation.0
        )));
    }
    if closed != ERROR_SUCCESS {
        return Err(AutostartError::new(format!(
            "closing the Windows Run key failed with code {}",
            closed.0
        )));
    }
    Ok(())
}

fn startup_command(executable: &Path, _start_hidden: bool) -> String {
    format!("\"{}\" --background", executable.display())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::startup_command;

    #[test]
    fn startup_command_always_targets_agent_login_policy() {
        let executable = Path::new("C:/Program Files/Quadrant/quadrant-agent.exe");
        assert_eq!(
            startup_command(executable, false),
            "\"C:/Program Files/Quadrant/quadrant-agent.exe\" --background"
        );
        assert_eq!(
            startup_command(executable, true),
            "\"C:/Program Files/Quadrant/quadrant-agent.exe\" --background"
        );
    }
}
