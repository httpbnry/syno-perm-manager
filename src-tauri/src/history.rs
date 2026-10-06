use serde::{Deserialize, Serialize};
use crate::{commands::AppState, models::{AuditLog, SnapshotData}, db};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct HistoryFilter {
    pub search: String,
    pub connection: String,
    pub action: String,
    pub status: String,
    pub from: String,
    pub to: String,
    pub page: u32,
    pub page_size: u32,
}
#[derive(Serialize)]
pub struct HistoryPage { rows: Vec<AuditLog>, total: i64, connections: Vec<String>, actions: Vec<String> }

const FILTER: &str = " WHERE (?1='' OR instr(lower(path || ' ' || action || ' ' || coalesce(details,'')),lower(?1))>0) AND (?2='' OR connection_name=?2) AND (?3='' OR action=?3) AND (?4='' OR success=CASE WHEN ?4='success' THEN 1 ELSE 0 END) AND (?5='' OR timestamp >= ?5) AND (?6='' OR timestamp < datetime(?6,'+1 day'))";

fn validate(f: &HistoryFilter) -> Result<(), String> {
    for d in [&f.from, &f.to] {
        if !d.is_empty() && chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err("Fecha inválida".into()); }
    }
    if !["", "success", "error"].contains(&f.status.as_str()) { return Err("Estado inválido".into()); }
    Ok(())
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditLog> {
    Ok(AuditLog { id: row.get(0)?, timestamp: row.get(1)?, connection_name: row.get(2)?, action: row.get(3)?, path: row.get(4)?, details: row.get(5)?, success: row.get(6)?, snapshot_ids: row.get(7)? })
}

fn query(conn: &rusqlite::Connection, f: &HistoryFilter, export: bool) -> Result<(Vec<AuditLog>, i64), String> {
    validate(f)?;
    let params = rusqlite::params![f.search, f.connection, f.action, f.status, f.from, f.to];
    let total = conn.query_row(&format!("SELECT count(*) FROM audit_logs{FILTER}"), params, |r| r.get(0)).map_err(|e| e.to_string())?;
    let size = if export { 10000 } else { f.page_size.clamp(1, 100) as i64 };
    let offset = if export { 0 } else { i64::from(f.page) * size };
    let detail = if export { "coalesce(details,'')" } else { "CASE WHEN action='user_sync_backup' THEN 'Respaldo de usuarios, grupos y ACL' ELSE substr(coalesce(details,''),1,200) END" };
    let sql = format!("SELECT id,timestamp,connection_name,action,path,{detail},success,coalesce(snapshot_ids,'') FROM audit_logs{FILTER} ORDER BY id DESC LIMIT ?7 OFFSET ?8");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![f.search, f.connection, f.action, f.status, f.from, f.to, size, offset], row).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok((rows, total))
}

#[tauri::command]
pub async fn query_history(filter: HistoryFilter, state: tauri::State<'_, AppState>) -> Result<HistoryPage, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let (rows, total) = query(&conn, &filter, false)?;
    let distinct = |column: &str| -> Result<Vec<String>, String> {
        let mut stmt = conn.prepare(&format!("SELECT DISTINCT {column} FROM audit_logs ORDER BY {column}")).map_err(|e| e.to_string())?;
        let result = stmt.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        Ok(result)
    };
    Ok(HistoryPage { rows, total, connections: distinct("connection_name")?, actions: distinct("action")? })
}

#[tauri::command]
pub async fn get_history_detail(id: i64, state: tauri::State<'_, AppState>) -> Result<AuditLog, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT id,timestamp,connection_name,action,path,coalesce(details,''),success,coalesce(snapshot_ids,'') FROM audit_logs WHERE id=?1", [id], row).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct HistoryExport { rows: Vec<AuditLog>, total: i64, truncated: bool }
#[tauri::command]
pub async fn export_history(filter: HistoryFilter, state: tauri::State<'_, AppState>) -> Result<HistoryExport, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let (rows, total) = query(&conn, &filter, true)?;
    let truncated = total > rows.len() as i64;
    Ok(HistoryExport { rows, total, truncated })
}

#[tauri::command]
pub async fn preview_snapshot(id: i64, state: tauri::State<'_, AppState>) -> Result<SnapshotData, String> {
    let name = state.ssh.connection_name.lock().await.clone();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let owner: String = conn.query_row("SELECT connection_name FROM acl_snapshots WHERE id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if name != owner { return Err("Conecta al NAS propietario del snapshot".into()); }
    db::get_snapshot_by_id(&conn, id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_pagination_and_lazy_backup_details() {
        let conn = db::init_db(":memory:").unwrap();
        db::insert_audit_log(&conn, "A", "user_sync_backup", "/a", "large backup", true, "").unwrap();
        db::insert_audit_log(&conn, "B", "apply_acl", "/b", "denied", false, "").unwrap();
        let f = HistoryFilter { connection: "A".into(), page_size: 1, ..Default::default() };
        let (rows, total) = query(&conn, &f, false).unwrap();
        assert_eq!(total, 1); assert!(!rows[0].details.contains("large backup"));
        assert_eq!(query(&conn, &f, true).unwrap().0[0].details, "large backup");
        let f = HistoryFilter { status: "error".into(), page_size: 50, ..Default::default() };
        assert_eq!(query(&conn, &f, false).unwrap().0[0].connection_name, "B");
        let f = HistoryFilter { search: "' OR 1=1 --".into(), page_size: 50, ..Default::default() };
        assert_eq!(query(&conn, &f, false).unwrap().1, 0);
    }
}
