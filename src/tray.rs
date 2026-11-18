//! Значок в трее. Обработчики не зависят от окна egui: когда окно спрятано,
//! eframe может не перерисовываться, а «Закрыть» и «Отключить» должны работать всегда.

use std::sync::atomic::{AtomicBool, Ordering};

/// Из меню трея нажали «Отключить»: окно при следующем кадре сбросит свои переключатели.
static DISABLE: AtomicBool = AtomicBool::new(false);

pub fn take_disable() -> bool {
    DISABLE.swap(false, Ordering::SeqCst)
}

/// Останавливает обход (zapret, TG-прокси, драйвер). Окно здесь не нужно.
fn kill_everything() {
    crate::sys::kill_image("winws.exe");
    crate::sys::kill_image(crate::engines::TG_EXE);
    crate::sys::stop_windivert();
}

#[cfg(windows)]
mod imp {
    use super::*;
    use eframe::egui::Context;
    use std::cell::RefCell;
    use std::sync::OnceLock;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    static CTX: OnceLock<Context> = OnceLock::new();

    thread_local! {
        // Значок живёт в главном потоке; при выходе его нужно убрать, иначе он «зависнет» в трее.
        static TRAY: RefCell<Option<TrayIcon>> = RefCell::new(None);
    }

    fn wake() {
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
    }

    fn show() {
        crate::sys::show_main_window();
        wake();
    }

    fn quit() {
        kill_everything();
        TRAY.with(|t| {
            t.borrow_mut().take();
        });
        std::process::exit(0);
    }

    pub fn init(ctx: &Context) -> bool {
        let _ = CTX.set(ctx.clone());
        let rgba = include_bytes!("../assets/icon64.rgba").to_vec();
        let Ok(icon) = Icon::from_rgba(rgba, 64, 64) else {
            return false;
        };

        let menu = Menu::new();
        let open = MenuItem::with_id("open", "Открыть Unblock", true, None);
        let off = MenuItem::with_id("off", "Отключить обход", true, None);
        let close = MenuItem::with_id("quit", "Закрыть", true, None);
        let sep = PredefinedMenuItem::separator();
        if menu.append_items(&[&open, &off, &sep, &close]).is_err() {
            return false;
        }

        let Ok(tray) = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Unblock")
            .with_icon(icon)
            .build()
        else {
            return false;
        };
        TRAY.with(|t| *t.borrow_mut() = Some(tray));

        MenuEvent::set_event_handler(Some(|e: MenuEvent| match e.id.0.as_str() {
            "open" => show(),
            "off" => {
                std::thread::spawn(|| {
                    kill_everything();
                    DISABLE.store(true, Ordering::SeqCst);
                    wake();
                });
            }
            "quit" => quit(),
            _ => {}
        }));
        TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                show();
            }
        }));
        true
    }
}

#[cfg(windows)]
pub use imp::init;

/// Не Windows: трея нет (программа собирается только для Windows, это нужно для проверок кода).
#[cfg(not(windows))]
pub fn init(_ctx: &eframe::egui::Context) -> bool {
    false
}
