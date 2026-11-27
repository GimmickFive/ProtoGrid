//! Single-instance guard with activation hand-off.
//!
//! Two ProtoGrid processes would each start their own core and fight over the
//! `protogrid_tun` adapter. The first process owns a platform lock and listens
//! for activation requests. A second launch pokes that listener, so the already
//! running window is restored and focused before the new process exits.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static ACTIVATION_PENDING: AtomicBool = AtomicBool::new(false);
static UI: OnceLock<egui::Context> = OnceLock::new();
/// URL подписки из deep link: переживает передачу второму экземпляру.
static PENDING_LINK: Mutex<Option<String>> = Mutex::new(None);

/// Lets the activation listener wake an idle UI immediately.
pub fn bind_ui(ctx: &egui::Context) {
    let _ = UI.set(ctx.clone());
}

/// Consumes one activation request from a second launch.
pub fn take_activation_request() -> bool {
    ACTIVATION_PENDING.swap(false, Ordering::AcqRel)
}

fn request_activation() {
    ACTIVATION_PENDING.store(true, Ordering::Release);
    if let Some(ctx) = UI.get() {
        ctx.request_repaint();
    }
}

/// Запоминает ссылку, которую должен получить запущенный экземпляр.
pub fn set_pending_link(link: &str) {
    *PENDING_LINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(link.to_owned());
}

fn pending_snapshot() -> Option<String> {
    PENDING_LINK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Забирает ссылку из deep link: сначала из памяти (первый запуск),
/// потом с диска (передача от второго экземпляра).
pub fn take_pending_link() -> Option<String> {
    if let Some(link) = PENDING_LINK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Some(link);
    }
    let path = handoff_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn handoff_path() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .map(|dir| dir.join("ProtoGrid").join("pending-link.txt"))
}

fn write_handoff(link: &str) -> bool {
    let Some(path) = handoff_path() else {
        return false;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, link).is_ok()
}

fn drop_handoff() {
    if let Some(path) = handoff_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(windows)]
mod platform {
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, MessageBoxW, ASFW_ANY, MB_ICONINFORMATION, MB_OK,
    };

    use super::request_activation;

    const MUTEX_NAME: &str = "Local\\ProtoGrid.Desktop.SingleInstance";
    const ACTIVATE_EVENT: &str = "Local\\ProtoGrid.Desktop.Activate";

    /// Kept for the whole process lifetime: closing the handle releases the mutex.
    pub struct SingleInstance {
        mutex: HANDLE,
    }

    impl Drop for SingleInstance {
        fn drop(&mut self) {
            if !self.mutex.is_null() {
                unsafe {
                    CloseHandle(self.mutex);
                }
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn acquire() -> Option<SingleInstance> {
        // The event is created before the mutex: if the mutex is already taken,
        // the owning process is guaranteed to have created its listener.
        let event_name = wide(ACTIVATE_EVENT);
        let event = unsafe { CreateEventW(ptr::null(), 0, 0, event_name.as_ptr()) };
        let mutex_name = wide(MUTEX_NAME);
        unsafe { SetLastError(0) };
        let mutex = unsafe { CreateMutexW(ptr::null(), 0, mutex_name.as_ptr()) };

        if mutex.is_null() {
            // Cannot create the mutex for a reason unrelated to another
            // instance (for example a restricted session). Do not block the
            // app in that case; TUN cleanup still protects the adapter.
            watch(event);
            return Some(SingleInstance { mutex });
        }

        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(mutex);
                // The freshly launched process is allowed to hand foreground
                // rights to the already running one.
                AllowSetForegroundWindow(ASFW_ANY);
                // Deep link: оставляем ссылку на диске, будим владельца событием.
                if let Some(link) = super::pending_snapshot() {
                    super::write_handoff(&link);
                }
                let signaled = !event.is_null() && SetEvent(event) != 0;
                if !event.is_null() {
                    CloseHandle(event);
                }
                if !signaled {
                    super::drop_handoff();
                    show_already_running();
                }
            }
            return None;
        }

        watch(event);
        Some(SingleInstance { mutex })
    }

    fn watch(event: HANDLE) {
        if event.is_null() {
            return;
        }
        // Raw Win32 handles are pointers and therefore not `Send`; the value
        // itself is safe to wait on from the watcher thread.
        let event = event as usize;
        let watcher = std::thread::Builder::new()
            .name("protogrid-activate".into())
            .spawn(move || loop {
                if unsafe { WaitForSingleObject(event as HANDLE, INFINITE) } != WAIT_OBJECT_0 {
                    break;
                }
                request_activation();
            });
        if watcher.is_err() {
            unsafe {
                CloseHandle(event as HANDLE);
            }
        }
    }

    fn show_already_running() {
        let text = "ProtoGrid уже запущен.\n\n\
                    Не удалось поднять окно работающего экземпляра. \
                    Откройте его из значка в трее.";
        let caption = "ProtoGrid";
        let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            MessageBoxW(
                ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    use super::request_activation;

    pub struct SingleInstance {
        path: PathBuf,
    }

    impl Drop for SingleInstance {
        fn drop(&mut self) {
            if !self.path.as_os_str().is_empty() {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    fn socket_path() -> PathBuf {
        let base = dirs::runtime_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir);
        // Keep the path short: macOS `sun_path` is limited to 104 bytes.
        base.join("protogrid-desktop.sock")
    }

    fn poke(path: &Path) -> bool {
        UnixStream::connect(path)
            .map(|mut stream| {
                let _ = stream.write_all(b"activate");
                true
            })
            .unwrap_or(false)
    }

    pub fn acquire() -> Option<SingleInstance> {
        let path = socket_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Deep link: второй экземпляр оставляет ссылку на диске и будит владельца.
        let handoff = super::pending_snapshot();
        if let Some(link) = handoff.as_deref() {
            super::write_handoff(link);
        }
        if poke(&path) {
            return None;
        }
        if handoff.is_some() {
            super::drop_handoff();
        }
        // A crashed process leaves its socket behind; replacing it is safe
        // because connect() above already proved nobody is listening.
        let _ = fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(_) => {
                // Another process won the bind race; hand the request to it.
                if poke(&path) {
                    return None;
                }
                // Cannot guard for a different reason. Start anyway instead of
                // blocking the app.
                return Some(SingleInstance {
                    path: PathBuf::new(),
                });
            }
        };
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        let listener_path = path.clone();
        let watcher = std::thread::Builder::new()
            .name("protogrid-activate".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stream.is_ok() {
                        request_activation();
                    }
                }
            });
        if watcher.is_err() {
            let _ = fs::remove_file(&listener_path);
            return Some(SingleInstance {
                path: PathBuf::new(),
            });
        }
        Some(SingleInstance { path })
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    pub struct SingleInstance;

    pub fn acquire() -> Option<SingleInstance> {
        Some(SingleInstance)
    }
}

pub use platform::acquire;
