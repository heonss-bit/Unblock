//! Движки Flowseal: где лежат и как скачиваются.
//! Берём только из официальных репозиториев Flowseal на GitHub (адреса зашиты в код).

use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::time::Duration;

pub const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
pub const TG_REPO: &str = "Flowseal/tg-ws-proxy";
pub const TG_EXE: &str = "TgWsProxy_windows.exe";

const MAX_DOWNLOAD: usize = 200 * 1024 * 1024;

// ---------------------------------------------------------------- пути

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Unblock")
}

pub fn zapret_dir() -> PathBuf {
    data_dir().join("zapret")
}

pub fn tg_dir() -> PathBuf {
    data_dir().join("tg")
}

/// Рядом с exe прокси эта папка включает его portable-режим: настройки лежат в ней.
pub fn tg_data_dir() -> PathBuf {
    tg_dir().join("TgWsProxy_data")
}

pub fn tg_exe() -> PathBuf {
    tg_dir().join(TG_EXE)
}

pub fn tg_installed() -> bool {
    tg_exe().is_file()
}

/// Папка, где лежит bin\winws.exe (в архиве файлы могут быть в подпапке).
pub fn zapret_root() -> Option<PathBuf> {
    let base = zapret_dir();
    if base.join("bin").join("winws.exe").is_file() {
        return Some(base);
    }
    for e in fs::read_dir(&base).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() && p.join("bin").join("winws.exe").is_file() {
            return Some(p);
        }
    }
    None
}

pub fn zapret_installed() -> bool {
    zapret_root().is_some()
}

// ---------------------------------------------------------------- сеть

pub struct Release {
    pub tag: String,
    pub url: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent("Unblock")
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .build()
}

/// Последний релиз репозитория; `pick` выбирает нужный файл по имени.
fn latest_release(repo: &str, pick: &dyn Fn(&str) -> bool) -> Result<Release, String> {
    let api = format!("https://api.github.com/repos/{repo}/releases/latest");
    let v: serde_json::Value = agent()
        .get(&api)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Не удалось связаться с GitHub ({repo}): {e}"))?
        .into_json()
        .map_err(|e| format!("Ответ GitHub не удалось прочитать: {e}"))?;
    let tag = v["tag_name"].as_str().unwrap_or("?").to_string();
    let assets = v["assets"]
        .as_array()
        .ok_or_else(|| format!("В релизе {repo} нет файлов"))?;
    let prefix = format!("https://github.com/{repo}/");
    for a in assets {
        let name = a["name"].as_str().unwrap_or("");
        let url = a["browser_download_url"].as_str().unwrap_or("");
        if pick(name) && url.starts_with(&prefix) {
            return Ok(Release {
                tag,
                url: url.to_string(),
            });
        }
    }
    Err(format!("В релизе {repo} {tag} не нашёл нужный файл"))
}

fn download(url: &str, progress: &mut dyn FnMut(u64, u64)) -> Result<Vec<u8>, String> {
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("Не удалось скачать файл: {e}"))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut data: Vec<u8> = Vec::with_capacity((total as usize).min(64 * 1024 * 1024));
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("Загрузка оборвалась: {e}"))?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..n]);
        if data.len() > MAX_DOWNLOAD {
            return Err("Файл слишком большой, остановил загрузку".to_string());
        }
        progress(data.len() as u64, total);
    }
    if data.is_empty() {
        return Err("Скачался пустой файл".to_string());
    }
    Ok(data)
}

// ---------------------------------------------------------------- установка

/// Скачивает и распаковывает zapret от Flowseal. Возвращает версию (тег релиза).
pub fn install_zapret(progress: &mut dyn FnMut(u64, u64)) -> Result<String, String> {
    let rel = latest_release(ZAPRET_REPO, &|n| n.to_ascii_lowercase().ends_with(".zip"))?;
    let bytes = download(&rel.url, progress)?;

    let root = data_dir();
    fs::create_dir_all(&root)
        .map_err(|e| format!("Не удалось создать папку {}: {e}", root.display()))?;
    let tmp = root.join("zapret.tmp");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| format!("Не удалось создать временную папку: {e}"))?;

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Архив запрета повреждён: {e}"))?;
    archive
        .extract(&tmp)
        .map_err(|e| format!("Не удалось распаковать архив: {e}"))?;

    let dest = zapret_dir();
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest).map_err(|e| {
        format!("Не удалось заменить старую версию (выключи запрет и повтори): {e}")
    })?;
    if zapret_root().is_none() {
        return Err("В архиве не нашёл bin\\winws.exe".to_string());
    }
    Ok(rel.tag)
}

/// Скачивает exe TG WS Proxy от Flowseal. Возвращает версию (тег релиза).
pub fn install_tg(progress: &mut dyn FnMut(u64, u64)) -> Result<String, String> {
    let rel = latest_release(TG_REPO, &|n| n == TG_EXE).or_else(|_| {
        latest_release(TG_REPO, &|n| {
            n.starts_with("TgWsProxy_windows")
                && n.ends_with(".exe")
                && !n.contains("arm64")
                && !n.contains("7_32bit")
        })
    })?;
    let bytes = download(&rel.url, progress)?;
    if bytes.len() < 1_000_000 || !bytes.starts_with(b"MZ") {
        return Err("Скачанный файл не похож на программу Windows".to_string());
    }
    let dir = tg_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Не удалось создать папку {}: {e}", dir.display()))?;
    let tmp = dir.join(format!("{TG_EXE}.tmp"));
    fs::write(&tmp, &bytes).map_err(|e| format!("Не удалось сохранить файл: {e}"))?;
    fs::rename(&tmp, tg_exe()).map_err(|e| {
        format!("Не удалось заменить старую версию (выключи TG-прокси и повтори): {e}")
    })?;
    Ok(rel.tag)
}
