//! Оформление в стиле TVCleaner: спокойная тёмная (или светлая) палитра, один акцент,
//! плавные переходы. Все элементы нарисованы вручную, чтобы выглядеть мягко и единообразно.

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Id, Layout, Margin, Painter, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};

/// Цвета акцента на выбор: название и RGB.
pub const ACCENTS: [(&str, [u8; 3]); 5] = [
    ("Мятный", [0x6E, 0xE7, 0xB7]),
    ("Небесный", [0x7D, 0xB8, 0xFF]),
    ("Сиреневый", [0xB7, 0x9C, 0xFF]),
    ("Янтарный", [0xFF, 0xB8, 0x6B]),
    ("Розовый", [0xFF, 0x8F, 0xB1]),
];

#[derive(Clone, Copy)]
pub struct Colors {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    /// Акцент для текста и значков: на светлой теме темнее, чтобы читался.
    pub accent_fg: Color32,
    pub on_accent: Color32,
    pub warn: Color32,
    pub junk: Color32,
}

impl Colors {
    pub fn new(dark: bool, accent_idx: usize) -> Colors {
        let [r, g, b] = ACCENTS[accent_idx.min(ACCENTS.len() - 1)].1;
        let accent = Color32::from_rgb(r, g, b);
        if dark {
            Colors {
                dark,
                bg: Color32::from_rgb(0x0B, 0x0D, 0x10),
                surface: Color32::from_rgb(0x14, 0x18, 0x1D),
                surface2: Color32::from_rgb(0x1B, 0x21, 0x28),
                line: Color32::from_rgb(0x26, 0x2D, 0x36),
                text: Color32::from_rgb(0xF2, 0xF5, 0xF8),
                muted: Color32::from_rgb(0x8B, 0x96, 0xA3),
                accent,
                accent_fg: accent,
                on_accent: Color32::from_rgb(0x08, 0x0F, 0x0D),
                warn: Color32::from_rgb(0xFF, 0xC8, 0x57),
                junk: Color32::from_rgb(0xFF, 0x7A, 0x7A),
            }
        } else {
            Colors {
                dark,
                bg: Color32::from_rgb(0xF3, 0xF5, 0xF8),
                surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                surface2: Color32::from_rgb(0xEC, 0xF0, 0xF4),
                line: Color32::from_rgb(0xE0, 0xE5, 0xEB),
                text: Color32::from_rgb(0x16, 0x1B, 0x22),
                muted: Color32::from_rgb(0x6A, 0x75, 0x82),
                accent,
                accent_fg: accent.lerp_to_gamma(Color32::BLACK, 0.45),
                on_accent: Color32::from_rgb(0x08, 0x0F, 0x0D),
                warn: Color32::from_rgb(0xC9, 0x80, 0x00),
                junk: Color32::from_rgb(0xD9, 0x45, 0x45),
            }
        }
    }
}

/// Цвет с заданной прозрачностью (0..255).
pub fn tint(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

fn alpha_f(k: f32, max: f32) -> u8 {
    (k.clamp(0.0, 1.0) * max) as u8
}

// ------------------------------------------------------------------ шрифты и стиль

/// Берём Segoe UI из самой Windows (он есть везде и выглядит родным).
/// Если файлов нет, остаются шрифты egui по умолчанию.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let dir = std::path::PathBuf::from(windir).join("Fonts");
    let load = |names: &[&str]| names.iter().find_map(|n| std::fs::read(dir.join(n)).ok());

    let mut prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = load(&["segoeui.ttf"]) {
        fonts
            .font_data
            .insert("unblock-regular".to_owned(), FontData::from_owned(bytes));
        prop.insert(0, "unblock-regular".to_owned());
        fonts
            .families
            .insert(FontFamily::Proportional, prop.clone());
    }
    let mut bold_chain = prop;
    if let Some(bytes) = load(&["seguisb.ttf", "segoeuib.ttf"]) {
        fonts
            .font_data
            .insert("unblock-bold".to_owned(), FontData::from_owned(bytes));
        bold_chain.insert(0, "unblock-bold".to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), bold_chain);
    ctx.set_fonts(fonts);
}

fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn apply(ctx: &egui::Context, c: &Colors) {
    let mut v = if c.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = c.bg;
    v.window_fill = c.surface;
    v.extreme_bg_color = c.bg;
    v.faint_bg_color = c.surface;
    v.window_rounding = egui::Rounding::same(16.0);
    v.menu_rounding = egui::Rounding::same(14.0);
    v.window_stroke = Stroke::new(1.0, c.line);
    v.selection.bg_fill = tint(c.accent, 60);
    v.selection.stroke = Stroke::new(1.0, c.accent_fg);

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = egui::Rounding::same(12.0);
        w.fg_stroke = Stroke::new(1.0, c.text);
    }
    v.widgets.inactive.weak_bg_fill = c.surface2;
    v.widgets.inactive.bg_fill = c.surface2;
    v.widgets.hovered.weak_bg_fill = c.line;
    v.widgets.hovered.bg_fill = c.line;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.line);
    v.widgets.active.weak_bg_fill = c.line;
    v.widgets.active.bg_fill = c.line;
    v.widgets.open.weak_bg_fill = c.surface2;
    v.widgets.open.bg_fill = c.surface2;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.spacing.button_padding = Vec2::new(14.0, 8.0);
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
    });
}

// ------------------------------------------------------------------ текст и контейнеры

pub fn bold(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text)
        .size(size)
        .color(color)
        .family(FontFamily::Name("bold".into()))
}

pub fn muted(c: &Colors, text: &str) -> RichText {
    RichText::new(text).size(14.0).color(c.muted)
}

/// Карточка: поверхность, тонкий контур, скругление 22.
pub fn card<R>(ui: &mut Ui, c: &Colors, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(c.surface)
        .stroke(Stroke::new(1.0, c.line))
        .rounding(22.0)
        .inner_margin(Margin::same(20.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Заголовок страницы.
pub fn header(ui: &mut Ui, c: &Colors, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(bold(title, 28.0, c.text));
        ui.label(muted(c, subtitle));
    });
}

/// Плашка-«таблетка»: цветной текст на мягкой подложке того же цвета.
pub fn pill(ui: &mut Ui, _c: &Colors, text: &str, color: Color32) {
    Frame::none()
        .fill(tint(color, 34))
        .rounding(50.0)
        .inner_margin(Margin::symmetric(11.0, 4.0))
        .show(ui, |ui| {
            ui.label(bold(text, 12.0, color));
        });
}

/// Тонкая разделительная линия.
pub fn divider(ui: &mut Ui, c: &Colors) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, c.line);
}

/// Строка настройки: слева название и пояснение, справа произвольный элемент.
pub fn row(ui: &mut Ui, c: &Colors, title: &str, subtitle: &str, add_right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(50.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(bold(title, 15.0, c.text));
            ui.label(RichText::new(subtitle).size(13.0).color(c.muted));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| add_right(ui));
    });
}

// ------------------------------------------------------------------ кнопки и переключатели

/// Кнопка с плавным наведением. `primary`: акцентная заливка, иначе контурная.
pub fn button(
    ui: &mut Ui,
    c: &Colors,
    text: &str,
    primary: bool,
    enabled: bool,
    width: f32,
    height: f32,
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), sense);
    let id = ui.id().with(("button", text));
    let ctx = ui.ctx().clone();
    let h = ctx.animate_bool_with_time(id, resp.hovered() && enabled, 0.15);

    let (fill, stroke, fg) = if primary {
        if enabled {
            (
                c.accent.lerp_to_gamma(Color32::WHITE, 0.14 * h),
                Stroke::NONE,
                c.on_accent,
            )
        } else {
            (c.surface2, Stroke::NONE, c.muted)
        }
    } else {
        (
            tint(c.text, alpha_f(h, 16.0)),
            Stroke::new(1.0, c.line.lerp_to_gamma(c.muted, 0.6 * h)),
            if enabled { c.text } else { c.muted },
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, 14.0, fill);
    if stroke != Stroke::NONE {
        p.rect_stroke(rect, 14.0, stroke);
    }
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        bold_font(15.0),
        fg,
    );
    if enabled && resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

/// Переключатель с плавным движком. Возвращает true в момент нажатия.
pub fn switch(ui: &mut Ui, c: &Colors, id: Id, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(48.0, 28.0), Sense::click());
    let ctx = ui.ctx().clone();
    let t = ctx.animate_bool_with_time(id, on, 0.16);
    let p = ui.painter();
    p.rect_filled(rect, 14.0, c.surface2.lerp_to_gamma(c.accent, t));
    p.rect_stroke(
        rect,
        14.0,
        Stroke::new(1.0, c.line.lerp_to_gamma(c.accent, t)),
    );
    let x = rect.left() + 14.0 + t * (rect.width() - 28.0);
    let knob = c.muted.lerp_to_gamma(c.on_accent, t);
    p.circle_filled(Pos2::new(x, rect.center().y), 10.0, knob);
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Сегментированный выбор: ползунок плавно переезжает к выбранному варианту.
pub fn segmented(
    ui: &mut Ui,
    c: &Colors,
    id: Id,
    labels: &[&str],
    selected: usize,
    width: f32,
) -> Option<usize> {
    let h = 42.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    p.rect_filled(rect, 15.0, c.surface2);

    let n = labels.len().max(1);
    let seg_w = (rect.width() - 8.0) / n as f32;
    let pos = ctx.animate_value_with_time(id.with("pos"), selected as f32, 0.2);
    let hl = Rect::from_min_size(
        Pos2::new(rect.left() + 4.0 + pos * seg_w, rect.top() + 4.0),
        Vec2::new(seg_w, h - 8.0),
    );
    p.rect_filled(hl, 11.0, tint(c.accent, 38));
    p.rect_stroke(hl, 11.0, Stroke::new(1.0, tint(c.accent, 110)));

    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + 4.0 + i as f32 * seg_w, rect.top() + 4.0),
            Vec2::new(seg_w, h - 8.0),
        );
        let resp = ui.interact(r, id.with(i), Sense::click());
        let sel_t = (1.0 - (pos - i as f32).abs()).clamp(0.0, 1.0);
        let col = c.muted.lerp_to_gamma(c.accent_fg, sel_t);
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            bold_font(14.0),
            col,
        );
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Ряд цветных кружков для выбора акцента.
pub fn swatches(ui: &mut Ui, c: &Colors, id: Id, selected: usize) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (i, (_, [r, g, b])) in ACCENTS.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
            let t = ctx.animate_bool_with_time(id.with(i), i == selected, 0.15);
            let p = ui.painter();
            p.circle_filled(rect.center(), 10.0, Color32::from_rgb(*r, *g, *b));
            p.circle_stroke(
                rect.center(),
                14.0,
                Stroke::new(2.0, tint(c.text, alpha_f(t, 230.0))),
            );
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// Большая круглая кнопка питания со свечением.
/// `run`: 0 выключено, 1 запускается, 2 работает.
pub fn power_button(ui: &mut Ui, c: &Colors, id: Id, run: u8, enabled: bool) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(160.0), sense);
    let ctx = ui.ctx().clone();
    let on_t = ctx.animate_bool_with_time(id.with("on"), run == 2, 0.4);
    let hov_t = ctx.animate_bool_with_time(id.with("hov"), resp.hovered() && enabled, 0.15);
    let time = ctx.input(|i| i.time) as f32;
    let center = rect.center();
    let p = ui.painter();

    // Свечение: при запуске пульсирует жёлтым, при работе ровно горит акцентом.
    let (glow_col, glow_k) = if run == 1 {
        (c.warn, 0.55 + 0.45 * (time * 3.2).sin())
    } else {
        (c.accent, on_t)
    };
    for i in (0..6).rev() {
        let r = 50.0 + i as f32 * 5.5 + hov_t * 2.0;
        let a = alpha_f((1.0 - i as f32 / 6.0) * glow_k, 38.0);
        p.circle_filled(center, r, tint(glow_col, a));
    }

    let body_r = 50.0 + hov_t * 1.5;
    let fill = c.surface2.lerp_to_gamma(c.accent, on_t);
    let edge = c.line.lerp_to_gamma(c.accent, on_t);
    p.circle_filled(center, body_r, fill);
    p.circle_stroke(center, body_r, Stroke::new(1.5, edge));

    let icon_col = if run == 1 {
        c.warn
    } else if enabled {
        c.muted
            .lerp_to_gamma(c.on_accent, on_t)
            .lerp_to_gamma(c.text, hov_t * 0.35 * (1.0 - on_t))
    } else {
        tint(c.muted, 120)
    };
    power_icon(p, center, 18.0, 3.0, icon_col);

    if run == 1 {
        ctx.request_repaint();
    }
    if enabled && resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

/// Пункт боковой колонки: значок и название, плавная подсветка выбранного.
pub fn nav_item(
    ui: &mut Ui,
    c: &Colors,
    id: Id,
    icon: fn(&Painter, Pos2, Color32),
    label: &str,
    selected: bool,
) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let ctx = ui.ctx().clone();
    let sel_t = ctx.animate_bool_with_time(id.with("sel"), selected, 0.18);
    let hov_t = ctx.animate_bool_with_time(id.with("hov"), resp.hovered(), 0.12);
    let p = ui.painter();
    p.rect_filled(
        rect,
        14.0,
        tint(c.text, alpha_f(hov_t * (1.0 - sel_t), 12.0)),
    );
    p.rect_filled(rect, 14.0, tint(c.accent, alpha_f(sel_t, 36.0)));
    icon_color(
        c,
        sel_t,
        hov_t,
        icon,
        p,
        Pos2::new(rect.left() + 26.0, rect.center().y),
    );
    let text_col = c
        .muted
        .lerp_to_gamma(c.text, (sel_t + hov_t * 0.5).min(1.0));
    p.text(
        Pos2::new(rect.left() + 50.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        bold_font(15.0),
        text_col,
    );
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn icon_color(
    c: &Colors,
    sel_t: f32,
    hov_t: f32,
    icon: fn(&Painter, Pos2, Color32),
    p: &Painter,
    at: Pos2,
) {
    let col = c
        .muted
        .lerp_to_gamma(c.accent_fg, (sel_t + hov_t * 0.4).min(1.0));
    icon(p, at, col);
}

// ------------------------------------------------------------------ значки (рисуются линиями)

fn off(c: Pos2, x: f32, y: f32) -> Pos2 {
    Pos2::new(c.x + x, c.y + y)
}

/// Ломаная с круглыми стыками.
fn poly(p: &Painter, pts: &[Pos2], w: f32, col: Color32, closed: bool) {
    let stroke = Stroke::new(w, col);
    for pair in pts.windows(2) {
        p.line_segment([pair[0], pair[1]], stroke);
    }
    if closed && pts.len() > 2 {
        p.line_segment([pts[pts.len() - 1], pts[0]], stroke);
    }
    for q in pts {
        p.circle_filled(*q, w / 2.0, col);
    }
}

fn arc_points(center: Pos2, r: f32, from_deg: f32, to_deg: f32, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / steps as f32).to_radians();
            Pos2::new(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect()
}

pub fn icon_shield(p: &Painter, ctr: Pos2, col: Color32) {
    let pts = [
        off(ctr, -8.0, -7.0),
        off(ctr, 0.0, -10.0),
        off(ctr, 8.0, -7.0),
        off(ctr, 8.0, 1.0),
        off(ctr, 0.0, 10.0),
        off(ctr, -8.0, 1.0),
    ];
    poly(p, &pts, 1.8, col, true);
    let check = [
        off(ctr, -3.5, 0.0),
        off(ctr, -0.5, 3.0),
        off(ctr, 4.0, -3.0),
    ];
    poly(p, &check, 1.8, col, false);
}

pub fn icon_plane(p: &Painter, ctr: Pos2, col: Color32) {
    let pts = [
        off(ctr, -10.0, 0.5),
        off(ctr, 10.0, -8.0),
        off(ctr, 4.5, 9.0),
        off(ctr, 0.0, 2.5),
    ];
    poly(p, &pts, 1.8, col, true);
    poly(
        p,
        &[off(ctr, 0.0, 2.5), off(ctr, 10.0, -8.0)],
        1.8,
        col,
        false,
    );
}

pub fn icon_gear(p: &Painter, ctr: Pos2, col: Color32) {
    p.circle_stroke(ctr, 5.0, Stroke::new(1.8, col));
    for k in 0..8 {
        let a = (k as f32 * 45.0).to_radians();
        let (s, c) = a.sin_cos();
        poly(
            p,
            &[off(ctr, c * 7.5, s * 7.5), off(ctr, c * 10.0, s * 10.0)],
            2.2,
            col,
            false,
        );
    }
}

fn power_icon(p: &Painter, ctr: Pos2, r: f32, w: f32, col: Color32) {
    // Дуга с разрывом сверху и вертикальная черта: значок питания.
    let arc = arc_points(ctr, r, -50.0, 230.0, 30);
    poly(p, &arc, w, col, false);
    poly(
        p,
        &[off(ctr, 0.0, -r - 3.0), off(ctr, 0.0, -2.0)],
        w,
        col,
        false,
    );
}

/// Фирменный знак: мятный квадрат с открытым замком.
pub fn brand_mark(p: &Painter, rect: Rect, c: &Colors) {
    p.rect_filled(rect, 11.0, c.accent);
    let ctr = rect.center();
    p.rect_filled(
        Rect::from_center_size(off(ctr, 0.0, 4.5), Vec2::new(16.0, 12.0)),
        3.0,
        c.on_accent,
    );
    let shackle = arc_points(off(ctr, -1.5, -2.5), 5.0, 180.0, 360.0, 10);
    poly(p, &shackle, 2.4, c.on_accent, false);
    poly(
        p,
        &[off(ctr, -6.5, -2.5), off(ctr, -6.5, 0.5)],
        2.4,
        c.on_accent,
        false,
    );
}
