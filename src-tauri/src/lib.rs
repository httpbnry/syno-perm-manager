mod commands;
mod config;
mod db;
mod models;
mod ssh;
mod syno;

use commands::AppState;
use ssh::client::SshState;
use std::sync::Mutex as StdMutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = app_data_dir();

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = format!("{}/syno-perm-manager.db", app_data_dir.display());
    let conn = db::init_db(&db_path).expect("failed to init database");

    let app_config = config::load_config(&conn);

    let state = AppState {
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
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
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
            commands::get_perm_matrix,
            commands::clear_logs,
            commands::clear_snapshots,
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
