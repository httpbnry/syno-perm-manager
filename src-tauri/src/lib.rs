mod commands;
mod comparison;
mod config;
mod db;
mod models;
mod nas_config;
mod history;
mod ssh;
mod syno;

use commands::AppState;
use ssh::client::SshState;
use std::sync::Mutex as StdMutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = app_data_dir();

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = format!("{}/syno-perm-manager.db", app_data_dir.display());
    let conn = db::init_db(&db_path).expect("failed to init database");

    let app_config = config::load_config(&conn);

    let state = AppState {
        comparison_scan: StdMutex::new(None),
        comparison: StdMutex::new(None),
        ssh: SshState {
            client: tokio::sync::Mutex::new(None),
            connection_name: tokio::sync::Mutex::new(String::new()),
        },
        db: StdMutex::new(conn),
        config: StdMutex::new(app_config),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::new(include_bytes!("../icons/icon-128.rgba"), 128, 128);
                window.set_icon(icon)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comparison::compare_users,
            comparison::cancel_comparison,
            comparison::apply_user_comparison,
            commands::test_connection,
            nas_config::get_nas_config,
            nas_config::save_nas_config,
            commands::save_connection,
            commands::list_connections,
            commands::delete_connection,
            commands::connect_to_nas,
            commands::disconnect,
            commands::list_shares,
            commands::list_users,
            commands::list_groups,
            commands::list_dirs,
            commands::get_acl,
            commands::dry_run,
            commands::apply_acl,
            commands::list_logs,
            history::query_history,
            history::get_history_detail,
            history::export_history,
            history::preview_snapshot,
            commands::list_snapshots,
            commands::restore_snapshot,
            commands::analyze_perms,
            commands::clear_acl_cache,
            commands::get_user_detail,
            commands::create_user,
            commands::delete_user,
            commands::set_user_password,
            commands::rename_user,
            commands::get_group_detail,
            commands::create_group,
            commands::delete_group,
            commands::add_group_member,
            commands::set_group_members,
            commands::rename_group,
            commands::get_dashboard_stats,
            commands::get_dashboard_audit_stats,
            commands::get_perm_matrix,
            commands::clear_logs,
            commands::clear_snapshots,
            commands::get_db_stats,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::run_startup_script,
            commands::test_startup_script,
            commands::get_config,
            commands::save_config,
            commands::export_config,
            commands::import_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn app_data_dir() -> std::path::PathBuf {
    if let Some(dir) = dirs::data_dir() {
        dir.join("syno-perm-manager")
    } else {
        std::path::PathBuf::from(".")
    }
}
