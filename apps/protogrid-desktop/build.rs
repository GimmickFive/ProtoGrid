use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "ProtoGrid");
        resource.set("FileDescription", "ProtoGrid — native proxy client");
        resource.set("LegalCopyright", "MIT");
        if let Err(error) = resource.compile() {
            eprintln!("не удалось встроить иконку Windows: {error}");
        }
    }

    emit_bundled_core();
}

/// Embed the ProtoGrid core into the application so the release build is a
/// single self-contained executable. The binary comes from `dist/fork/`; when
/// it is absent (clean checkout, unsupported platform) the app falls back to
/// the regular file lookup.
fn emit_bundled_core() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.clone());
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let file = match (os.as_str(), arch.as_str()) {
        ("windows", "x86_64") => Some("sing-box-protogrid-windows-amd64.exe"),
        ("linux", "x86_64") => Some("sing-box-protogrid-linux-amd64"),
        _ => None,
    };
    let explicit = std::env::var_os("PROTOGRID_CORE_PATH").map(PathBuf::from);
    let path = explicit.or_else(|| file.map(|name| root.join("dist").join("fork").join(name)));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bundled_core.rs");
    let mut source = match path {
        Some(path) if path.is_file() => {
            println!("cargo:rerun-if-changed={}", path.display());
            let literal = path.to_string_lossy().replace('\\', "/");
            format!(
                "/// Embedded ProtoGrid core, extracted on first launch.\npub static BUNDLED_CORE: Option<&'static [u8]> = Some(include_bytes!(r#\"{literal}\"#));\n"
            )
        }
        _ => "/// No core embedded in this build.\npub static BUNDLED_CORE: Option<&'static [u8]> = None;\n"
            .to_string(),
    };
    let wintun = if os == "windows" {
        root.join("dist").join("fork").join("wintun.dll")
    } else {
        PathBuf::new()
    };
    if wintun.is_file() {
        println!("cargo:rerun-if-changed={}", wintun.display());
        let literal = wintun.to_string_lossy().replace('\\', "/");
        source.push_str(&format!(
            "/// Embedded wintun.dll for TUN mode.\npub static BUNDLED_WINTUN: Option<&'static [u8]> = Some(include_bytes!(r#\"{literal}\"#));\n"
        ));
    } else {
        source.push_str("pub static BUNDLED_WINTUN: Option<&'static [u8]> = None;\n");
    }
    let _ = std::fs::write(out, source);
}
