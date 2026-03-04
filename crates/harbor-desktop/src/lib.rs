mod commands;
mod logging;

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tracing_subscriber::prelude::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize tracing with the Tauri log layer
            let log_layer = logging::TauriLogLayer::new(app.handle().clone());
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                        "harbor_core::gateway=info,harbor_desktop=info"
                            .parse()
                            .unwrap()
                    }),
                )
                .with(log_layer)
                .init();

            // Load harbor config and store as managed state
            let config = harbor_core::HarborConfig::load().unwrap_or_default();
            app.manage(commands::AppState::new(config));

            // Auto-start the gateway (lighthouse)
            {
                let handle = app.handle().clone();
                let state = app.state::<commands::AppState>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    match commands::start_gateway_inner(handle, &state).await {
                        Ok(msg) => tracing::info!("{}", msg),
                        Err(e) => tracing::warn!(error = %e, "Failed to auto-start gateway"),
                    }
                });
            }

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

            // -- System tray icon --
            let tray_status = MenuItem::with_id(
                app,
                "tray-status",
                "Lighthouse: Starting...",
                false,
                None::<&str>,
            )?;
            let tray_toggle =
                MenuItem::with_id(app, "tray-toggle", "Stop Lighthouse", true, None::<&str>)?;
            let tray_quit = MenuItem::with_id(app, "tray-quit", "Quit Harbor", true, None::<&str>)?;

            let tray_menu = tauri::menu::Menu::with_items(
                app,
                &[
                    &tray_status,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &tray_toggle,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &tray_quit,
                ],
            )?;

            let tray_toggle_for_event = tray_toggle.clone();

            let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
            let tray_icon_image =
                tauri::image::Image::from_bytes(tray_icon_bytes).expect("failed to load tray icon");

            TrayIconBuilder::new()
                .icon(tray_icon_image)
                .icon_as_template(true)
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app_handle, event| {
                    if event.id() == tray_toggle_for_event.id() {
                        let state = app_handle.state::<commands::AppState>();
                        let is_running = state.gateway_running();
                        if is_running {
                            let _ = commands::stop_gateway_inner(app_handle.clone(), &state);
                        } else {
                            let handle = app_handle.clone();
                            let state_clone = (*state).clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = commands::start_gateway_inner(handle, &state_clone).await;
                            });
                        }
                    } else if event.id() == "tray-quit" {
                        app_handle.exit(0);
                    }
                })
                .build(app)?;

            // Listen for gateway status changes to update tray menu text
            let tray_toggle_for_listen = tray_toggle.clone();
            let tray_status_for_listen = tray_status.clone();
            app.listen("gateway-status-changed", move |event| {
                let running = event.payload() == "true";
                let _ = tray_status_for_listen.set_text(if running {
                    "Lighthouse: Running"
                } else {
                    "Lighthouse: Off"
                });
                let _ = tray_toggle_for_listen.set_text(if running {
                    "Stop Lighthouse"
                } else {
                    "Start Lighthouse"
                });
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
            commands::catalog_list,
            commands::dock_native,
            commands::oauth_list_providers,
            commands::oauth_provider_for_server,
            commands::oauth_start_charter,
            commands::oauth_get_status,
            commands::oauth_revoke_charter,
            commands::oauth_set_custom_credentials,
            commands::gdrive_credential_paths,
            commands::discover_tools,
            commands::get_tool_filters,
            commands::set_tool_allowlist,
            commands::set_tool_blocklist,
            commands::set_tool_host_override,
            commands::start_gateway,
            commands::stop_gateway,
            commands::gateway_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Harbor");
}
