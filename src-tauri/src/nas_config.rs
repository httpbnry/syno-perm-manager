use serde::{Deserialize, Serialize};
use crate::{commands::AppState, config, db};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct NasConfig {
    pub volume_path: String,
    pub synoacltool_path: String,
    pub synoshare_path: String,
    pub synouser_path: String,
    pub synogroup_path: String,
    pub find_path: String,
    pub excluded_folders: String,
    pub ssh_timeout_secs: u64,
    pub keepalive_secs: u64,
    pub startup_script: String,
}

impl Default for NasConfig {
    fn default() -> Self { Self::from_app(&config::AppConfig::default()) }
}

impl NasConfig {
    fn from_app(c: &config::AppConfig) -> Self {
        Self { volume_path: c.volume_path.clone(), synoacltool_path: c.synoacltool_path.clone(), synoshare_path: c.synoshare_path.clone(), synouser_path: c.synouser_path.clone(), synogroup_path: c.synogroup_path.clone(), find_path: c.find_path.clone(), excluded_folders: c.excluded_folders.join(", "), ssh_timeout_secs: c.ssh_timeout_secs, keepalive_secs: c.keepalive_secs, startup_script: String::new() }
    }
    pub fn validate(&self) -> Result<(), String> {
        for path in [&self.volume_path, &self.synoacltool_path, &self.synoshare_path, &self.synouser_path, &self.synogroup_path, &self.find_path] {
            if !path.starts_with('/') || path.contains(|c: char| c.is_control()) { return Err("Las rutas NAS deben ser absolutas y sin caracteres de control".into()); }
        }
        if !(10..=3600).contains(&self.ssh_timeout_secs) || !(5..=120).contains(&self.keepalive_secs) { return Err("Timeout SSH: 10–3600 s; keepalive: 5–120 s".into()); }
        if self.excluded_folders.contains(|c: char| c.is_control() || c == '/') { return Err("Las exclusiones deben ser nombres de carpeta separados por comas".into()); }
        Ok(())
    }
}

pub fn migrate(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS nas_configs (connection_id INTEGER PRIMARY KEY, content TEXT NOT NULL);")?;
    // Existing connections inherit the old settings once. New connections start independently.
    let mut legacy = NasConfig::from_app(&config::load_config(conn));
    legacy.startup_script = db::get_setting(conn, "startup_script")?.unwrap_or_default();
    let content = serde_json::to_string(&legacy)?;
    conn.execute("INSERT OR IGNORE INTO nas_configs(connection_id, content) SELECT id, ?1 FROM connections", [content])?;
    Ok(())
}

pub fn load(conn: &rusqlite::Connection, id: i64) -> anyhow::Result<NasConfig> {
    db::get_connection(conn, id)?;
    match conn.query_row::<String, _, _>("SELECT content FROM nas_configs WHERE connection_id=?1", [id], |r| r.get(0)) {
        Ok(json) => Ok(serde_json::from_str(&json)?),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(NasConfig::default()),
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
pub fn get_nas_config(id: i64, state: tauri::State<'_, AppState>) -> Result<NasConfig, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    load(&conn, id).map_err(|e| e.to_string())
}

pub fn store(conn: &rusqlite::Connection, id: i64, config: &NasConfig) -> anyhow::Result<()> {
    config.validate().map_err(|e| anyhow::anyhow!(e))?;
    db::get_connection(conn, id)?;
    let json = serde_json::to_string(config)?;
    conn.execute("INSERT INTO nas_configs(connection_id,content) VALUES (?1,?2) ON CONFLICT(connection_id) DO UPDATE SET content=excluded.content", rusqlite::params![id, json])?;
    Ok(())
}

pub fn export_all(conn: &rusqlite::Connection) -> anyhow::Result<serde_json::Value> {
    let mut stmt = conn.prepare("SELECT c.name, n.content FROM nas_configs n JOIN connections c ON c.id = n.connection_id ORDER BY c.name")?;
    let mut map = serde_json::Map::new();
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for row in rows {
        let (name, content) = row?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            map.insert(name, value);
        }
    }
    Ok(serde_json::Value::Object(map))
}

#[tauri::command]
pub fn save_nas_config(id: i64, config: NasConfig, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let connection = db::get_connection(&conn, id).map_err(|e| e.to_string())?;
    store(&conn, id, &config).map_err(|e| e.to_string())?;
    db::insert_audit_log(&conn, &connection.name, "update_nas_config", "connection", "Configuración NAS actualizada. Se aplicará al reconectar.", true, "").map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_are_isolated_and_migrated_once() {
        let conn = db::init_db(":memory:").unwrap();
        let a = db::insert_connection(&conn, "A", "a", 22, "u", false, "key", None).unwrap();
        let b = db::insert_connection(&conn, "B", "b", 22, "u", false, "key", None).unwrap();
        conn.execute("DELETE FROM nas_configs", []).unwrap();
        db::set_setting(&conn, "volume_path", "/volume2").unwrap();
        migrate(&conn).unwrap();
        db::set_setting(&conn, "volume_path", "/volume3").unwrap();
        migrate(&conn).unwrap();
        assert_eq!(load(&conn, a).unwrap().volume_path, "/volume2");
        let mut profile = load(&conn, a).unwrap(); profile.volume_path = "/volume4".into();
        conn.execute("UPDATE nas_configs SET content=?1 WHERE connection_id=?2", rusqlite::params![serde_json::to_string(&profile).unwrap(), a]).unwrap();
        assert_eq!(load(&conn, b).unwrap().volume_path, "/volume2");
    }
}
