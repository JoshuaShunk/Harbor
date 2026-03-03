mod commands;

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load harbor config and store as managed state
            let config = harbor_core::HarborConfig::load().unwrap_or_default();
            app.manage(commands::AppState::new(config));

            // Build native macOS application menu
            let settings_item = MenuItemBuilder::with_id("settings", "Settings...")
                .accelerator("CmdOrCtrl+,")
                .build(app)?;
            let check_updates_item =
                MenuItemBuilder::with_id("check-updates", "Check for Updates...").build(app)?;

            let app_menu = SubmenuBuilder::new(app, "Harbor")
                .about(None)
                .separator()
                .item(&settings_item)
                .item(&check_updates_item)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let window_menu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .maximize()
                .separator()
                .close_window()
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu, &edit_menu, &window_menu])
                .build()?;

            app.set_menu(menu)?;

            app.on_menu_event(move |app_handle, event| {
                if event.id() == settings_item.id() {
                    let _ = app_handle.emit("menu-navigate", "/settings");
                } else if event.id() == check_updates_item.id() {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let msg = match handle.updater() {
                            Ok(updater) => match updater.check().await {
                                Ok(Some(update)) => format!(
                                    "A new version (v{}) is available. Go to Settings to update.",
                                    update.version
                                ),
                                Ok(None) => {
                                    "You're running the latest version of Harbor.".to_string()
                                }
                                Err(e) => format!("Could not check for updates: {e}"),
                            },
                            Err(e) => format!("Could not check for updates: {e}"),
                        };
                        handle
                            .dialog()
                            .message(msg)
                            .title("Harbor")
                            .kind(MessageDialogKind::Info)
                            .show(|_| {});
                    });
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::add_server,
            commands::remove_server,
            commands::toggle_server,
            commands::sync_host,
            commands::sync_all,
            commands::connect_host,
            commands::disconnect_host,
            commands::vault_set,
            commands::vault_get,
            commands::vault_delete,
            commands::vault_list,
            commands::marketplace_search,
            commands::oauth_list_providers,
            commands::oauth_provider_for_server,
            commands::oauth_start_charter,
            commands::oauth_get_status,
            commands::oauth_revoke_charter,
            commands::oauth_set_custom_credentials,
            commands::gdrive_credential_paths,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Harbor");
}
