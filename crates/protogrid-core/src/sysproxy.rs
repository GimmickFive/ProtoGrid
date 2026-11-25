//! Minimal cross-platform system proxy integration.
//!
//! Windows uses WinINet registry settings, Linux talks to GNOME/KDE settings,
//! macOS uses `networksetup`.

pub fn is_supported() -> bool {
    imp::supported()
}

pub fn enable(http_port: u16, socks_port: u16) -> Result<(), String> {
    imp::enable(http_port, socks_port)
}

pub fn disable() -> Result<(), String> {
    imp::disable()
}

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;

    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    fn refresh() {
        unsafe {
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_SETTINGS_CHANGED,
                std::ptr::null::<c_void>(),
                0,
            );
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                std::ptr::null::<c_void>(),
                0,
            );
        }
    }

    pub fn supported() -> bool {
        true
    }

    pub fn enable(http_port: u16, socks_port: u16) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(PATH, KEY_SET_VALUE | KEY_QUERY_VALUE)
            .map_err(|e| format!("не удалось открыть настройки прокси Windows: {e}"))?;
        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| format!("ProxyEnable: {e}"))?;
        key.set_value(
            "ProxyServer",
            &format!(
                "http=127.0.0.1:{http_port};https=127.0.0.1:{http_port};socks=127.0.0.1:{socks_port}"
            ),
        )
        .map_err(|e| format!("ProxyServer: {e}"))?;
        key.set_value("ProxyOverride", &"<local>")
            .map_err(|e| format!("ProxyOverride: {e}"))?;
        refresh();
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(PATH, KEY_SET_VALUE)
            .map_err(|e| format!("не удалось открыть настройки прокси Windows: {e}"))?;
        key.set_value("ProxyEnable", &0u32)
            .map_err(|e| format!("ProxyEnable: {e}"))?;
        refresh();
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::process::Command;

    fn run(program: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if error.is_empty() {
                format!("{program} завершился с ошибкой")
            } else {
                error
            })
        }
    }

    fn gsettings_available() -> bool {
        Command::new("gsettings")
            .args(["get", "org.gnome.system.proxy", "mode"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn kde_available() -> bool {
        ["kwriteconfig6", "kwriteconfig5"]
            .iter()
            .any(|program| Command::new(program).arg("--help").output().is_ok())
    }

    pub fn supported() -> bool {
        gsettings_available() || kde_available()
    }

    fn enable_gnome(http_port: u16, socks_port: u16) -> Result<(), String> {
        run(
            "gsettings",
            &["set", "org.gnome.system.proxy", "mode", "manual"],
        )?;
        for key in ["http", "https"] {
            run(
                "gsettings",
                &[
                    "set",
                    &format!("org.gnome.system.proxy.{key}"),
                    "host",
                    "127.0.0.1",
                ],
            )?;
            run(
                "gsettings",
                &[
                    "set",
                    &format!("org.gnome.system.proxy.{key}"),
                    "port",
                    &http_port.to_string(),
                ],
            )?;
        }
        run(
            "gsettings",
            &["set", "org.gnome.system.proxy.socks", "host", "127.0.0.1"],
        )?;
        run(
            "gsettings",
            &[
                "set",
                "org.gnome.system.proxy.socks",
                "port",
                &socks_port.to_string(),
            ],
        )?;
        run(
            "gsettings",
            &[
                "set",
                "org.gnome.system.proxy",
                "ignore-hosts",
                "['localhost', '127.0.0.0/8', '::1', '10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16']",
            ],
        )?;
        Ok(())
    }

    fn enable_kde(http_port: u16, socks_port: u16) -> Result<(), String> {
        let program = if Command::new("kwriteconfig6").arg("--help").output().is_ok() {
            "kwriteconfig6"
        } else {
            "kwriteconfig5"
        };
        let http = format!("http://127.0.0.1 {http_port}");
        let socks = format!("socks://127.0.0.1 {socks_port}");
        for (key, value) in [
            ("ProxyType", "1".to_string()),
            ("httpProxy", http.clone()),
            ("httpsProxy", http),
            ("socksProxy", socks),
        ] {
            run(
                program,
                &[
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    key,
                    &value,
                ],
            )?;
        }
        Ok(())
    }

    pub fn enable(http_port: u16, socks_port: u16) -> Result<(), String> {
        if gsettings_available() {
            enable_gnome(http_port, socks_port)
        } else if kde_available() {
            enable_kde(http_port, socks_port)
        } else {
            Err("автоматическая настройка системного прокси не поддерживается в этом окружении. Укажите HTTP-прокси 127.0.0.1 вручную".into())
        }
    }

    fn disable_gnome() -> Result<(), String> {
        run(
            "gsettings",
            &["set", "org.gnome.system.proxy", "mode", "none"],
        )
    }

    fn disable_kde() -> Result<(), String> {
        let program = if Command::new("kwriteconfig6").arg("--help").output().is_ok() {
            "kwriteconfig6"
        } else {
            "kwriteconfig5"
        };
        run(
            program,
            &[
                "--file",
                "kioslaverc",
                "--group",
                "Proxy Settings",
                "--key",
                "ProxyType",
                "0",
            ],
        )
    }

    pub fn disable() -> Result<(), String> {
        if gsettings_available() {
            disable_gnome()
        } else if kde_available() {
            disable_kde()
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    fn run(program: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn services() -> Vec<String> {
        let output = match Command::new("networksetup")
            .arg("-listallnetworkservices")
            .output()
        {
            Ok(output) => output,
            Err(_) => return Vec::new(),
        };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('*'))
            .map(str::to_string)
            .collect()
    }

    pub fn supported() -> bool {
        true
    }

    pub fn enable(http_port: u16, socks_port: u16) -> Result<(), String> {
        let port = http_port.to_string();
        let socks = socks_port.to_string();
        let mut any = false;
        for service in services() {
            let service = service.as_str();
            let _ = run(
                "networksetup",
                &["-setwebproxy", service, "127.0.0.1", &port],
            );
            let _ = run(
                "networksetup",
                &["-setsecurewebproxy", service, "127.0.0.1", &port],
            );
            let _ = run(
                "networksetup",
                &["-setsocksfirewallproxy", service, "127.0.0.1", &socks],
            );
            let _ = run("networksetup", &["-setwebproxystate", service, "on"]);
            let _ = run("networksetup", &["-setsecurewebproxystate", service, "on"]);
            let _ = run(
                "networksetup",
                &["-setsocksfirewallproxystate", service, "on"],
            );
            any = true;
        }
        if any {
            Ok(())
        } else {
            Err("не удалось определить сетевые сервисы macOS".into())
        }
    }

    pub fn disable() -> Result<(), String> {
        for service in services() {
            let service = service.as_str();
            let _ = run("networksetup", &["-setwebproxystate", service, "off"]);
            let _ = run("networksetup", &["-setsecurewebproxystate", service, "off"]);
            let _ = run(
                "networksetup",
                &["-setsocksfirewallproxystate", service, "off"],
            );
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod imp {
    pub fn supported() -> bool {
        false
    }
    pub fn enable(_: u16, _: u16) -> Result<(), String> {
        Err("системный прокси не поддерживается на этой платформе".into())
    }
    pub fn disable() -> Result<(), String> {
        Ok(())
    }
}
