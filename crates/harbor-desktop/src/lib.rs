mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Load harbor config and store as managed state
            let config = harbor_core::HarborConfig::load().unwrap_or_default();
            app.manage(commands::AppState::new(config));
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
