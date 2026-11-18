//! Мелкие системные помощники: скрытые процессы, taskkill, ссылки, права, случайные байты.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Команда без окна консоли и без вывода.
pub fn hidden<S: AsRef<OsStr>>(program: S) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Завершает процесс вместе с дочерними (PyInstaller-exe запускает ещё один процесс).
pub fn kill_tree(pid: u32) {
    let p = pid.to_string();
    let _ = hidden("taskkill")
        .args(["/F", "/T", "/PID", p.as_str()])
        .status();
}

pub fn kill_image(name: &str) {
    let _ = hidden("taskkill").args(["/F", "/T", "/IM", name]).status();
}

/// Останавливает и убирает драйвер WinDivert (как это делает service.bat при удалении).
pub fn stop_windivert() {
    let _ = hidden("sc").args(["stop", "WinDivert"]).status();
    let _ = hidden("sc").args(["delete", "WinDivert"]).status();
}

/// Открывает ссылку (в том числе tg://) в программе по умолчанию.
pub fn open_url(url: &str) {
    let _ = hidden("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
}

pub fn open_folder(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}

/// Запущено ли приложение с правами администратора (fltmc работает только у админа).
pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        hidden("fltmc")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

const TASK_NAME: &str = "Unblock";

/// Есть ли задача автозапуска. Задача планировщика запускает приложение сразу
/// с правами администратора, поэтому Windows не спрашивает подтверждение при каждом входе.
pub fn autostart_enabled() -> bool {
    hidden("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    let ok = if on {
        let exe = std::env::current_exe().map_err(|e| format!("Не нашёл путь к программе: {e}"))?;
        let cmd = format!("\"{}\" --autostart", exe.display());
        hidden("schtasks")
            .args([
                "/Create",
                "/TN",
                TASK_NAME,
                "/TR",
                cmd.as_str(),
                "/SC",
                "ONLOGON",
                "/RL",
                "HIGHEST",
                "/F",
            ])
            .status()
    } else {
        hidden("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .status()
    }
    .map(|s| s.success())
    .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err("Не удалось изменить автозапуск (нужны права администратора).".to_string())
    }
}

#[cfg(windows)]
mod win {
    type Hwnd = isize;

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(cb: extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
        fn GetWindowThreadProcessId(h: Hwnd, pid: *mut u32) -> u32;
        fn GetWindowTextW(h: Hwnd, buf: *mut u16, max: i32) -> i32;
        fn ShowWindow(h: Hwnd, cmd: i32) -> i32;
        fn SetForegroundWindow(h: Hwnd) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcessId() -> u32;
    }

    extern "system" fn each(h: Hwnd, lparam: isize) -> i32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        if pid == unsafe { GetCurrentProcessId() } {
            let mut buf = [0u16; 64];
            let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), 64) };
            if n > 0 && String::from_utf16_lossy(&buf[..n as usize]) == "Unblock" {
                unsafe { *(lparam as *mut isize) = h };
                return 0; // нашли, перебор можно остановить
            }
        }
        1
    }

    /// Главное окно этого процесса (по заголовку «Unblock»; окно проводника с таким же именем чужое).
    fn main_window() -> Option<Hwnd> {
        let mut found: isize = 0;
        unsafe { EnumWindows(each, &mut found as *mut isize as isize) };
        if found != 0 {
            Some(found)
        } else {
            None
        }
    }

    pub fn hide() {
        if let Some(h) = main_window() {
            unsafe { ShowWindow(h, 0) }; // SW_HIDE
        }
    }

    pub fn show() {
        if let Some(h) = main_window() {
            unsafe {
                ShowWindow(h, 5); // SW_SHOW
                ShowWindow(h, 9); // SW_RESTORE (если было свёрнуто)
                SetForegroundWindow(h);
            }
        }
    }
}

/// Прячет главное окно (программа остаётся в трее).
pub fn hide_main_window() {
    #[cfg(windows)]
    win::hide();
}

/// Показывает главное окно из трея.
pub fn show_main_window() {
    #[cfg(windows)]
    win::show();
}

pub fn valid_secret(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// 16 случайных байт в виде 32 hex-символов (секрет для ссылки на прокси).
pub fn random_hex16() -> String {
    let mut b = [0u8; 16];
    fill_random(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(windows)]
fn fill_random(buf: &mut [u8]) {
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(alg: *mut core::ffi::c_void, buf: *mut u8, len: u32, flags: u32) -> i32;
    }
    // 2 = BCRYPT_USE_SYSTEM_PREFERRED_RNG
    let r =
        unsafe { BCryptGenRandom(core::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32, 2) };
    if r != 0 {
        fallback_random(buf);
    }
}

#[cfg(not(windows))]
fn fill_random(buf: &mut [u8]) {
    use std::io::Read;
    let ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(buf))
        .is_ok();
    if !ok {
        fallback_random(buf);
    }
}

fn fallback_random(buf: &mut [u8]) {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    for chunk in buf.chunks_mut(8) {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(nanos);
        let v = h.finish().to_le_bytes();
        chunk.copy_from_slice(&v[..chunk.len()]);
    }
}
