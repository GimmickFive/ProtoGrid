//! Deep links `protogrid://`.
//!
//! Формы, которые понимаем:
//!
//! * `protogrid://add?url=<percent-encoded https://…>`
//! * `protogrid://add/<percent-encoded https://…>`
//! * `protogrid://import?url=…`
//!
//! Ссылка несёт только URL подписки: открытие окна и импорт делает уже
//! запущенный экземпляр (см. `single_instance`). Схема регистрируется в ОС при
//! старте: Windows — HKCU\Software\Classes, Linux — `.desktop` + `xdg-mime`.

/// Имя схемы без `://`.
pub const SCHEME: &str = "protogrid";

/// Ищет ссылку в аргументах командной строки (так ОС передаёт её обработчику).
pub fn subscription_from_args() -> Option<String> {
    std::env::args().skip(1).find_map(|arg| parse(&arg))
}

/// Разбирает `protogrid://…` и возвращает URL подписки.
pub fn parse(link: &str) -> Option<String> {
    let lower = link.get(..SCHEME.len() + 3)?.to_ascii_lowercase();
    if lower != format!("{SCHEME}://") {
        return None;
    }
    let rest = &link[SCHEME.len() + 3..];
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, query),
        None => (rest, ""),
    };

    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("url=") {
            if let Some(url) = decode(value).filter(|url| is_subscription_url(url)) {
                return Some(url);
            }
        }
    }

    let (action, tail) = path.split_once('/').unwrap_or((path, ""));
    if matches!(action, "add" | "import" | "install" | "install-sub") {
        let tail = tail.trim_end_matches('/');
        if !tail.is_empty() {
            if let Some(url) = decode(tail).filter(|url| is_subscription_url(url)) {
                return Some(url);
            }
        }
    }
    None
}

/// Подписка — это HTTP(S)-URL; file:// и прочее не принимаем.
fn is_subscription_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Минимальный percent-decode: `+` не трогаем, он бывает в токенах подписки.
fn decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    let text = String::from_utf8(out).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// Регистрирует обработчик схемы. Идемпотентно, без прав администратора.
pub fn register_scheme() {
    #[cfg(windows)]
    register_windows();
    #[cfg(target_os = "linux")]
    register_linux();
}

#[cfg(windows)]
fn register_windows() {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe = exe.display().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let Ok((root, _)) = hkcu.create_subkey(format!("Software\\Classes\\{SCHEME}")) else {
        return;
    };
    let _ = root.set_value("", &"URL:ProtoGrid Protocol");
    let _ = root.set_value("URL Protocol", &"");

    if let Ok((icon, _)) = hkcu.create_subkey(format!("Software\\Classes\\{SCHEME}\\DefaultIcon")) {
        let _ = icon.set_value("", &format!("\"{exe}\",0"));
    }
    if let Ok((cmd, _)) =
        hkcu.create_subkey(format!("Software\\Classes\\{SCHEME}\\shell\\open\\command"))
    {
        // %1 — ссылка, которую ОС передаёт обработчику аргументом.
        let _ = cmd.set_value("", &format!("\"{exe}\" \"%1\""));
    }
}

#[cfg(target_os = "linux")]
fn register_linux() {
    let Some(data) = dirs::data_dir() else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let apps = data.join("applications");
    if std::fs::create_dir_all(&apps).is_err() {
        return;
    }
    let entry = apps.join("protogrid-handler.desktop");
    let body = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=ProtoGrid\n\
         Comment=Открыть ссылку ProtoGrid\n\
         Exec=\"{}\" %u\n\
         Terminal=false\n\
         NoDisplay=true\n\
         MimeType=x-scheme-handler/{SCHEME};\n",
        exe.display()
    );
    if std::fs::write(&entry, body).is_err() {
        return;
    }
    let _ = std::process::Command::new("xdg-mime")
        .args([
            "default",
            "protogrid-handler.desktop",
            &format!("x-scheme-handler/{SCHEME}"),
        ])
        .output();
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps)
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_query_form() {
        let link = "protogrid://add?url=https%3A%2F%2Fsub.dlspeed.ru%2F0hw%2Fabc";
        assert_eq!(
            parse(link).as_deref(),
            Some("https://sub.dlspeed.ru/0hw/abc")
        );
    }

    #[test]
    fn parses_path_form() {
        let link = "protogrid://add/https%3A%2F%2Fsub.example.org%2Fx%3Fa%3D1%26b%3D2";
        assert_eq!(
            parse(link).as_deref(),
            Some("https://sub.example.org/x?a=1&b=2")
        );
    }

    #[test]
    fn rejects_other_schemes_and_paths() {
        assert!(parse("https://example.org/").is_none());
        assert!(parse("protogrid://open").is_none());
        assert!(parse("protogrid://add?url=file%3A%2F%2F%2Fetc%2Fpasswd").is_none());
        assert!(parse("protogrid://add?url=").is_none());
    }

    #[test]
    fn keeps_plus_and_uppercase_scheme() {
        let link = "PROTOGRID://add?url=https%3A%2F%2Fa.example%2Fp%3Ft%3Da%2Bb";
        assert_eq!(parse(link).as_deref(), Some("https://a.example/p?t=a+b"));
    }
}
