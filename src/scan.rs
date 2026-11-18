//! Подбор стратегии: по очереди запускаем каждую и проверяем, открываются ли
//! YouTube и Discord. Лучшей считается та, у которой открылось больше всего адресов,
//! при равенстве — та, что ответила быстрее.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::bat::Services;
use crate::{sys, zapret};

/// Адреса для проверки: и сайты, и видео-серверы YouTube, и Discord.
const TARGETS: [&str; 6] = [
    "https://www.youtube.com/",
    "https://i.ytimg.com/",
    "https://redirector.googlevideo.com/",
    "https://discord.com/",
    "https://gateway.discord.gg/",
    "https://cdn.discordapp.com/",
];

#[derive(Clone, Debug)]
pub struct Score {
    pub name: String,
    /// Сколько адресов открылось.
    pub ok: u8,
    pub total: u8,
    /// Суммарное время ответов успешных запросов, мс.
    pub ms: u32,
}

/// Индекс лучшей стратегии: больше открытых адресов, затем меньше время. Нужен хотя бы один успех.
pub fn best(scores: &[Score]) -> Option<usize> {
    scores
        .iter()
        .enumerate()
        .filter(|(_, s)| s.ok > 0)
        .min_by_key(|(_, s)| (u8::MAX - s.ok, s.ms))
        .map(|(i, _)| i)
}

/// Открывается ли адрес: любой ответ сервера (даже 403) значит, что соединение прошло.
fn probe(url: &'static str) -> Option<u32> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let t0 = Instant::now();
    let reached = match agent.get(url).set("User-Agent", "Mozilla/5.0").call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    };
    if reached {
        Some(t0.elapsed().as_millis() as u32)
    } else {
        None
    }
}

fn check_all() -> (u8, u32) {
    let handles: Vec<_> = TARGETS
        .iter()
        .map(|u| {
            let u: &'static str = *u;
            thread::spawn(move || probe(u))
        })
        .collect();
    let mut ok = 0u8;
    let mut ms = 0u32;
    for h in handles {
        if let Ok(Some(t)) = h.join() {
            ok += 1;
            ms = ms.saturating_add(t);
        }
    }
    (ok, ms)
}

/// Проверяет все стратегии. `progress(номер, всего, имя)` вызывается перед каждой.
/// После проверки winws остановлен.
pub fn run(
    root: &Path,
    names: &[String],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Vec<Score> {
    let mut out = Vec::new();
    let total = names.len();
    for (i, name) in names.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        progress(i + 1, total, name);
        let mut score = Score {
            name: name.clone(),
            ok: 0,
            total: TARGETS.len() as u8,
            ms: 0,
        };
        if let Ok(mut child) = zapret::start(root, name, Services::Both) {
            // Даём winws время запуститься и подцепить драйвер.
            thread::sleep(Duration::from_millis(2500));
            let alive = matches!(child.try_wait(), Ok(None));
            if alive && !cancel.load(Ordering::SeqCst) {
                let (ok, ms) = check_all();
                score.ok = ok;
                score.ms = ms;
            }
            let _ = child.kill();
            let _ = child.wait();
            if cancel.load(Ordering::SeqCst) {
                break;
            }
        }
        out.push(score);
    }
    sys::kill_image("winws.exe");
    sys::stop_windivert();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str, ok: u8, ms: u32) -> Score {
        Score {
            name: name.into(),
            ok,
            total: 6,
            ms,
        }
    }

    #[test]
    fn best_prefers_more_ok_then_faster() {
        let v = vec![s("a", 3, 100), s("b", 6, 900), s("c", 6, 400), s("d", 0, 1)];
        assert_eq!(best(&v), Some(2));
    }

    #[test]
    fn best_none_when_all_failed() {
        assert_eq!(best(&[s("a", 0, 0)]), None);
        assert_eq!(best(&[]), None);
    }
}
