use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};

const TRAY_ID: &str = "main-tray";
const MENU_SHOW: &str = "show";
const MENU_STATS: &str = "open-stats";
const MENU_SETTINGS: &str = "open-settings";
const MENU_QUIT: &str = "quit";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(MENU_SHOW, "Show")
        .separator()
        .text(MENU_STATS, "Statistics")
        .text(MENU_SETTINGS, "Settings...")
        .separator()
        .text(MENU_QUIT, "Quit")
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .tooltip("Sessions Viewer")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
            _ => {}
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW => show_main_window(app),
            MENU_STATS | MENU_SETTINGS => {
                show_main_window(app);
                let _ = app.emit(
                    "menu://action",
                    serde_json::json!({ "id": event.id().as_ref() }),
                );
            }
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}
