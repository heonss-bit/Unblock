//! Оформление в стиле TVCleaner: тёмный минимализм и один мятный акцент.
//! Цвета, скругления и отступы перенесены из его Ui-объекта и компонентов.

use eframe::egui::{self, Color32, Frame, Margin, RichText, Rounding, Stroke, Ui, Vec2};

pub const BG: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);
pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x18, 0x1D);
pub const SURFACE2: Color32 = Color32::from_rgb(0x1B, 0x21, 0x28);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x2D, 0x36);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF5, 0xF8);
pub const MUTED: Color32 = Color32::from_rgb(0x8B, 0x96, 0xA3);
pub const ACCENT: Color32 = Color32::from_rgb(0x6E, 0xE7, 0xB7);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x04, 0x28, 0x1B);
pub const JUNK: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x7A);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xC8, 0x57);

/// Цвет с заданной прозрачностью (0..255).
pub fn tint(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE2;
    v.extreme_bg_color = BG;
    v.faint_bg_color = SURFACE;
    v.window_rounding = Rounding::same(16.0);
    v.menu_rounding = Rounding::same(14.0);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.selection.bg_fill = tint(ACCENT, 60);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(12.0);
        w.fg_stroke = Stroke::new(1.0, TEXT);
    }
    v.widgets.inactive.weak_bg_fill = SURFACE2;
    v.widgets.inactive.bg_fill = SURFACE2;
    v.widgets.hovered.weak_bg_fill = LINE;
    v.widgets.hovered.bg_fill = LINE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.active.weak_bg_fill = LINE;
    v.widgets.active.bg_fill = LINE;
    v.widgets.open.weak_bg_fill = SURFACE2;
    v.widgets.open.bg_fill = SURFACE2;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.spacing.button_padding = Vec2::new(16.0, 10.0);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
    });
}

/// Приглушённый текст (серый).
pub fn muted(text: &str) -> RichText {
    RichText::new(text).size(14.0).color(MUTED)
}

/// Карточка: поверхность, скругление 20, внутренний отступ 18.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(SURFACE)
        .rounding(20.0)
        .inner_margin(Margin::same(18.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Плашка-«таблетка»: цветной текст на полупрозрачной подложке того же цвета.
pub fn tag(ui: &mut Ui, text: &str, color: Color32) {
    Frame::none()
        .fill(tint(color, 36))
        .rounding(50.0)
        .inner_margin(Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).strong().color(color));
        });
}

/// Основная кнопка: мятная заливка, тёмный текст.
pub fn primary(ui: &mut Ui, text: &str, enabled: bool, height: f32) -> bool {
    let (fill, fg) = if enabled {
        (ACCENT, ON_ACCENT)
    } else {
        (SURFACE2, MUTED)
    };
    let b = egui::Button::new(RichText::new(text).size(16.0).strong().color(fg))
        .fill(fill)
        .rounding(16.0)
        .min_size(Vec2::new(ui.available_width(), height));
    ui.add_enabled(enabled, b).clicked()
}

/// Второстепенная кнопка: контур и прозрачный фон, на всю ширину.
pub fn ghost(ui: &mut Ui, text: &str, enabled: bool, height: f32) -> bool {
    let w = ui.available_width();
    ghost_w(ui, text, enabled, w, height)
}

pub fn ghost_w(ui: &mut Ui, text: &str, enabled: bool, width: f32, height: f32) -> bool {
    let fg = if enabled { TEXT } else { MUTED };
    let b = egui::Button::new(RichText::new(text).size(15.0).color(fg))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, LINE))
        .rounding(16.0)
        .min_size(Vec2::new(width, height));
    ui.add_enabled(enabled, b).clicked()
}
