//! Главное окно Unblock: слева колонка с разделами, справа страница выбранного раздела.

use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Align2, Color32, Id, Layout, RichText, Ui, Vec2};

use crate::settings::{Mode, Settings};
use crate::ui::{self, Colors};
use crate::{engines, sys, tgproxy, zapret};

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

    /// Для кнопки питания: 0 выключено, 1 запускается, 2 работает.
    fn code(self) -> u8 {
        match self {
            Run::Off => 0,
            Run::Starting(_) => 1,
            Run::On => 2,
        }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Zapret,
    Tg,
    Settings,
}

/// Действия пользователя. Окно только собирает их, а выполняются они после отрисовки.
enum Act {
    SetPage(Page),
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
    SetAutostart(bool),
    SetAutoEnable(bool),
    SetTheme(bool),
    SetAccent(usize),
}

pub struct App {
    s: Settings,
    c: Colors,
    page: Page,
    z: Option<Child>,
    z_run: Run,
    t: Option<Child>,
    t_run: Run,
    engines: Engines,
    admin: bool,
    autostart: bool,
    pending_auto: bool,
    /// Запуск из автозапуска Windows: в первом кадре сворачиваем окно.
    start_minimized: bool,
    strategies: Vec<String>,
    toast: Option<(String, Instant)>,
    down: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut s = Settings::load();
        if !sys::valid_secret(&s.secret) {
            s.secret = sys::random_hex16();
            s.save();
        }
        let c = Colors::new(s.theme_dark, s.accent);
        ui::install_fonts(&cc.egui_ctx);
        ui::apply(&cc.egui_ctx, &c);

        let (tx, rx) = channel();
        let pending_auto = s.auto_enable;
        let mut app = App {
            s,
            c,
            page: Page::Zapret,
            z: None,
            z_run: Run::Off,
            t: None,
            t_run: Run::Off,
            engines: Engines::Ready,
            admin: sys::is_admin(),
            autostart: sys::autostart_enabled(),
            pending_auto,
            start_minimized: std::env::args().any(|a| a == "--autostart"),
            strategies: Vec::new(),
            toast: None,
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
                    "Запрет остановился ({status}). Подробности в файле zapret.log: Настройки → «Открыть папку с логами»."
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
            Act::SetPage(p) => self.page = p,
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
            Act::SetAutostart(on) => match sys::set_autostart(on) {
                Ok(()) => self.autostart = on,
                Err(e) => self.toast(e),
            },
            Act::SetAutoEnable(on) => {
                self.s.auto_enable = on;
                self.s.save();
            }
            Act::SetTheme(dark) => {
                self.s.theme_dark = dark;
                self.s.save();
                self.c = Colors::new(self.s.theme_dark, self.s.accent);
                ui::apply(ctx, &self.c);
            }
            Act::SetAccent(i) => {
                self.s.accent = i;
                self.s.save();
                self.c = Colors::new(self.s.theme_dark, self.s.accent);
                ui::apply(ctx, &self.c);
            }
        }
    }

    fn restart_zapret_if_running(&mut self) {
        if !self.z_run.is_off() {
            self.stop_zapret();
            self.start_zapret();
        }
    }

    // ------------------------------------------------------------ боковая колонка

    fn sidebar(&self, ui: &mut Ui, act: &mut Option<Act>) {
        let c = &self.c;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
            ui::brand_mark(ui.painter(), rect, c);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                ui.label(ui::bold("Unblock", 20.0, c.text));
                ui.label(RichText::new("обход блокировок").size(12.0).color(c.muted));
            });
        });
        ui.add_space(18.0);

        ui.spacing_mut().item_spacing.y = 6.0;
        if ui::nav_item(
            ui,
            c,
            Id::new("nav_zapret"),
            ui::icon_shield,
            "Запрет",
            self.page == Page::Zapret,
        ) {
            *act = Some(Act::SetPage(Page::Zapret));
        }
        if ui::nav_item(
            ui,
            c,
            Id::new("nav_tg"),
            ui::icon_plane,
            "TG-прокси",
            self.page == Page::Tg,
        ) {
            *act = Some(Act::SetPage(Page::Tg));
        }
        if ui::nav_item(
            ui,
            c,
            Id::new("nav_settings"),
            ui::icon_gear,
            "Настройки",
            self.page == Page::Settings,
        ) {
            *act = Some(Act::SetPage(Page::Settings));
        }

        // Внизу: общая кнопка и версия.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.label(
                RichText::new(format!("версия {}", env!("CARGO_PKG_VERSION")))
                    .size(12.0)
                    .color(c.muted),
            );
            let ready = matches!(self.engines, Engines::Ready);
            let both_active = !self.z_run.is_off() && !self.t_run.is_off();
            let w = ui.available_width();
            if both_active {
                if ui::button(ui, c, "Выключить всё", false, true, w, 46.0) {
                    *act = Some(Act::ToggleAll);
                }
            } else if ui::button(ui, c, "Включить всё", true, ready, w, 46.0) {
                *act = Some(Act::ToggleAll);
            }
        });
    }

    // ------------------------------------------------------------ страницы

    fn page(&self, ui: &mut Ui, act: &mut Option<Act>) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                self.banners(ui, act);
                match self.page {
                    Page::Zapret => self.page_zapret(ui, act),
                    Page::Tg => self.page_tg(ui, act),
                    Page::Settings => self.page_settings(ui, act),
                }
            });
    }

    /// Предупреждения сверху: права администратора и загрузка движков.
    fn banners(&self, ui: &mut Ui, act: &mut Option<Act>) {
        let c = &self.c;
        if !self.admin {
            ui::card(ui, c, |ui| {
                ui::pill(ui, c, "ПРАВА АДМИНИСТРАТОРА", c.warn);
                ui.label(ui::muted(
                    c,
                    "Запусти Unblock от имени администратора: без этого запрет не заработает.",
                ));
            });
        }
        match &self.engines {
            Engines::Installing { label, done, total } => {
                ui::card(ui, c, |ui| {
                    ui.label(ui::bold("Скачиваю движки Flowseal", 16.0, c.text));
                    let mb = |b: u64| b as f64 / 1_048_576.0;
                    let line = if *total > 0 {
                        format!("{label}: {:.1} из {:.1} МБ", mb(*done), mb(*total))
                    } else if *done > 0 {
                        format!("{label}: {:.1} МБ", mb(*done))
                    } else {
                        format!("{label}…")
                    };
                    ui.label(ui::muted(c, &line));
                    let frac = if *total > 0 {
                        *done as f32 / *total as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .fill(c.accent)
                            .desired_height(8.0)
                            .animate(*total == 0),
                    );
                });
            }
            Engines::Failed(e) => {
                ui::card(ui, c, |ui| {
                    ui::pill(ui, c, "НЕ УДАЛОСЬ СКАЧАТЬ", c.junk);
                    ui.label(ui::muted(c, e));
                    let w = ui.available_width();
                    if ui::button(ui, c, "Повторить", false, true, w, 42.0) {
                        *act = Some(Act::Retry);
                    }
                });
            }
            Engines::Ready => {}
        }
    }

    fn status_of(&self, run: Run, ready: bool) -> (&'static str, Color32) {
        let c = &self.c;
        match run {
            Run::On => ("Работает", c.accent_fg),
            Run::Starting(_) => ("Запускается…", c.warn),
            Run::Off => {
                if ready {
                    ("Выключено", c.muted)
                } else {
                    ("Ждёт движок", c.warn)
                }
            }
        }
    }

    fn page_zapret(&self, ui: &mut Ui, act: &mut Option<Act>) {
        let c = &self.c;
        let ready = matches!(self.engines, Engines::Ready);
        ui::header(ui, c, "Запрет", "Открывает YouTube и Discord");

        ui::card(ui, c, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                if ui::power_button(ui, c, Id::new("power_zapret"), self.z_run.code(), ready) {
                    *act = Some(Act::ToggleZapret);
                }
                let (text, color) = self.status_of(self.z_run, ready);
                ui.label(ui::bold(text, 21.0, color));
                ui.label(ui::muted(c, self.s.mode.label()));
                ui.add_space(6.0);
            });
        });

        ui::card(ui, c, |ui| {
            ui.label(ui::bold("Сервисы", 16.0, c.text));
            ui.label(
                RichText::new("Что нужно разблокировать")
                    .size(13.0)
                    .color(c.muted),
            );
            ui.add_space(4.0);
            let w = ui.available_width();
            if let Some(i) = ui::segmented(
                ui,
                c,
                Id::new("seg_mode"),
                &["YouTube и Discord", "Только YouTube", "Только Discord"],
                self.s.mode.index(),
                w,
            ) {
                *act = Some(Act::SetMode(Mode::from_index(i)));
            }
        });

        ui::card(ui, c, |ui| {
            ui::row(
                ui,
                c,
                "Стратегия",
                "Если что-то не открывается, попробуй другую",
                |ui| {
                    let current = self.s.strategy.trim_end_matches(".bat").to_string();
                    ui.menu_button(ui::bold(&current, 14.0, c.accent_fg), |ui| {
                        ui.set_min_width(200.0);
                        if self.strategies.is_empty() {
                            ui.label(ui::muted(c, "Список появится после загрузки"));
                        }
                        for st in &self.strategies {
                            let title = st.trim_end_matches(".bat");
                            if ui.selectable_label(self.s.strategy == *st, title).clicked() {
                                *act = Some(Act::SetStrategy(st.clone()));
                                ui.close_menu();
                            }
                        }
                    });
                },
            );
        });
    }

    fn page_tg(&self, ui: &mut Ui, act: &mut Option<Act>) {
        let c = &self.c;
        let ready = matches!(self.engines, Engines::Ready);
        ui::header(ui, c, "TG-прокси", "Для Telegram Desktop");

        ui::card(ui, c, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                if ui::power_button(ui, c, Id::new("power_tg"), self.t_run.code(), ready) {
                    *act = Some(Act::ToggleTg);
                }
                let (text, color) = self.status_of(self.t_run, ready);
                ui.label(ui::bold(text, 21.0, color));
                ui.label(ui::muted(
                    c,
                    &format!("{}:{}", tgproxy::HOST, tgproxy::PORT),
                ));
                ui.add_space(6.0);
            });
        });

        ui::card(ui, c, |ui| {
            ui.label(ui::bold("Подключение", 16.0, c.text));
            ui.label(
                RichText::new(
                    "Когда прокси работает, нажми «Открыть в Telegram»: приложение предложит добавить прокси.",
                )
                .size(13.0)
                .color(c.muted),
            );
            ui.add_space(4.0);
            let on = matches!(self.t_run, Run::On);
            let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            ui.horizontal(|ui| {
                if ui::button(ui, c, "Открыть в Telegram", true, on, w, 44.0) {
                    *act = Some(Act::OpenTelegram);
                }
                if ui::button(ui, c, "Скопировать ссылку", false, on, w, 44.0) {
                    *act = Some(Act::CopyLink);
                }
            });
        });
    }

    fn page_settings(&self, ui: &mut Ui, act: &mut Option<Act>) {
        let c = &self.c;
        ui::header(ui, c, "Настройки", "Запуск, оформление и движки");

        ui::card(ui, c, |ui| {
            ui.label(ui::bold("Запуск", 16.0, c.text));
            ui::row(
                ui,
                c,
                "Запускать вместе с Windows",
                "Unblock откроется свёрнутым при входе в систему",
                |ui| {
                    if ui::switch(ui, c, Id::new("sw_autostart"), self.autostart) {
                        *act = Some(Act::SetAutostart(!self.autostart));
                    }
                },
            );
            ui::divider(ui, c);
            ui::row(
                ui,
                c,
                "Включать при запуске",
                "Запрет и TG-прокси включатся сами",
                |ui| {
                    if ui::switch(ui, c, Id::new("sw_auto_enable"), self.s.auto_enable) {
                        *act = Some(Act::SetAutoEnable(!self.s.auto_enable));
                    }
                },
            );
        });

        ui::card(ui, c, |ui| {
            ui.label(ui::bold("Оформление", 16.0, c.text));
            ui::row(
                ui,
                c,
                "Тема",
                "Тёмная или светлая",
                |ui| {
                    if let Some(i) = ui::segmented(
                        ui,
                        c,
                        Id::new("seg_theme"),
                        &["Тёмная", "Светлая"],
                        if self.s.theme_dark { 0 } else { 1 },
                        220.0,
                    ) {
                        *act = Some(Act::SetTheme(i == 0));
                    }
                },
            );
            ui::divider(ui, c);
            ui::row(
                ui,
                c,
                "Цвет акцента",
                ui::ACCENTS[self.s.accent.min(4)].0,
                |ui| {
                    if let Some(i) = ui::swatches(ui, c, Id::new("accent"), self.s.accent) {
                        *act = Some(Act::SetAccent(i));
                    }
                },
            );
        });

        ui::card(ui, c, |ui| {
            ui.label(ui::bold("Движки Flowseal", 16.0, c.text));
            let dash = |s: &str| {
                if s.is_empty() {
                    "не скачан".to_string()
                } else {
                    s.to_string()
                }
            };
            ui.label(ui::muted(
                c,
                &format!(
                    "zapret: {}   ·   tg-ws-proxy: {}",
                    dash(&self.s.zapret_tag),
                    dash(&self.s.tg_tag)
                ),
            ));
            ui.add_space(4.0);
            let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            ui.horizontal(|ui| {
                if ui::button(ui, c, "Обновить движки", false, true, w, 44.0) {
                    *act = Some(Act::UpdateEngines);
                }
                if ui::button(ui, c, "Открыть папку с логами", false, true, w, 44.0)
                {
                    *act = Some(Act::OpenLogs);
                }
            });
        });

        ui.label(
            RichText::new(
                "Unblock управляет проектами Flowseal: zapret-discord-youtube и tg-ws-proxy. В основе запрета лежит zapret от bol-van.",
            )
            .size(12.0)
            .color(c.muted),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            self.shutdown();
        }
        if self.start_minimized {
            self.start_minimized = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        self.poll();

        // «Включать при запуске»: ждём, пока движки готовы, и включаем один раз.
        if self.pending_auto && matches!(self.engines, Engines::Ready) {
            self.pending_auto = false;
            if self.z_run.is_off() {
                self.start_zapret();
            }
            if self.t_run.is_off() {
                self.start_tg(ctx);
            }
        }

        let mut act: Option<Act> = None;
        let c = self.c;

        egui::SidePanel::left("sidebar")
            .exact_width(236.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(c.surface)
                    .stroke(egui::Stroke::new(1.0, c.line))
                    .inner_margin(egui::Margin::symmetric(16.0, 18.0)),
            )
            .show(ctx, |ui| {
                self.sidebar(ui, &mut act);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(c.bg)
                    .inner_margin(egui::Margin::symmetric(28.0, 24.0)),
            )
            .show(ctx, |ui| {
                self.page(ui, &mut act);
            });

        if let Some((text, _)) = &self.toast {
            egui::Area::new(Id::new("toast"))
                .order(egui::Order::Foreground)
                .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-20.0, -20.0))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(c.surface2)
                        .stroke(egui::Stroke::new(1.0, c.line))
                        .rounding(18.0)
                        .inner_margin(egui::Margin::symmetric(18.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.label(RichText::new(text.as_str()).color(c.text));
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
