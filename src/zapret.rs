//! Запуск и остановка winws.exe (движок zapret) по стратегии из релиза Flowseal.

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::bat::{self, Services};
use crate::{engines, sys};

/// Файлы стратегий (general*.bat), «general.bat» первым.
pub fn strategies(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| {
                    let l = n.to_ascii_lowercase();
                    l.starts_with("general") && l.ends_with(".bat")
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| {
        let ka = (a != "general.bat", a.to_ascii_lowercase());
        let kb = (b != "general.bat", b.to_ascii_lowercase());
        ka.cmp(&kb)
    });
    v
}

/// Файлы-списки пользователя, которые ждёт winws (их создаёт service.bat).
fn ensure_user_lists(root: &Path) -> Result<(), String> {
    let lists = root.join("lists");
    fs::create_dir_all(&lists).map_err(|e| format!("Не удалось создать папку lists: {e}"))?;
    let defaults = [
        ("ipset-exclude-user.txt", "203.0.113.113/32\r\n"),
        (
            "list-general-user.txt",
            "# Never leave this file empty\r\ndomain.example.abc\r\n",
        ),
        ("list-exclude-user.txt", "domain.example.abc\r\n"),
    ];
    for (name, body) in defaults {
        let p = lists.join(name);
        if !p.exists() {
            fs::write(&p, body).map_err(|e| format!("Не удалось создать {name}: {e}"))?;
        }
    }
    Ok(())
}

fn with_slash(p: &Path) -> String {
    let mut s = p.display().to_string();
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

fn command_line(root: &Path, strategy: &str, svc: Services) -> Result<String, String> {
    let file = root.join(strategy);
    let bytes =
        fs::read(&file).map_err(|e| format!("Не удалось прочитать стратегию {strategy}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    bat::build(
        &text,
        svc,
        &with_slash(root),
        &with_slash(&root.join("bin")),
        &with_slash(&root.join("lists")),
    )
}

/// Запускает winws.exe без окна. Вывод пишется в zapret.log рядом с настройками.
pub fn start(root: &Path, strategy: &str, svc: Services) -> Result<Child, String> {
    ensure_user_lists(root)?;
    let cmdline = command_line(root, strategy, svc)?;
    sys::kill_image("winws.exe"); // остатки от прошлого запуска

    let winws = root.join("bin").join("winws.exe");
    if !winws.is_file() {
        return Err("Не нашёл winws.exe. Нажми «Обновить движки» в меню ••• .".to_string());
    }

    let mut cmd = Command::new(&winws);
    cmd.current_dir(root.join("bin")).stdin(Stdio::null());
    match fs::File::create(engines::data_dir().join("zapret.log")) {
        Ok(f) => {
            let f2 = f.try_clone().map_err(|e| e.to_string())?;
            cmd.stdout(Stdio::from(f)).stderr(Stdio::from(f2));
        }
        Err(_) => {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(sys::CREATE_NO_WINDOW);
        // Строка параметров передаётся как есть, ровно как это делает bat-файл.
        cmd.raw_arg(cmdline);
    }
    #[cfg(not(windows))]
    {
        let _ = cmdline;
        return Err("Запрет работает только в Windows".to_string());
    }

    #[cfg(windows)]
    {
        cmd.spawn()
            .map_err(|e| format!("Не удалось запустить winws.exe: {e}"))
    }
}
