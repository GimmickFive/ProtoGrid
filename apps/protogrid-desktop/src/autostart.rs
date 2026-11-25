pub fn is_enabled() -> bool {
    imp::is_enabled()
}

pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    imp::set_enabled(enabled)
}

/// Removes autostart entries left by the pre-rename build.
pub fn remove_legacy() {
    imp::remove_legacy()
}

fn executable() -> Result<std::path::PathBuf, String> {
    std::env::current_exe()
        .map_err(|error| format!("не удалось определить путь приложения: {error}"))
}

#[cfg(target_os = "windows")]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "ProtoGrid";
    // Name used by the build from before the rename.
    const LEGACY_VALUE: &str = "Rast";

    pub fn is_enabled() -> bool {
        let Ok(key) =
            RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_QUERY_VALUE)
        else {
            return false;
        };
        key.get_value::<String, _>(VALUE).is_ok()
    }

    pub fn set_enabled(enabled: bool) -> Result<bool, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)
            .or_else(|_| hkcu.create_subkey(RUN_KEY).map(|(key, _)| key))
            .map_err(|error| format!("реестр Windows: {error}"))?;
        if enabled {
            let exe = super::executable()?;
            key.set_value(VALUE, &format!("\"{}\"", exe.display()))
                .map_err(|error| format!("реестр Windows: {error}"))?;
        } else {
            let _ = key.delete_value(VALUE);
        }
        Ok(enabled)
    }

    pub fn remove_legacy() {
        let Ok(key) =
            RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        else {
            return;
        };
        let _ = key.delete_value(LEGACY_VALUE);
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::path::PathBuf;
    fn desktop_file() -> Result<PathBuf, String> {
        let home =
            std::env::var_os("HOME").ok_or_else(|| "не задана переменная HOME".to_string())?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("protogrid.desktop"))
    }
    fn legacy_desktop_file() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join(".config")
                .join("autostart")
                .join("rast.desktop")
        })
    }
    pub fn is_enabled() -> bool {
        desktop_file().map(|path| path.is_file()).unwrap_or(false)
    }
    pub fn set_enabled(enabled: bool) -> Result<bool, String> {
        let path = desktop_file()?;
        if enabled {
            let exe = super::executable()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("не удалось создать {}: {error}", parent.display()))?;
            }
            let escaped = exe
                .display()
                .to_string()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            let content = format!(
                "[Desktop Entry]\nType=Application\nName=ProtoGrid\nComment=Native proxy client\nExec=\"{escaped}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
            );
            std::fs::write(&path, content)
                .map_err(|error| format!("не удалось записать {}: {error}", path.display()))?;
        } else if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("не удалось удалить {}: {error}", path.display()))?;
        }
        Ok(enabled)
    }

    pub fn remove_legacy() {
        if let Some(path) = legacy_desktop_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;
    fn plist_file() -> Result<PathBuf, String> {
        let home =
            std::env::var_os("HOME").ok_or_else(|| "не задана переменная HOME".to_string())?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("app.protogrid.client.plist"))
    }
    fn legacy_plist_file() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("LaunchAgents")
                .join("app.rast.client.plist")
        })
    }
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
    pub fn is_enabled() -> bool {
        plist_file().map(|path| path.is_file()).unwrap_or(false)
    }
    pub fn set_enabled(enabled: bool) -> Result<bool, String> {
        let path = plist_file()?;
        if enabled {
            let exe = super::executable()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("не удалось создать {}: {error}", parent.display()))?;
            }
            let content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>app.protogrid.client</string>
  <key>ProgramArguments</key><array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
                escape(&exe.display().to_string())
            );
            std::fs::write(&path, content)
                .map_err(|error| format!("не удалось записать {}: {error}", path.display()))?;
        } else if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("не удалось удалить {}: {error}", path.display()))?;
        }
        Ok(enabled)
    }

    pub fn remove_legacy() {
        if let Some(path) = legacy_plist_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn set_enabled(_: bool) -> Result<bool, String> {
        Err("автозапуск не поддерживается".into())
    }
    pub fn remove_legacy() {}
}
