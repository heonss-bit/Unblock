//! Разбор .bat-стратегий Flowseal в строку параметров для winws.exe.
//! Только стандартная библиотека: так эту часть можно проверять обычными тестами.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Services {
    Both,
    Youtube,
    Discord,
}

/// Достаёт из bat-файла параметры winws.exe: всё, что идёт после `winws.exe"`
/// в строке `start ...`, с учётом переносов через `^`.
pub fn extract_winws_args(bat: &str) -> Option<String> {
    let mut args = String::new();
    let mut found = false;
    for raw in bat.lines() {
        let line = raw.trim();
        let piece: &str;
        if !found {
            let low = line.to_ascii_lowercase();
            if !low.starts_with("start") {
                continue;
            }
            let key = "winws.exe\"";
            let pos = match low.find(key) {
                Some(p) => p + key.len(),
                None => continue,
            };
            piece = &line[pos..];
            found = true;
        } else {
            piece = line;
        }
        let (body, more) = match piece.strip_suffix('^') {
            Some(b) => (b, true),
            None => (piece, false),
        };
        let body = body.trim();
        if !body.is_empty() {
            if !args.is_empty() {
                args.push(' ');
            }
            args.push_str(body);
        }
        if !more {
            break;
        }
    }
    if found && !args.trim().is_empty() {
        Some(args)
    } else {
        None
    }
}

/// Делит строку по пробелам вне кавычек. Кавычки остаются на месте.
pub fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in s.chars() {
        if ch == '"' {
            quoted = !quoted;
            cur.push(ch);
        } else if ch.is_whitespace() && !quoted {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Оставляет в стратегии только нужные сервисы.
/// Стратегия состоит из профилей, разделённых `--new`. Профили YouTube используют
/// list-google.txt, профили Discord: list-general.txt и слово discord.
/// Вызывать нужно ДО подстановки путей, чтобы имя папки пользователя не мешало.
pub fn filter_services(args: &str, svc: Services) -> Result<String, String> {
    let tokens = tokenize(args);
    let mut globals: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() && tokens[i].starts_with("--wf-") {
        globals.push(tokens[i].clone());
        i += 1;
    }
    let mut segments: Vec<Vec<String>> = vec![Vec::new()];
    for t in &tokens[i..] {
        if t == "--new" {
            segments.push(Vec::new());
        } else if let Some(last) = segments.last_mut() {
            last.push(t.clone());
        }
    }
    segments.retain(|s| !s.is_empty());

    let keep = |seg: &Vec<String>| -> bool {
        let text = seg.join(" ").to_ascii_lowercase();
        match svc {
            Services::Both => true,
            Services::Youtube => !text.contains("discord") && !text.contains("list-general"),
            Services::Discord => !text.contains("list-google"),
        }
    };
    let kept: Vec<Vec<String>> = segments.into_iter().filter(|s| keep(s)).collect();
    if kept.is_empty() {
        return Err(
            "В этой стратегии нет правил для выбранного сервиса. Выбери другую стратегию.".into(),
        );
    }

    let mut parts = globals;
    for (n, seg) in kept.into_iter().enumerate() {
        if n > 0 {
            parts.push("--new".to_string());
        }
        parts.extend(seg);
    }
    Ok(parts.join(" "))
}

/// Подставляет переменные bat-файла. Пути `bin` и `lists` должны заканчиваться на `\`.
pub fn substitute(args: &str, root: &str, bin: &str, lists: &str) -> Result<String, String> {
    let t = args
        .replace("%GameFilterTCP%", "12")
        .replace("%GameFilterUDP%", "12")
        .replace("%GameFilter%", "12");
    let probe = t
        .replace("%BIN%", "")
        .replace("%LISTS%", "")
        .replace("%~dp0", "");
    if let Some(p) = probe.find('%') {
        let snippet: String = probe[p..].chars().take(24).collect();
        return Err(format!(
            "Стратегия использует неизвестную переменную: {snippet}"
        ));
    }
    Ok(t.replace("%BIN%", bin)
        .replace("%LISTS%", lists)
        .replace("%~dp0", root))
}

/// Всё вместе: bat-файл, сервисы, пути, готовая командная строка для winws.exe.
pub fn build(
    bat: &str,
    svc: Services,
    root: &str,
    bin: &str,
    lists: &str,
) -> Result<String, String> {
    let args = extract_winws_args(bat)
        .ok_or_else(|| "В файле стратегии не нашёл запуск winws.exe".to_string())?;
    let filtered = filter_services(&args, svc)?;
    substitute(&filtered, root, bin, lists)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@echo off\r\ncall service.bat status_zapret\r\n\r\nset \"BIN=%~dp0bin\\\"\r\n\r\n\
start \"zapret: %~n0\" /min \"%BIN%winws.exe\" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443,%GameFilterUDP% ^\r\n\
--filter-udp=443 --hostlist=\"%LISTS%list-general.txt\" --dpi-desync=fake --new ^\r\n\
--filter-udp=19294-19344 --filter-l7=discord,stun --dpi-desync=fake --new ^\r\n\
--filter-tcp=443 --hostlist=\"%LISTS%list-google.txt\" --dpi-desync=multisplit --new ^\r\n\
--filter-tcp=80,443 --hostlist=\"%LISTS%list-general.txt\" --dpi-desync=multisplit\r\n\
echo done\r\n";

    #[test]
    fn extracts_and_joins_lines() {
        let a = extract_winws_args(SAMPLE).unwrap();
        assert!(a.starts_with("--wf-tcp=80,443,%GameFilterTCP%"));
        assert!(a.ends_with("--dpi-desync=multisplit"));
        assert!(!a.contains('^'));
        assert!(!a.contains("echo"));
    }

    #[test]
    fn youtube_only_keeps_google_profile() {
        let a = extract_winws_args(SAMPLE).unwrap();
        let f = filter_services(&a, Services::Youtube).unwrap();
        assert!(f.contains("list-google.txt"));
        assert!(!f.contains("list-general"));
        assert!(!f.contains("discord"));
        assert!(f.starts_with("--wf-tcp="));
        assert!(!f.contains("--new"));
    }

    #[test]
    fn discord_only_drops_google_profile() {
        let a = extract_winws_args(SAMPLE).unwrap();
        let f = filter_services(&a, Services::Discord).unwrap();
        assert!(!f.contains("list-google"));
        assert!(f.contains("list-general.txt"));
        assert!(f.contains("discord"));
    }

    #[test]
    fn both_keeps_everything() {
        let a = extract_winws_args(SAMPLE).unwrap();
        assert_eq!(
            filter_services(&a, Services::Both).unwrap(),
            tokenize(&a).join(" ")
        );
    }

    #[test]
    fn substitutes_paths_with_spaces_and_odd_names() {
        let out = build(
            SAMPLE,
            Services::Youtube,
            "C:\\Users\\discord fan 100%\\U\\zapret\\",
            "C:\\Users\\discord fan 100%\\U\\zapret\\bin\\",
            "C:\\Users\\discord fan 100%\\U\\zapret\\lists\\",
        )
        .unwrap();
        assert!(out.contains(
            "--hostlist=\"C:\\Users\\discord fan 100%\\U\\zapret\\lists\\list-google.txt\""
        ));
        assert!(out.contains("--wf-tcp=80,443,12"));
    }

    #[test]
    fn unknown_variable_is_reported() {
        let e = substitute("--x=%FOO%bar", "r", "b", "l").unwrap_err();
        assert!(e.contains("%FOO%"));
    }

    #[test]
    fn tokenizer_keeps_quoted_spaces() {
        let t = tokenize("--a=\"x y\" --b --new");
        assert_eq!(t, vec!["--a=\"x y\"", "--b", "--new"]);
    }
}
