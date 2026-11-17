//! Запуск TG WS Proxy от Flowseal в его portable-режиме.
//! Рядом с exe лежит папка TgWsProxy_data: туда мы кладём настройки и отметки
//! «первый запуск уже был», поэтому окно с инструкцией у него не появляется.

use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::{engines, sys};

pub const PORT: u16 = 1443;
pub const HOST: &str = "127.0.0.1";

/// Ссылка для Telegram: открывает окно добавления прокси.
pub fn link(secret: &str) -> String {
    format!("tg://proxy?server={HOST}&port={PORT}&secret=dd{secret}")
}

fn prepare(secret: &str) -> Result<(), String> {
    let dir = engines::tg_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать папку прокси: {e}"))?;
    let cfg = serde_json::json!({
        "port": PORT,
        "host": HOST,
        "secret": secret,
        "check_updates": false,
        "autostart": false
    });
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(dir.join("config.json"), text)
        .map_err(|e| format!("Не удалось записать настройки прокси: {e}"))?;
    for marker in [".first_run_done_mtproto", ".ipv6_warned"] {
        let p = dir.join(marker);
        if !p.exists() {
            let _ = fs::write(p, b"");
        }
    }
    Ok(())
}

pub fn start(secret: &str) -> Result<Child, String> {
    let exe = engines::tg_exe();
    if !exe.is_file() {
        return Err(
            "Не нашёл TG-прокси. Нажми «Обновить движки» в меню ••• у «Запрета».".to_string(),
        );
    }
    prepare(secret)?;
    sys::kill_image(engines::TG_EXE); // остатки от прошлого запуска
    let mut cmd = Command::new(&exe);
    cmd.current_dir(engines::tg_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(sys::CREATE_NO_WINDOW);
    }
    cmd.spawn()
        .map_err(|e| format!("Не удалось запустить TG-прокси: {e}"))
}

/// Ждёт, пока прокси начнёт принимать подключения (первый запуск может занять несколько секунд).
pub fn wait_ready(timeout: Duration) -> bool {
    let addr: SocketAddr = format!("{HOST}:{PORT}").parse().expect("адрес прокси");
    let started = Instant::now();
    while started.elapsed() < timeout {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}
