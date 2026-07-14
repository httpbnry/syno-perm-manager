use std::sync::Mutex;

pub struct AppConfig {
    pub volume_path: String,
    pub synoacltool_path: String,
    pub synoshare_path: String,
    pub synouser_path: String,
    pub synogroup_path: String,
    pub find_path: String,
    pub excluded_folders: Vec<String>,
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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            volume_path: "/volume1".to_string(),
            synoacltool_path: "/usr/syno/bin/synoacltool".to_string(),
            synoshare_path: "/usr/syno/sbin/synoshare".to_string(),
            synouser_path: "/usr/syno/sbin/synouser".to_string(),
            synogroup_path: "/usr/syno/sbin/synogroup".to_string(),
            find_path: "/bin/find".to_string(),
            excluded_folders: vec!["@eaDir".to_string(), "#recycle".to_string()],
            ssh_timeout_secs: 600,
            keepalive_secs: 15,
            log_retention_days: 90,
            snapshot_retention_count: 100,
            password_length: 12,
            password_special_chars: true,
            username_format: "{first_initial}{last_name}".to_string(),
            description_template: "{full_name} Alta {date} {password}".to_string(),
            theme: "dark".to_string(),
            language: "es".to_string(),
        }
    }
}

pub fn load_config(conn: &rusqlite::Connection) -> AppConfig {
    let mut config = AppConfig::default();

    fn get_str(conn: &rusqlite::Connection, key: &str, default: &str) -> String {
        match conn.query_row::<String, _, _>(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        ) {
            Ok(v) if !v.is_empty() => v,
            _ => default.to_string(),
        }
    }

    fn get_int(conn: &rusqlite::Connection, key: &str, default: i64) -> i64 {
        get_str(conn, key, &default.to_string())
            .parse()
            .unwrap_or(default)
    }

    config.volume_path = get_str(conn, "volume_path", &config.volume_path);
    config.synoacltool_path = get_str(conn, "synoacltool_path", &config.synoacltool_path);
    config.synoshare_path = get_str(conn, "synoshare_path", &config.synoshare_path);
    config.synouser_path = get_str(conn, "synouser_path", &config.synouser_path);
    config.synogroup_path = get_str(conn, "synogroup_path", &config.synogroup_path);
    config.find_path = get_str(conn, "find_path", &config.find_path);
    config.ssh_timeout_secs = get_int(conn, "ssh_timeout_secs", config.ssh_timeout_secs as i64) as u64;
    config.keepalive_secs = get_int(conn, "keepalive_secs", config.keepalive_secs as i64) as u64;
    config.log_retention_days = get_int(conn, "log_retention_days", config.log_retention_days);
    config.snapshot_retention_count = get_int(conn, "snapshot_retention_count", config.snapshot_retention_count);
    config.password_length = get_int(conn, "password_length", config.password_length as i64) as usize;
    config.password_special_chars = get_str(conn, "password_special_chars", "true") == "true";
    config.username_format = get_str(conn, "username_format", &config.username_format);
    config.description_template = get_str(conn, "description_template", &config.description_template);
    config.theme = get_str(conn, "theme", &config.theme);
    config.language = get_str(conn, "language", &config.language);

    let excluded = get_str(conn, "excluded_folders", &config.excluded_folders.join(","));
    config.excluded_folders = excluded
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    config
}

pub type ConfigState = Mutex<AppConfig>;
