use std::sync::Mutex as StdMutex;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use rusqlite::Connection as DbConnection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::config;
use crate::models::*;
use crate::ssh::client::{AuthMethod, SshClient, SshState};
use crate::syno::{shell_escape, extract_error, AclProvider, SynologyProvider, SYNOUSER, SYNOGROUP};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("SSH: {0}")]
    Ssh(String),
    #[error("DB: {0}")]
    Db(String),
    #[error("No hay conexion activa al NAS")]
    NotConnected,
    #[error("Keyring: {0}")]
    Keyring(String),
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

type AppResult<T> = Result<T, AppError>;

fn ssh_err(e: anyhow::Error) -> AppError {
    AppError::Ssh(e.to_string())
}

fn db_err(e: anyhow::Error) -> AppError {
    AppError::Db(e.to_string())
}

pub struct AppState {
    pub ssh: SshState,
    pub db: StdMutex<DbConnection>,
    pub config: StdMutex<config::AppConfig>,
}

async fn get_connected_ssh(
    state: &AppState,
) -> AppResult<tokio::sync::MutexGuard<'_, Option<SshClient>>> {
    let guard = state.ssh.client.lock().await;
    if guard.is_none() {
        return Err(AppError::NotConnected);
    }
    Ok(guard)
}

#[tauri::command]
pub async fn test_connection(
    host: String,
    port: u16,
    username: String,
    password: String,
    use_sudo: bool,
    auth_method: String,
    key_path: Option<String>,
    key_passphrase: Option<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<String> {
    let known_host = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        db::get_known_host(&conn, &host, port).map_err(db_err)?
    };

    let auth = if auth_method == "key" {
        let kp = key_path.ok_or_else(|| AppError::Other("Falta la ruta de la clave SSH".to_string()))?;
        AuthMethod::Key { key_path: kp, passphrase: key_passphrase }
    } else {
        AuthMethod::Password(password)
    };

    let mut client = SshClient::connect(&host, port, &username, &auth, use_sudo, known_host.as_deref())
        .await
        .map_err(ssh_err)?;
    client.init_sudo_cache().await.map_err(ssh_err)?;

    if let Some(fp) = client.get_host_fingerprint() {
        if known_host.is_none() {
            let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
            let _ = db::save_known_host(&conn, &host, port, fp);
        }
    }

    let res = client.exec("uname -a").await.map_err(ssh_err)?;
    let _ = client.close().await;
    Ok(res.stdout.trim().to_string())
}

#[tauri::command]
pub async fn save_connection(
    input: ConnectionInput,
    state: tauri::State<'_, AppState>,
) -> AppResult<i64> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let id = db::insert_connection(
        &conn,
        &input.name,
        &input.host,
        input.port,
        &input.username,
        input.use_sudo,
        &input.auth_method,
        input.key_path.as_deref(),
    )
    .map_err(db_err)?;
    drop(conn);

    if input.auth_method == "password" {
        db::save_password(&input.name, &input.password)
            .map_err(|e| AppError::Keyring(e.to_string()))?;
    } else if input.auth_method == "key" && input.key_passphrase.is_some() {
        db::save_password(&format!("{}_key", input.name), input.key_passphrase.as_ref().unwrap())
            .map_err(|e| AppError::Keyring(e.to_string()))?;
    }
    Ok(id)
}

#[tauri::command]
pub async fn list_connections(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<crate::models::Connection>> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::list_connections(&conn).map_err(db_err)
}

#[tauri::command]
pub async fn delete_connection(
    id: i64,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::delete_connection(&conn, id).map_err(db_err)
}

#[tauri::command]
pub async fn connect_to_nas(
    id: i64,
    state: tauri::State<'_, AppState>,
) -> AppResult<String> {
    let (name, host, port, username, use_sudo, auth_method, key_path) = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let c = db::get_connection(&conn, id).map_err(db_err)?;
        (
            c.name.clone(),
            c.host.clone(),
            c.port,
            c.username.clone(),
            c.use_sudo,
            c.auth_method.clone(),
            c.key_path.clone(),
        )
    };

    let startup_script = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        db::get_setting(&conn, "startup_script").map_err(db_err)?
    };

    if let Some(script) = &startup_script {
        if !script.trim().is_empty() {
            let mut cmd = Command::new("cmd");
            cmd.arg("/S").arg("/C").raw_arg(script);
            let _ = cmd.output();
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }

    let auth = if auth_method == "key" {
        let kp = key_path.ok_or_else(|| AppError::Other("Falta la ruta de la clave SSH".to_string()))?;
        let passphrase = db::get_password(&format!("{}_key", name)).ok().filter(|p| !p.is_empty());
        AuthMethod::Key { key_path: kp, passphrase }
    } else {
        let password = db::get_password(&name).map_err(|e| AppError::Keyring(e.to_string()))?;
        AuthMethod::Password(password)
    };

    let known_host = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        db::get_known_host(&conn, &host, port).map_err(db_err)?
    };

    let mut client = SshClient::connect(&host, port, &username, &auth, use_sudo, known_host.as_deref())
        .await
        .map_err(ssh_err)?;
    client.init_sudo_cache().await.map_err(ssh_err)?;

    if let Some(fp) = client.get_host_fingerprint() {
        if known_host.is_none() {
            let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
            let _ = db::save_known_host(&conn, &host, port, fp);
        }
    }

    {
        let mut guard = state.ssh.client.lock().await;
        *guard = Some(client);
    }
    {
        let mut name_guard = state.ssh.connection_name.lock().await;
        *name_guard = name.clone();
    }

    Ok(name)
}

#[tauri::command]
pub async fn disconnect(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let mut guard = state.ssh.client.lock().await;
    if let Some(mut client) = guard.take() {
        let _ = client.close().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_shares(state: tauri::State<'_, AppState>) -> AppResult<Vec<ShareFolder>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    provider.list_shares(client).await.map_err(ssh_err)
}

#[tauri::command]
pub async fn list_users(state: tauri::State<'_, AppState>) -> AppResult<Vec<String>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    provider.list_users(client).await.map_err(ssh_err)
}

#[tauri::command]
pub async fn list_groups(state: tauri::State<'_, AppState>) -> AppResult<Vec<String>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    provider.list_groups(client).await.map_err(ssh_err)
}

#[tauri::command]
pub async fn list_dirs(
    path: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<DirNode>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    provider.list_dirs(client, &path).await.map_err(ssh_err)
}

#[tauri::command]
pub async fn get_acl(
    path: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AclEntry>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let acls = provider.get_acl(client, &path).await.map_err(ssh_err)?;
    Ok(acls)
}

#[tauri::command]
pub async fn dry_run(
    request: ApplyRequest,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AclDiff>> {
    let provider = SynologyProvider::new();
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    provider
        .dry_run(client, &request.paths, &request.entries, request.recursive)
        .await
        .map_err(ssh_err)
}

#[tauri::command]
pub async fn apply_acl(
    request: ApplyRequest,
    state: tauri::State<'_, AppState>,
) -> AppResult<ApplyResult> {
    let provider = SynologyProvider::new();

    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };

    let mut snapshot_ids: Vec<String> = Vec::new();

    for path in &request.paths {
        let snapshot = {
            let mut guard = get_connected_ssh(&state).await?;
            let client = guard.as_mut().unwrap();
            provider
                .snapshot_acl(client, path, request.recursive)
                .await
                .map_err(ssh_err)?
        };
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let snap_id = db::insert_snapshot(&conn, &conn_name, path, request.recursive, &snapshot)
            .map_err(db_err)?;
        snapshot_ids.push(snap_id.to_string());
    }

    let result = {
        let mut guard = get_connected_ssh(&state).await?;
        let client = guard.as_mut().unwrap();
        provider
            .apply_acl(client, &request.paths, &request.entries, request.recursive)
            .await
            .map_err(ssh_err)?
    };

    {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let paths_str = request.paths.join(", ");
        let details = format!(
            "Entradas: {} | Recursivo: {} | Modificadas: {} | Errores: {}",
            request.entries.len(),
            request.recursive,
            result.paths_modified,
            result.errors.len()
        );
        let _ = db::insert_audit_log(
            &conn,
            &conn_name,
            "apply_acl",
            &paths_str,
            &details,
            result.success,
            &snapshot_ids.join(","),
        );
    }

    Ok(result)
}

#[tauri::command]
pub async fn list_logs(
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AuditLog>> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::list_audit_logs(&conn, limit.unwrap_or(100)).map_err(db_err)
}

#[tauri::command]
pub async fn list_snapshots(
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SnapshotInfo>> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::list_snapshots(&conn, limit.unwrap_or(50)).map_err(db_err)
}

#[tauri::command]
pub async fn clear_logs(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    conn.execute("DELETE FROM audit_logs", [])
        .map_err(|e| AppError::Db(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn clear_snapshots(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    conn.execute("DELETE FROM acl_snapshots", [])
        .map_err(|e| AppError::Db(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn restore_snapshot(
    snapshot_id: i64,
    state: tauri::State<'_, AppState>,
) -> AppResult<ApplyResult> {
    let provider = SynologyProvider::new();

    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };

    let snapshot_data = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        db::get_snapshot_by_id(&conn, snapshot_id).map_err(db_err)?
    };

    let snapshot_json = serde_json::to_string(&snapshot_data)
        .map_err(|e| AppError::Other(e.to_string()))?;

    let paths: Vec<String> = snapshot_data.snapshots.iter().map(|s| s.path.clone()).collect();

    let result = {
        let mut guard = get_connected_ssh(&state).await?;
        let client = guard.as_mut().unwrap();
        provider
            .restore_acl(client, &snapshot_json)
            .await
            .map_err(ssh_err)?
    };

    {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let paths_str = paths.join(", ");
        let details = format!(
            "Restore snapshot #{} | Modificadas: {} | Errores: {}",
            snapshot_id,
            result.paths_modified,
            result.errors.len()
        );
        let _ = db::insert_audit_log(
            &conn,
            &conn_name,
            "restore_snapshot",
            &paths_str,
            &details,
            result.success,
            &snapshot_id.to_string(),
        );
    }

    Ok(result)
}

#[tauri::command]
pub async fn analyze_perms(
    paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<PathAcl>> {
    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };

    let mut results = Vec::new();
    let mut uncached_paths: Vec<String> = Vec::new();

    for path in &paths {
        let cached = {
            let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
            db::get_cached_acl(&conn, &conn_name, path).map_err(db_err)?
        };

        if let Some(content) = cached {
            let entries = serde_json::from_str::<Vec<AclEntry>>(&content).unwrap_or_default();
            results.push(PathAcl {
                path: path.clone(),
                entries,
            });
        } else {
            uncached_paths.push(path.clone());
        }
    }

    if !uncached_paths.is_empty() {
        let paths_arg = uncached_paths
            .iter()
            .map(|p| shell_escape(p))
            .collect::<Vec<_>>()
            .join(" ");

        let script = format!(
            "for d in {}; do echo \"@@@PATH:$d@@@\"; /usr/syno/bin/synoacltool -get \"$d\" 2>/dev/null; echo \"@@@END@@@\"; done",
            paths_arg
        );

        let res = {
            let mut guard = get_connected_ssh(&state).await?;
            let client = guard.as_mut().unwrap();
            client.exec_script(&script).await.map_err(ssh_err)?
        };

        let mut current_path = String::new();
        let mut current_output = String::new();

        for line in res.stdout.lines() {
            if line.starts_with("@@@PATH:") && line.ends_with("@@@") {
                current_path = line
                    .trim_start_matches("@@@PATH:")
                    .trim_end_matches("@@@")
                    .to_string();
                current_output.clear();
            } else if line == "@@@END@@@" {
                if !current_path.is_empty() {
                    let entries = crate::syno::parser::parse_acl_output(&current_output);

                    let json = serde_json::to_string(&entries)
                        .map_err(|e| AppError::Other(e.to_string()))?;
                    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
                    let _ = db::cache_acl(&conn, &conn_name, &current_path, &json).map_err(db_err);

                    results.push(PathAcl {
                        path: current_path.clone(),
                        entries,
                    });
                    current_path.clear();
                }
            } else {
                current_output.push_str(line);
                current_output.push('\n');
            }
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn clear_acl_cache(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::clear_acl_cache(&conn, &conn_name).map_err(db_err)
}

const SYNOUSER_CMD: &str = SYNOUSER;
const SYNOGROUP_CMD: &str = SYNOGROUP;

async fn log_action(
    state: &AppState,
    action: &str,
    path: &str,
    details: &str,
    success: bool,
) {
    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };
    let conn = state.db.lock();
    if let Ok(conn) = conn {
        let _ = db::insert_audit_log(&conn, &conn_name, action, path, details, success, "");
    }
}

#[tauri::command]
pub async fn get_user_detail(
    username: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<UserDetail> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --get {}", SYNOUSER_CMD, shell_escape(&username)))
        .await
        .map_err(ssh_err)?;
    Ok(crate::syno::parser::parse_user_detail(&res.stdout))
}

#[tauri::command]
pub async fn create_user(
    input: CreateUserInput,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let cmd = format!(
        "{} --add {} {} {} {} {} {}",
        SYNOUSER_CMD,
        shell_escape(&input.username),
        shell_escape(&input.password),
        shell_escape(&input.full_name),
        if input.expired { 1 } else { 0 },
        shell_escape(&input.mail),
        input.privilege
    );
    let res = client.exec(&cmd).await.map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "create_user", &input.username, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "create_user", &input.username, &format!("Usuario creado | Mail: {}", input.mail), true).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_user(
    usernames: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let names: Vec<String> = usernames.iter().map(|u| shell_escape(u)).collect();
    let res = client
        .exec(&format!("{} --del {}", SYNOUSER_CMD, names.join(" ")))
        .await
        .map_err(ssh_err)?;
    let names_str = usernames.join(", ");
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "delete_user", &names_str, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "delete_user", &names_str, "Usuario(s) eliminado(s)", true).await;
    Ok(())
}

#[tauri::command]
pub async fn set_user_password(
    username: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --setpw {} {}", SYNOUSER_CMD, shell_escape(&username), shell_escape(&password)))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "set_user_password", &username, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "set_user_password", &username, "Password cambiada", true).await;
    Ok(())
}

#[tauri::command]
pub async fn rename_user(
    old_name: String,
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --rename {} {}", SYNOUSER_CMD, shell_escape(&old_name), shell_escape(&new_name)))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "rename_user", &old_name, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "rename_user", &old_name, &format!("Renombrado a {}", new_name), true).await;
    Ok(())
}

#[tauri::command]
pub async fn get_group_detail(
    groupname: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<GroupDetail> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --get {}", SYNOGROUP_CMD, shell_escape(&groupname)))
        .await
        .map_err(ssh_err)?;
    let mut detail = crate::syno::parser::parse_group_detail(&res.stdout);

    let desc_res = client
        .exec(&format!("{} --descget {}", SYNOGROUP_CMD, shell_escape(&groupname)))
        .await;
    if let Ok(desc) = desc_res {
        let desc_clean = desc.stdout.trim();
        if !desc_clean.is_empty() {
            detail.description = desc_clean.to_string();
        }
    }

    Ok(detail)
}

#[tauri::command]
pub async fn create_group(
    input: CreateGroupInput,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let members = if input.members.is_empty() {
        String::new()
    } else {
        let escaped: Vec<String> = input.members.iter().map(|m| shell_escape(m)).collect();
        format!(" {}", escaped.join(" "))
    };
    let res = client
        .exec(&format!("{} --add {}{}", SYNOGROUP_CMD, shell_escape(&input.name), members))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "create_group", &input.name, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "create_group", &input.name, &format!("Grupo creado | Miembros: {}", input.members.join(", ")), true).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_group(
    groupnames: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let names: Vec<String> = groupnames.iter().map(|g| shell_escape(g)).collect();
    let res = client
        .exec(&format!("{} --del {}", SYNOGROUP_CMD, names.join(" ")))
        .await
        .map_err(ssh_err)?;
    let names_str = groupnames.join(", ");
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "delete_group", &names_str, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "delete_group", &names_str, "Grupo(s) eliminado(s)", true).await;
    Ok(())
}

#[tauri::command]
pub async fn add_group_member(
    groupname: String,
    username: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --memberadd {} {}", SYNOGROUP_CMD, shell_escape(&groupname), shell_escape(&username)))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "add_group_member", &groupname, &format!("Error anadiendo {}: {}", username, err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "add_group_member", &groupname, &format!("Anadido {}", username), true).await;
    Ok(())
}

#[tauri::command]
pub async fn set_group_members(
    groupname: String,
    members: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let escaped: Vec<String> = members.iter().map(|m| shell_escape(m)).collect();
    let res = client
        .exec(&format!("{} --member {} {}", SYNOGROUP_CMD, shell_escape(&groupname), escaped.join(" ")))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "set_group_members", &groupname, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "set_group_members", &groupname, &format!("Miembros: {}", members.join(", ")), true).await;
    Ok(())
}

#[tauri::command]
pub async fn rename_group(
    old_name: String,
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut guard = get_connected_ssh(&state).await?;
    let client = guard.as_mut().unwrap();
    let res = client
        .exec(&format!("{} --rename {} {}", SYNOGROUP_CMD, shell_escape(&old_name), shell_escape(&new_name)))
        .await
        .map_err(ssh_err)?;
    if res.exit_code != 0 {
        let err = extract_error(&res);
        log_action(&state, "rename_group", &old_name, &format!("Error: {}", err), false).await;
        return Err(AppError::Ssh(err));
    }
    log_action(&state, "rename_group", &old_name, &format!("Renombrado a {}", new_name), true).await;
    Ok(())
}

#[tauri::command]
pub async fn get_dashboard_stats(
    state: tauri::State<'_, AppState>,
) -> AppResult<DashboardStats> {
    let provider = SynologyProvider::new();

    let (users, groups, shares) = {
        let mut guard = get_connected_ssh(&state).await?;
        let client = guard.as_mut().unwrap();
        let u = provider.list_users(client).await.unwrap_or_default();
        let g = provider.list_groups(client).await.unwrap_or_default();
        let s = provider.list_shares(client).await.unwrap_or_default();
        (u, g, s)
    };

    let (recent_logs, total, success, failed) = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let logs = db::list_audit_logs(&conn, 5).map_err(db_err)?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))
            .unwrap_or(0);
        let success: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_logs WHERE success = 1", [], |row| row.get(0))
            .unwrap_or(0);
        let failed: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_logs WHERE success = 0", [], |row| row.get(0))
            .unwrap_or(0);
        (logs, total, success, failed)
    };

    Ok(DashboardStats {
        user_count: users.len(),
        group_count: groups.len(),
        share_count: shares.len(),
        recent_logs,
        total_changes: total,
        successful_changes: success,
        failed_changes: failed,
    })
}

fn compute_perm_color(entries: &[AclEntry], ptype: &str, pname: &str) -> (String, String) {
    let mut has_allow = false;
    let mut allow_r = false;
    let mut allow_w = false;
    let mut has_deny = false;
    let mut perms_str = String::new();

    for entry in entries {
        let parts: Vec<&str> = entry.principal_type.split(':').collect();
        let entry_type = parts.first().unwrap_or(&"");
        let entry_allow = parts.get(1).unwrap_or(&"allow");

        if *entry_type != ptype || entry.name != pname {
            continue;
        }

        if *entry_allow == "deny" {
            has_deny = true;
        } else {
            has_allow = true;
            perms_str = entry.permissions.clone();
            if entry.permissions.contains('r') {
                allow_r = true;
            }
            if entry.permissions.contains('w') {
                allow_w = true;
            }
        }
    }

    let color = if has_deny && !has_allow {
        "red"
    } else if !has_allow {
        "red"
    } else if allow_w {
        "green"
    } else if allow_r {
        "orange"
    } else {
        "red"
    };

    (color.to_string(), perms_str)
}

#[tauri::command]
pub async fn get_perm_matrix(
    paths: Vec<String>,
    principals: Vec<(String, String)>,
    state: tauri::State<'_, AppState>,
) -> AppResult<PermMatrix> {
    let conn_name = {
        let guard = state.ssh.connection_name.lock().await;
        guard.clone()
    };

    let mut acls: std::collections::HashMap<String, Vec<AclEntry>> =
        std::collections::HashMap::new();

    for path in &paths {
        let cached = {
            let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
            db::get_cached_acl(&conn, &conn_name, path).map_err(db_err)?
        };

        let entries = if let Some(content) = cached {
            serde_json::from_str::<Vec<AclEntry>>(&content).unwrap_or_default()
        } else {
            let provider = SynologyProvider::new();
            let mut guard = get_connected_ssh(&state).await?;
            let client = guard.as_mut().unwrap();
            let fetched = provider.get_acl(client, path).await.map_err(ssh_err)?;

            let json = serde_json::to_string(&fetched)
                .map_err(|e| AppError::Other(e.to_string()))?;
            let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
            let _ = db::cache_acl(&conn, &conn_name, path, &json).map_err(db_err);

            fetched
        };

        acls.insert(path.clone(), entries);
    }

    let mut rows = Vec::new();
    for (ptype, pname) in &principals {
        let mut cells = Vec::new();
        for path in &paths {
            let entries = acls.get(path).cloned().unwrap_or_default();
            let (color, perms) = compute_perm_color(&entries, ptype, pname);
            cells.push(PermMatrixCell {
                path: path.clone(),
                color,
                permissions: perms,
            });
        }
        rows.push(PermMatrixRow {
            principal: pname.clone(),
            principal_type: ptype.clone(),
            cells,
        });
    }

    Ok(PermMatrix { paths, rows })
}

#[tauri::command]
pub async fn get_app_setting(
    key: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<String>> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::get_setting(&conn, &key).map_err(db_err)
}

#[tauri::command]
pub async fn set_app_setting(
    key: String,
    value: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::set_setting(&conn, &key, &value).map_err(db_err)
}

#[tauri::command]
pub async fn run_startup_script(
    state: tauri::State<'_, AppState>,
) -> AppResult<StartupResult> {
    let script = {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        db::get_setting(&conn, "startup_script").map_err(db_err)?
    };

    if script.is_none() || script.as_ref().unwrap().trim().is_empty() {
        return Ok(StartupResult {
            ran: false,
            output: String::new(),
            success: true,
        });
    }

    let script = script.unwrap();
    let mut cmd = Command::new("cmd");
    cmd.arg("/S").arg("/C").raw_arg(&script);
    let output = cmd
        .output()
        .map_err(|e| AppError::Other(format!("No se pudo ejecutar el script: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    let success = output.status.success();

    log_action(
        &state,
        "run_startup_script",
        "local",
        &format!("Exit: {} | Output: {}", output.status, combined.chars().take(200).collect::<String>()),
        success,
    )
    .await;

    Ok(StartupResult {
        ran: true,
        output: combined,
        success,
    })
}

#[tauri::command]
pub async fn test_startup_script(
    script: String,
) -> AppResult<StartupResult> {
    let mut cmd = Command::new("cmd");
    cmd.arg("/S").arg("/C").raw_arg(&script);
    let output = cmd
        .output()
        .map_err(|e| AppError::Other(format!("No se pudo ejecutar: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    Ok(StartupResult {
        ran: true,
        output: combined,
        success: output.status.success(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDto {
    pub volume_path: String,
    pub synoacltool_path: String,
    pub synoshare_path: String,
    pub synouser_path: String,
    pub synogroup_path: String,
    pub find_path: String,
    pub excluded_folders: String,
    pub ssh_timeout_secs: u64,
    pub keepalive_secs: u64,
    pub log_retention_days: i64,
    pub snapshot_retention_count: i64,
    pub password_length: usize,
    pub password_special_chars: bool,
    pub username_format: String,
    pub description_template: String,
    pub theme: String,
    pub language: String,
}

#[tauri::command]
pub async fn get_config(
    state: tauri::State<'_, AppState>,
) -> AppResult<ConfigDto> {
    let config = state.config.lock().map_err(|e| AppError::Other(e.to_string()))?;
    Ok(ConfigDto {
        volume_path: config.volume_path.clone(),
        synoacltool_path: config.synoacltool_path.clone(),
        synoshare_path: config.synoshare_path.clone(),
        synouser_path: config.synouser_path.clone(),
        synogroup_path: config.synogroup_path.clone(),
        find_path: config.find_path.clone(),
        excluded_folders: config.excluded_folders.join(", "),
        ssh_timeout_secs: config.ssh_timeout_secs,
        keepalive_secs: config.keepalive_secs,
        log_retention_days: config.log_retention_days,
        snapshot_retention_count: config.snapshot_retention_count,
        password_length: config.password_length,
        password_special_chars: config.password_special_chars,
        username_format: config.username_format.clone(),
        description_template: config.description_template.clone(),
        theme: config.theme.clone(),
        language: config.language.clone(),
    })
}

#[tauri::command]
pub async fn save_config(
    dto: ConfigDto,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    {
        let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let pairs: &[(&str, &str)] = &[
            ("volume_path", &dto.volume_path),
            ("synoacltool_path", &dto.synoacltool_path),
            ("synoshare_path", &dto.synoshare_path),
            ("synouser_path", &dto.synouser_path),
            ("synogroup_path", &dto.synogroup_path),
            ("find_path", &dto.find_path),
            ("excluded_folders", &dto.excluded_folders),
            ("ssh_timeout_secs", &dto.ssh_timeout_secs.to_string()),
            ("keepalive_secs", &dto.keepalive_secs.to_string()),
            ("log_retention_days", &dto.log_retention_days.to_string()),
            ("snapshot_retention_count", &dto.snapshot_retention_count.to_string()),
            ("password_length", &dto.password_length.to_string()),
            ("password_special_chars", &dto.password_special_chars.to_string()),
            ("username_format", &dto.username_format),
            ("description_template", &dto.description_template),
            ("theme", &dto.theme),
            ("language", &dto.language),
        ];
        for (key, value) in pairs {
            let _ = db::set_setting(&conn, key, value);
        }
    }

    {
        let mut config = state.config.lock().map_err(|e| AppError::Other(e.to_string()))?;
        config.volume_path = dto.volume_path;
        config.synoacltool_path = dto.synoacltool_path;
        config.synoshare_path = dto.synoshare_path;
        config.synouser_path = dto.synouser_path;
        config.synogroup_path = dto.synogroup_path;
        config.find_path = dto.find_path;
        config.excluded_folders = dto.excluded_folders.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        config.ssh_timeout_secs = dto.ssh_timeout_secs;
        config.keepalive_secs = dto.keepalive_secs;
        config.log_retention_days = dto.log_retention_days;
        config.snapshot_retention_count = dto.snapshot_retention_count;
        config.password_length = dto.password_length;
        config.password_special_chars = dto.password_special_chars;
        config.username_format = dto.username_format;
        config.description_template = dto.description_template;
        config.theme = dto.theme;
        config.language = dto.language;
    }

    Ok(())
}

#[tauri::command]
pub async fn export_config(
    state: tauri::State<'_, AppState>,
) -> AppResult<String> {
    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;

    let mut settings = serde_json::json!({});
    let mut stmt = conn.prepare("SELECT key, value FROM app_settings")
        .map_err(|e| AppError::Db(e.to_string()))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| AppError::Db(e.to_string()))?;

    for row in rows {
        let (key, value) = row.map_err(|e| AppError::Db(e.to_string()))?;
        settings[&key] = serde_json::Value::String(value);
    }

    let mut connections = Vec::new();
    let conn_list = db::list_connections(&conn).map_err(db_err)?;
    for c in conn_list {
        connections.push(serde_json::json!({
            "name": c.name,
            "host": c.host,
            "port": c.port,
            "username": c.username,
            "use_sudo": c.use_sudo,
            "auth_method": c.auth_method,
            "key_path": c.key_path,
        }));
    }

    let startup = db::get_setting(&conn, "startup_script").map_err(db_err)?;

    let export = serde_json::json!({
        "version": "1.0.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "settings": settings,
        "connections": connections,
        "startup_script": startup,
    });

    Ok(serde_json::to_string_pretty(&export).unwrap_or_default())
}

#[tauri::command]
pub async fn import_config(
    json: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let data: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| AppError::Other(format!("JSON invalido: {}", e)))?;

    let conn = state.db.lock().map_err(|e| AppError::Other(e.to_string()))?;

    if let Some(settings) = data.get("settings").and_then(|v| v.as_object()) {
        for (key, value) in settings {
            if let Some(val_str) = value.as_str() {
                let _ = db::set_setting(&conn, key, val_str);
            }
        }
    }

    if let Some(script) = data.get("startup_script").and_then(|v| v.as_str()) {
        let _ = db::set_setting(&conn, "startup_script", script);
    }

    if let Some(connections) = data.get("connections").and_then(|v| v.as_array()) {
        for c in connections {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let host = c.get("host").and_then(|v| v.as_str()).unwrap_or("");
            let port = c.get("port").and_then(|v| v.as_i64()).unwrap_or(22) as u16;
            let username = c.get("username").and_then(|v| v.as_str()).unwrap_or("");
            let use_sudo = c.get("use_sudo").and_then(|v| v.as_bool()).unwrap_or(true);
            let auth_method = c.get("auth_method").and_then(|v| v.as_str()).unwrap_or("password");
            let key_path = c.get("key_path").and_then(|v| v.as_str());

            if !name.is_empty() {
                let _ = db::insert_connection(&conn, name, host, port, username, use_sudo, auth_method, key_path);
            }
        }
    }

    Ok(())
}
