use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: Option<i64>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub use_sudo: bool,
    pub has_password: bool,
    pub auth_method: String,
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_sudo: bool,
    pub auth_method: String,
    pub key_path: Option<String>,
    pub key_passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareFolder {
    pub name: String,
    pub path: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirNode {
    pub name: String,
    pub path: String,
    pub has_children: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    pub principal_type: String,
    pub name: String,
    pub permissions: String,
    pub flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclDiff {
    pub path: String,
    pub principal_type: String,
    pub name: String,
    pub old_permissions: Option<String>,
    pub new_permissions: Option<String>,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: i64,
    pub timestamp: String,
    pub connection_name: String,
    pub action: String,
    pub path: String,
    pub details: String,
    pub success: bool,
    pub snapshot_ids: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub success: bool,
    pub paths_modified: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRequest {
    pub paths: Vec<String>,
    pub entries: Vec<AclEntry>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSnapshot {
    pub path: String,
    pub entries: Vec<AclEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    pub snapshots: Vec<PathSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub id: i64,
    pub timestamp: String,
    pub connection_name: String,
    pub path: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathAcl {
    pub path: String,
    pub entries: Vec<AclEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub user_count: usize,
    pub group_count: usize,
    pub share_count: usize,
    pub recent_logs: Vec<AuditLog>,
    pub total_changes: i64,
    pub successful_changes: i64,
    pub failed_changes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAuditStats {
    pub recent_logs: Vec<AuditLog>,
    pub total_changes: i64,
    pub successful_changes: i64,
    pub failed_changes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermMatrixCell {
    pub path: String,
    pub color: String,
    pub permissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermMatrixRow {
    pub principal: String,
    pub principal_type: String,
    pub cells: Vec<PermMatrixCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermMatrix {
    pub paths: Vec<String>,
    pub rows: Vec<PermMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupResult {
    pub ran: bool,
    pub output: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDetail {
    pub name: String,
    pub uid: String,
    pub primary_gid: String,
    pub full_name: String,
    pub user_dir: String,
    pub shell: String,
    pub expired: bool,
    pub mail: String,
    pub member_of: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDetail {
    pub name: String,
    pub gid: String,
    pub group_type: String,
    pub description: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserInput {
    pub username: String,
    pub password: String,
    pub full_name: String,
    pub expired: bool,
    pub mail: String,
    pub privilege: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupInput {
    pub name: String,
    pub members: Vec<String>,
}
