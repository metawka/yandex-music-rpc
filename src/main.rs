#![windows_subsystem = "windows"]

mod config;
mod discord;
mod presence;
mod smtc;
mod yandex;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, SW_SHOWNORMAL, SetTimer, TranslateMessage, WM_TIMER,
};
use windows::core::{HSTRING, w};

use presence::Shared;

const TITLE: &str = "Yandex Music RPC";

fn main() {
    if std::env::args().any(|a| a == "--dump") {
        let _ = std::fs::create_dir_all(config::dir());
        let _ = std::fs::write(config::dir().join("dump.txt"), presence::dump());
        return;
    }
    unsafe {
        let _mutex = CreateMutexW(None, true, w!("Local\\YandexMusicRPC"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
    }

    let shared = Arc::new(Shared { now_playing: Mutex::new(String::new()), discord: AtomicBool::new(false) });
    let worker = shared.clone();
    std::thread::spawn(move || presence::run(worker));

    // Menu
    let now = MenuItem::new("Ничего не играет", false, None);
    let link = MenuItem::new("Discord: не подключён", false, None);
    let autostart = CheckMenuItem::new("Запускать вместе с Windows", true, config::autostart_enabled(), None);
    let github = MenuItem::new("Страница проекта", true, None);
    let quit = MenuItem::new("Выход", true, None);

    let menu = Menu::new();
    menu.append_items(&[
        &now,
        &link,
        &PredefinedMenuItem::separator(),
        &autostart,
        &PredefinedMenuItem::separator(),
        &github,
        &quit,
    ])
    .unwrap();

    let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip(TITLE);
    if let Ok(icon) = Icon::from_resource(1, None) {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build().unwrap();

    // Loop
    let mut shown = (String::from("-"), false);
    unsafe {
        SetTimer(None, 0, 1000, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                let id = event.id();
                if id == autostart.id() {
                    config::set_autostart(autostart.is_checked());
                } else if id == github.id() {
                    ShellExecuteW(None, w!("open"), &HSTRING::from(presence::REPO), None, None, SW_SHOWNORMAL);
                } else if id == quit.id() {
                    drop(tray);
                    std::process::exit(0);
                }
            }

            if msg.message == WM_TIMER {
                let state = (shared.now_playing.lock().unwrap().clone(), shared.discord.load(Ordering::Relaxed));
                if state != shown {
                    now.set_text(if state.0.is_empty() { "Ничего не играет".to_string() } else { cut(&state.0, 60) });
                    link.set_text(if state.1 { "Discord: подключён" } else { "Discord: не подключён" });
                    let tip = if state.0.is_empty() { TITLE.to_string() } else { cut(&format!("{TITLE}\n{}", state.0), 120) };
                    let _ = tray.set_tooltip(Some(tip));
                    shown = state;
                }
            }
        }
    }
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    }
}
