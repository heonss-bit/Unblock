//! Главное окно Unblock: слева «Запрет», справа «TG-прокси», внизу «Включить обе функции».

use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Align2, Color32, Layout, RichText, Ui, Vec2};

use crate::settings::{Mode, Settings};
use crate::{engines, sys, tgproxy, ui, zapret};

/// Сообщения из фоновых потоков в окно.
enum Msg {
    Progress {
        label: &'static str,
        done: u64,
        total: u64,
    },
    Installed {
        zapret: Option<String>,
        tg: Option<String>,
    },
    InstallFailed(String),
    TgReady(bool),
}

#[derive(Clone, Copy)]
enum Run {
    Off,
    Starting(Instant),
    On,
}

impl Run {
    fn is_off(self) -> bool {
        matches!(self, Run::Off)
    }
}

enum Engines {
    Installing {
        label: &'static str,
        done: u64,
        total: u64,
    },
    Failed(String),
    Ready,
}

/// Действия пользователя. Окно только собирает их, а выполняются они после отрисовки.
enum Act {
    ToggleHint,
    ToggleZapret,
    ToggleTg,
    ToggleAll,
    SetMode(Mode),
    SetStrategy(String),
    UpdateEngines,
    Retry,
    CopyLink,
    OpenTelegram,
    OpenLogs,
}

pub struct App {
    s: Settings,
    z: Option<Child>,
    z_run: Run,
    t: Option<Child>,
    t_run: Run,
    engines: Engines,
    admin: bool,
    strategies: Vec<String>,
    toast: Option<(String, Instant)>,
    hint: bool,
    down: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui::apply(&cc.egui_ctx);
        let mut s = Settings::load();
        if !sys::valid_secret(&s.secret) {
            s.secret = sys::random_hex16();
            s.save();
        }
        let (tx, rx) = channel();
        let mut app = App {
            s,
            z: None,
            z_run: Run::Off,
            t: None,
            t_run: Run::Off,
            engines: Engines::Ready,
            admin: sys::is_admin(),
            strategies: Vec::new(),
            toast: None,
            hint: false,
            down: false,
            tx,
            rx,
        };
        app.refresh_strategies();
        // Первый запуск: движков ещё нет, скачиваем их с GitHub Flowseal.
        app.spawn_install(&cc.egui_ctx, false);
        app
    }

    // ------------------------------------------------------------ состояние

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn refresh_strategies(&mut self) {
        self.strategies = engines::zapret_root()
            .map(|r| zapret::strategies(&r))
            .unwrap_or_default();
        if !self.strategies.is_empty() && !self.strategies.contains(&self.s.strategy) {
            self.s.strategy = self.strategies[0].clone();
        }
    }

    fn spawn_install(&mut self, ctx: &egui::Context, force: bool) {
        let need_zapret = force || !engines::zapret_installed();
        let need_tg = force || !engines::tg_installed();
        if !need_zapret && !need_tg {
            self.engines = Engines::Ready;
            return;
        }
        self.engines = Engines::Installing {
            label: "Подготовка…",
            done: 0,
            total: 0,
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let send = |m: Msg| {
                let _ = tx.send(m);
                ctx.request_repaint();
            };
            let mut zapret_tag = None;
            let mut tg_tag = None;
            if need_zapret {
                let mut cb = |done: u64, total: u64| {
                    send(Msg::Progress {
                        label: "Запрет (zapret)",
                        done,
                        total,
                    })
                };
                match engines::install_zapret(&mut cb) {
                    Ok(t) => zapret_tag = Some(t),
                    Err(e) => {
                        send(Msg::InstallFailed(e));
                        return;
                    }
                }
            }
            if need_tg {
                let mut cb = |done: u64, total: u64| {
                    send(Msg::Progress {
                        label: "TG-прокси",
                        done,
                        total,
                    })
                };
                match engines::install_tg(&mut cb) {
                    Ok(t) => tg_tag = Some(t),
                    Err(e) => {
                        send(Msg::InstallFailed(e));
                        return;
                    }
                }
            }
            send(Msg::Installed {
                zapret: zapret_tag,
                tg: tg_tag,
            });
        });
    }

    // ------------------------------------------------------------ запрет

    fn start_zapret(&mut self) {
        if !self.admin {
            self.toast(
                "Нужны права администратора: закрой Unblock и запусти его от имени администратора.",
            );
            return;
        }
        let Some(root) = engines::zapret_root() else {
            self.toast("Движок запрета ещё не скачан.");
            return;
        };
        match zapret::start(&root, &self.s.strategy, self.s.mode.services()) {
            Ok(child) => {
                self.z = Some(child);
                self.z_run = Run::Starting(Instant::now());
            }
            Err(e) => self.toast(e),
        }
    }

    fn stop_zapret(&mut self) {
        if let Some(mut c) = self.z.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.z_run = Run::Off;
    }

    // ------------------------------------------------------------ TG-прокси

    fn start_tg(&mut self, ctx: &egui::Context) {
        match tgproxy::start(&self.s.secret) {
            Ok(child) => {
                self.t = Some(child);
                self.t_run = Run::Starting(Instant::now());
                let tx = self.tx.clone();
                let ctx = ctx.clone();
                thread::spawn(move || {
                    let ok = tgproxy::wait_ready(Duration::from_secs(25));
                    let _ = tx.send(Msg::TgReady(ok));
                    ctx.request_repaint();
                });
            }
            Err(e) => self.toast(e),
        }
    }

    fn stop_tg(&mut self) {
        if let Some(mut c) = self.t.take() {
            sys::kill_tree(c.id());
            let _ = c.kill();
            let _ = c.wait();
        }
        sys::kill_image(engines::TG_EXE);
        self.t_run = Run::Off;
    }

    /// Выключает всё и убирает драйвер WinDivert. Безопасно вызывать несколько раз.
    fn shutdown(&mut self) {
        if self.down {
            return;
        }
        self.down = true;
        let had_zapret = self.z.is_some();
        self.stop_zapret();
        self.stop_tg();
        if had_zapret {
            sys::stop_windivert();
        }
    }

    // ------------------------------------------------------------ события

    fn poll(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Progress { label, done, total } => {
                    self.engines = Engines::Installing { label, done, total };
                }
                Msg::Installed { zapret, tg } => {
                    if let Some(t) = zapret {
                        self.s.zapret_tag = t;
                    }
                    if let Some(t) = tg {
                        self.s.tg_tag = t;
                    }
                    self.refresh_strategies();
                    self.s.save();
                    self.engines = Engines::Ready;
                    self.toast("Движки Flowseal готовы");
                }
                Msg::InstallFailed(e) => self.engines = Engines::Failed(e),
                Msg::TgReady(ok) => {
                    if matches!(self.t_run, Run::Starting(_)) {
                        if ok {
                            self.t_run = Run::On;
                        } else {
                            self.stop_tg();
                            self.toast("TG-прокси не запустился. Проверь, что порт 1443 свободен и антивирус не блокирует файл.");
                        }
                    }
                }
            }
        }

        let z_status = self.z.as_mut().map(|c| c.try_wait());
        match z_status {
            Some(Ok(Some(status))) => {
                self.z = None;
                self.z_run = Run::Off;
                self.toast(format!(
                    "Запрет остановился ({status}). Подробности в файле zapret.log: меню ••• → «Открыть папку с логами»."
                ));
            }
            Some(Ok(None)) => {
                if let Run::Starting(t0) = self.z_run {
                    if t0.elapsed() > Duration::from_millis(1500) {
                        self.z_run = Run::On;
                    }
                }
            }
            _ => {}
        }

        let t_status = self.t.as_mut().map(|c| c.try_wait());
        if let Some(Ok(Some(_))) = t_status {
            self.t = None;
            self.t_run = Run::Off;
            self.toast("TG-прокси закрылся. Возможно, он уже запущен отдельно.");
        }

        if let Some((_, at)) = &self.toast {
            if at.elapsed() > Duration::from_secs(6) {
                self.toast = None;
            }
        }
    }

    fn handle(&mut self, a: Act, ctx: &egui::Context) {
        match a {
            Act::ToggleHint => self.hint = !self.hint,
            Act::ToggleZapret => {
                if self.z_run.is_off() {
                    self.start_zapret();
                } else {
                    self.stop_zapret();
                }
            }
            Act::ToggleTg => {
                if self.t_run.is_off() {
                    self.start_tg(ctx);
                } else {
                    self.stop_tg();
                }
            }
            Act::ToggleAll => {
                if !self.z_run.is_off() && !self.t_run.is_off() {
                    self.stop_zapret();
                    self.stop_tg();
                } else {
                    if self.z_run.is_off() {
                        self.start_zapret();
                    }
                    if self.t_run.is_off() {
                        self.start_tg(ctx);
                    }
                }
            }
            Act::SetMode(m) => {
                if self.s.mode != m {
                    self.s.mode = m;
                    self.s.save();
                    self.restart_zapret_if_running();
                }
            }
            Act::SetStrategy(name) => {
                if self.s.strategy != name {
                    self.s.strategy = name;
                    self.s.save();
                    self.restart_zapret_if_running();
                }
            }
            Act::UpdateEngines => {
                self.stop_zapret();
                self.stop_tg();
                sys::stop_windivert();
                self.spawn_install(ctx, true);
            }
            Act::Retry => self.spawn_install(ctx, false),
            Act::CopyLink => {
                ctx.copy_text(tgproxy::link(&self.s.secret));
                self.toast("Ссылка скопирована");
            }
            Act::OpenTelegram => sys::open_url(&tgproxy::link(&self.s.secret)),
            Act::OpenLogs => sys::open_folder(&engines::data_dir()),
        }
    }

    fn restart_zapret_if_running(&mut self) {
        if !self.z_run.is_off() {
            self.stop_zapret();
            self.start_zapret();
        }
    }

    // ------------------------------------------------------------ рисование

    fn view(&self, ui: &mut Ui, act: &mut Option<Act>) {
        // Заголовок: как Header в TVCleaner.
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new("Unblock").size(28.0).strong());
                ui.label(ui::muted(
                    "Обход блокировок для YouTube, Discord и Telegram",
                ));
            });
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                let color = if self.hint { ui::ACCENT } else { ui::MUTED };
                let b = egui::Button::new(RichText::new("?").size(15.0).strong().color(color))
                    .fill(ui::SURFACE)
                    .rounding(50.0)
                    .min_size(Vec2::splat(32.0));
                if ui.add(b).clicked() {
                    *act = Some(Act::ToggleHint);
                }
            });
        });

        if self.hint {
            egui::Frame::none()
                .fill(ui::tint(ui::ACCENT, 26))
                .rounding(16.0)
                .inner_margin(egui::Margin::same(14.0))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        RichText::new(
                            "«Запрет» помогает открывать YouTube и Discord. Три точки рядом с ним: выбор сервисов и стратегии. \
                             «TG-прокси» ускоряет Telegram Desktop: включи его и нажми «Открыть в Telegram» один раз. \
                             Если что-то не работает, попробуй другую стратегию. Нужны права администратора.",
                        )
                        .size(13.0),
                    );
                });
        }

        if !self.admin {
            ui::card(ui, |ui| {
                ui::tag(ui, "ПРАВА АДМИНИСТРАТОРА", ui::WARN);
                ui.label(ui::muted(
                    "Запусти Unblock от имени администратора: без этого запрет не заработает.",
                ));
            });
        }

        match &self.engines {
            Engines::Installing { label, done, total } => {
                ui::card(ui, |ui| {
                    ui.label(RichText::new("Скачиваю движки Flowseal").strong());
                    let mb = |b: u64| b as f64 / 1_048_576.0;
                    let line = if *total > 0 {
                        format!("{label}: {:.1} из {:.1} МБ", mb(*done), mb(*total))
                    } else if *done > 0 {
                        format!("{label}: {:.1} МБ", mb(*done))
                    } else {
                        format!("{label}…")
                    };
                    ui.label(ui::muted(&line));
                    let frac = if *total > 0 {
                        *done as f32 / *total as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .fill(ui::ACCENT)
                            .desired_height(8.0)
                            .animate(*total == 0),
                    );
                });
            }
            Engines::Failed(e) => {
                ui::card(ui, |ui| {
                    ui::tag(ui, "НЕ УДАЛОСЬ СКАЧАТЬ", ui::JUNK);
                    ui.label(ui::muted(e));
                    if ui::ghost(ui, "Повторить", true, 40.0) {
                        *act = Some(Act::Retry);
                    }
                });
            }
            Engines::Ready => {}
        }

        let ready = matches!(self.engines, Engines::Ready);

        // Две колонки.
        ui.columns(2, |cols| {
            self.zapret_card(&mut cols[0], act, ready);
            self.tg_card(&mut cols[1], act, ready);
        });

        // Кнопка снизу.
        let both_active = !self.z_run.is_off() && !self.t_run.is_off();
        if both_active {
            if ui::ghost(ui, "Выключить обе функции", true, 52.0) {
                *act = Some(Act::ToggleAll);
            }
        } else if ui::primary(ui, "Включить обе функции", ready, 52.0) {
            *act = Some(Act::ToggleAll);
        }

        let dash = |s: &str| {
            if s.is_empty() {
                "—".to_string()
            } else {
                s.to_string()
            }
        };
        ui.label(
            RichText::new(format!(
                "Движки Flowseal · zapret {} · tg-ws-proxy {}",
                dash(&self.s.zapret_tag),
                dash(&self.s.tg_tag)
            ))
            .size(12.0)
            .color(ui::MUTED),
        );
    }

    fn zapret_card(&self, ui: &mut Ui, act: &mut Option<Act>, ready: bool) {
        ui::card(ui, |ui| {
            ui.set_min_height(206.0);
            ui.horizontal(|ui| {
                ui.set_min_height(30.0);
                ui.label(RichText::new("Запрет").size(20.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.dots_menu(ui, act);
                });
            });
            let (text, color) = status_of(self.z_run, ready);
            ui::tag(ui, text, color);
            ui.label(ui::muted(self.s.mode.label()));
            let strategy = self.s.strategy.trim_end_matches(".bat").to_string();
            ui.label(ui::muted(&format!("Стратегия: {strategy}")).size(13.0));
            ui.add_space(8.0);
            if self.z_run.is_off() {
                if ui::primary(ui, "Включить", ready, 46.0) {
                    *act = Some(Act::ToggleZapret);
                }
            } else if ui::ghost(ui, "Выключить", true, 46.0) {
                *act = Some(Act::ToggleZapret);
            }
        });
    }

    fn tg_card(&self, ui: &mut Ui, act: &mut Option<Act>, ready: bool) {
        ui::card(ui, |ui| {
            ui.set_min_height(206.0);
            // Строка заголовка той же высоты, что и в левой карточке (там кнопка меню).
            ui.horizontal(|ui| {
                ui.set_min_height(30.0);
                ui.label(RichText::new("TG-прокси").size(20.0).strong());
            });
            let (text, color) = status_of(self.t_run, ready);
            ui::tag(ui, text, color);
            ui.label(ui::muted("Для Telegram Desktop"));
            ui.label(ui::muted(&format!("Адрес {}:{}", tgproxy::HOST, tgproxy::PORT)).size(13.0));
            ui.add_space(8.0);
            if self.t_run.is_off() {
                if ui::primary(ui, "Включить", ready, 46.0) {
                    *act = Some(Act::ToggleTg);
                }
            } else if ui::ghost(ui, "Выключить", true, 46.0) {
                *act = Some(Act::ToggleTg);
            }
            let on = matches!(self.t_run, Run::On);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                if ui::ghost_w(ui, "Открыть в Telegram", on, w, 38.0) {
                    *act = Some(Act::OpenTelegram);
                }
                if ui::ghost_w(ui, "Скопировать ссылку", on, w, 38.0) {
                    *act = Some(Act::CopyLink);
                }
            });
        });
    }

    /// Меню ••• рядом с «Запретом»: сервисы, стратегия, обновление.
    fn dots_menu(&self, ui: &mut Ui, act: &mut Option<Act>) {
        ui.scope(|ui| {
            {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.inactive.bg_fill = Color32::TRANSPARENT;
            }
            ui.menu_button(
                RichText::new("•••").size(16.0).color(ui::MUTED),
                |ui| {
                    ui.set_min_width(230.0);
                    ui.label(ui::muted("Сервисы").size(12.0));
                    for m in [Mode::Both, Mode::Youtube, Mode::Discord] {
                        if ui.selectable_label(self.s.mode == m, m.label()).clicked() {
                            *act = Some(Act::SetMode(m));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    ui.menu_button("Стратегия", |ui| {
                        ui.set_min_width(200.0);
                        if self.strategies.is_empty() {
                            ui.label(ui::muted("Список появится после загрузки"));
                        }
                        for st in &self.strategies {
                            let title = st.trim_end_matches(".bat");
                            if ui.selectable_label(self.s.strategy == *st, title).clicked() {
                                *act = Some(Act::SetStrategy(st.clone()));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Обновить движки").clicked() {
                        *act = Some(Act::UpdateEngines);
                        ui.close_menu();
                    }
                    if ui.button("Открыть папку с логами").clicked() {
                        *act = Some(Act::OpenLogs);
                        ui.close_menu();
                    }
                },
            );
        });
    }
}

fn status_of(run: Run, ready: bool) -> (&'static str, Color32) {
    match run {
        Run::On => ("РАБОТАЕТ", ui::ACCENT),
        Run::Starting(_) => ("ЗАПУСК…", ui::WARN),
        Run::Off => {
            if ready {
                ("ВЫКЛЮЧЕНО", ui::MUTED)
            } else {
                ("ЖДЁТ ДВИЖОК", ui::WARN)
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            self.shutdown();
        }
        self.poll();

        let mut act: Option<Act> = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(ui::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 20.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                self.view(ui, &mut act);
            });

        if let Some((text, _)) = &self.toast {
            egui::Area::new(egui::Id::new("toast"))
                .order(egui::Order::Foreground)
                .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -16.0))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(ui::SURFACE2)
                        .stroke(egui::Stroke::new(1.0, ui::LINE))
                        .rounding(18.0)
                        .inner_margin(egui::Margin::symmetric(18.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_max_width(540.0);
                            ui.label(RichText::new(text.as_str()).color(ui::TEXT));
                        });
                });
        }

        if let Some(a) = act {
            self.handle(a, ctx);
            ctx.request_repaint();
        }

        // Пока что-то запускается или работает, изредка просыпаемся и проверяем процессы.
        let busy = matches!(self.engines, Engines::Installing { .. })
            || matches!(self.z_run, Run::Starting(_))
            || matches!(self.t_run, Run::Starting(_));
        if busy {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if self.z.is_some() || self.t.is_some() || self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.shutdown();
    }
}
