// Icône native de la zone de notification Windows (PB-025).
// Le masquage est toujours volontaire : fermer la fenêtre garde le comportement
// Windows habituel et quitte PadBridge en déclenchant le nettoyage HID.
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Masquer n'est pas fermer : les ressources restent détenues par PadBridge.
        let _ = window.hide();
    }
}

pub fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        std::io::Error::other("l'icône de l'application est absente")
    })?;
    let menu = MenuBuilder::new(app)
        .text("show", "Afficher PadBridge")
        .text("hide", "Masquer PadBridge")
        .separator()
        .text("quit", "Quitter PadBridge")
        .build()?;

    TrayIconBuilder::with_id("padbridge-tray")
        .icon(icon)
        .tooltip("PadBridge — manettes PlayStation")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "hide" => hide_main(app),
            // Sortie explicite, même lorsque la fenêtre est masquée. RunEvent::Exit
            // dans lifecycle.rs libère les ressources et arrête le moteur.
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
