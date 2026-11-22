use crate::error::{MarshalError, Result};

/// Sends an F10 keypress to reload 3DMigoto in the active game window.
/// Supports X11/XWayland (via xdotool), pure Wayland (via wtype), and Enigo fallback.
pub fn send_reload_f10() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // 1. Try xdotool (works reliably for X11 and XWayland game windows)
        if let Ok(status) = std::process::Command::new("xdotool")
            .args(["key", "--delay", "40", "F10"])
            .status()
        {
            if status.success() {
                println!("Input: Sent F10 via xdotool");
                return Ok(());
            }
        }

        // 2. Try wtype for pure Wayland native windows
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|s| s.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false);

        if is_wayland {
            if let Ok(status) = std::process::Command::new("wtype")
                .args(["-k", "F10"])
                .status()
            {
                if status.success() {
                    println!("Input: Sent F10 via wtype");
                    return Ok(());
                }
            }
        }

        // 3. Fallback to Enigo
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| MarshalError::Io(std::io::Error::other(e.to_string())))?;
        enigo
            .key(Key::F10, Direction::Click)
            .map_err(|e| MarshalError::Io(std::io::Error::other(e.to_string())))?;
        println!("Input: Sent F10 via Enigo fallback");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            MarshalError::Io(std::io::Error::other(format!(
                "Failed to initialize Enigo input simulator: {}",
                e
            )))
        })?;
        enigo.key(Key::F10, Direction::Click).map_err(|e| {
            MarshalError::Io(std::io::Error::other(format!(
                "Failed to send F10 keystroke (game elevation/UAC may block synthetic input): {}",
                e
            )))
        })?;
        println!("Input: Sent F10 via Enigo");
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| MarshalError::Io(std::io::Error::other(e.to_string())))?;
        enigo
            .key(Key::F10, Direction::Click)
            .map_err(|e| MarshalError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }
}
