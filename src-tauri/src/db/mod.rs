use anyhow::Result;
use rusqlite::Connection;

pub fn init_db(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    create_schema(&conn)?;
    migrate_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS connections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            host TEXT NOT NULL,
            port INTEGER NOT NULL DEFAULT 22,
            username TEXT NOT NULL,
            use_sudo INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            connection_name TEXT NOT NULL,
            action TEXT NOT NULL,
            path TEXT NOT NULL,
            details TEXT,
            success INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS acl_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            connection_name TEXT NOT NULL,
            path TEXT NOT NULL,
            recursive INTEGER NOT NULL DEFAULT 0,
            content TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS acl_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            connection_name TEXT NOT NULL,
            path TEXT NOT NULL,
            content TEXT NOT NULL,
            cached_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(connection_name, path)
        );

        CREATE TABLE IF NOT EXISTS known_hosts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            host TEXT NOT NULL,
            port INTEGER NOT NULL DEFAULT 22,
            fingerprint TEXT NOT NULL,
            UNIQUE(host, port)
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn migrate_schema(conn: &Connection) -> Result<()> {
    let _ = conn.execute(
        "ALTER TABLE audit_logs ADD COLUMN snapshot_ids TEXT DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE connections ADD COLUMN auth_method TEXT DEFAULT 'password'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE connections ADD COLUMN key_path TEXT",
        [],
    );
    Ok(())
}

pub fn insert_connection(
    conn: &Connection,
    name: &str,
    host: &str,
    port: u16,
    username: &str,
    use_sudo: bool,
    auth_method: &str,
    key_path: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT OR REPLACE INTO connections (name, host, port, username, use_sudo, auth_method, key_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, host, port, username, use_sudo as i32, auth_method, key_path],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_connections(conn: &Connection) -> Result<Vec<crate::models::Connection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, use_sudo, auth_method, key_path FROM connections ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::Connection {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            use_sudo: row.get::<_, i32>(5)? != 0,
            has_password: false,
            auth_method: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "password".to_string()),
            key_path: row.get(7)?,
        })
    })?;

    let mut connections = Vec::new();
    for row in rows {
        let mut c = row?;
        c.has_password = has_password(&c.name);
        connections.push(c);
    }
    Ok(connections)
}

pub fn get_connection(conn: &Connection, id: i64) -> Result<crate::models::Connection> {
    let c = conn.query_row(
        "SELECT id, name, host, port, username, use_sudo, auth_method, key_path FROM connections WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(crate::models::Connection {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                use_sudo: row.get::<_, i32>(5)? != 0,
                has_password: false,
                auth_method: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "password".to_string()),
                key_path: row.get(7)?,
            })
        },
    )?;
    Ok(c)
}

pub fn delete_connection(conn: &Connection, id: i64) -> Result<()> {
    let name = conn.query_row::<String, _, _>(
        "SELECT name FROM connections WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )?;

    conn.execute("DELETE FROM connections WHERE id = ?1", rusqlite::params![id])?;

    if let Ok(entry) = keyring::Entry::new("syno-perm-manager", &name) {
        let _ = entry.delete_credential();
    }

    Ok(())
}

pub fn insert_audit_log(
    conn: &Connection,
    connection_name: &str,
    action: &str,
    path: &str,
    details: &str,
    success: bool,
    snapshot_ids: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_logs (connection_name, action, path, details, success, snapshot_ids) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![connection_name, action, path, details, success as i32, snapshot_ids],
    )?;
    Ok(())
}

pub fn list_audit_logs(conn: &Connection, limit: i64) -> Result<Vec<crate::models::AuditLog>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, connection_name, action, path, details, success, snapshot_ids FROM audit_logs ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| {
        Ok(crate::models::AuditLog {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            connection_name: row.get(2)?,
            action: row.get(3)?,
            path: row.get(4)?,
            details: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            success: row.get::<_, i32>(6)? != 0,
            snapshot_ids: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        })
    })?;

    let mut logs = Vec::new();
    for row in rows {
        logs.push(row?);
    }
    Ok(logs)
}

pub fn insert_snapshot(
    conn: &Connection,
    connection_name: &str,
    path: &str,
    recursive: bool,
    content: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO acl_snapshots (connection_name, path, recursive, content) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![connection_name, path, recursive as i32, content],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_snapshot_by_id(conn: &Connection, id: i64) -> Result<crate::models::SnapshotData> {
    let content: String = conn.query_row(
        "SELECT content FROM acl_snapshots WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )?;
    let data: crate::models::SnapshotData = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn list_snapshots(conn: &Connection, limit: i64) -> Result<Vec<crate::models::SnapshotInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, connection_name, path, recursive FROM acl_snapshots ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| {
        Ok(crate::models::SnapshotInfo {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            connection_name: row.get(2)?,
            path: row.get(3)?,
            recursive: row.get::<_, i32>(4)? != 0,
        })
    })?;

    let mut snapshots = Vec::new();
    for row in rows {
        snapshots.push(row?);
    }
    Ok(snapshots)
}

fn has_password(connection_name: &str) -> bool {
    match keyring::Entry::new("syno-perm-manager", connection_name) {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    }
}

pub fn save_password(connection_name: &str, password: &str) -> Result<()> {
    let entry = keyring::Entry::new("syno-perm-manager", connection_name)
        .map_err(|e| anyhow::anyhow!("Keyring: {}", e))?;
    entry
        .set_password(password)
        .map_err(|e| anyhow::anyhow!("Keyring: {}", e))?;
    Ok(())
}

pub fn get_password(connection_name: &str) -> Result<String> {
    let entry = keyring::Entry::new("syno-perm-manager", connection_name)
        .map_err(|e| anyhow::anyhow!("Keyring: {}", e))?;
    entry
        .get_password()
        .map_err(|e| anyhow::anyhow!("Keyring: {}", e))
}

pub fn cache_acl(
    conn: &Connection,
    connection_name: &str,
    path: &str,
    content: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO acl_cache (connection_name, path, content, cached_at) VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![connection_name, path, content],
    )?;
    Ok(())
}

pub fn get_cached_acl(conn: &Connection, connection_name: &str, path: &str) -> Result<Option<String>> {
    let result = conn.query_row::<String, _, _>(
        "SELECT content FROM acl_cache WHERE connection_name = ?1 AND path = ?2",
        rusqlite::params![connection_name, path],
        |row| row.get(0),
    );
    match result {
        Ok(content) => Ok(Some(content)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("DB: {}", e)),
    }
}

pub fn clear_acl_cache(conn: &Connection, connection_name: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM acl_cache WHERE connection_name = ?1",
        rusqlite::params![connection_name],
    )?;
    Ok(())
}

pub fn get_known_host(conn: &Connection, host: &str, port: u16) -> Result<Option<String>> {
    let result = conn.query_row::<String, _, _>(
        "SELECT fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2",
        rusqlite::params![host, port],
        |row| row.get(0),
    );
    match result {
        Ok(fp) => Ok(Some(fp)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("DB: {}", e)),
    }
}

pub fn save_known_host(conn: &Connection, host: &str, port: u16, fingerprint: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO known_hosts (host, port, fingerprint) VALUES (?1, ?2, ?3)",
        rusqlite::params![host, port, fingerprint],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn.query_row::<String, _, _>(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![key],
        |row| row.get(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("DB: {}", e)),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )?;
    Ok(())
}
