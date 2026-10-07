//! Server-owned, single-use permission plans. Never accept shell commands from IPC.
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};
use crate::{commands::AppState, db, ssh::client::SshClient};
use crate::syno::{shell_escape, extract_error, parser, SYNOUSER, SYNOGROUP};
use crate::syno::utils::SYNOACLTOOL;

static NEXT_PLAN: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompareRequest {
    #[serde(default)]
    scan_id: String,
    source: String,
    target: String,
    paths: Vec<String>,
    recursive: bool,
    groups: bool,
    mode: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Change {
    pub scope: String,
    pub action: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FolderComparison {
    path: String,
    source: Vec<PermissionInfo>,
    target: Vec<PermissionInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PermissionInfo {
    principal: String,
    origin: String,
    access: String,
    permissions: String,
    flags: String,
    level: usize,
}

fn permission_info(ace: &Ace) -> PermissionInfo {
    PermissionInfo {
        principal: ace.name.clone(),
        origin: if ace.kind == "group" { "group" } else if ace.kind == "everyone" { "everyone" } else if ace.level > 0 { "inherited" } else { "manual" }.into(),
        access: ace.access.clone(), permissions: ace.permissions.clone(), flags: ace.flags.clone(), level: ace.level,
    }
}

#[tauri::command]
pub fn cancel_comparison(scan_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut current = state.comparison_scan.lock().map_err(|e| e.to_string())?;
    if current.as_ref() == Some(&scan_id) { *current = None; }
    Ok(())
}

async fn read_scan(ssh: &mut SshClient, cmd: &str, state: &AppState, id: &str) -> Result<String, String> {
    let check = || -> Result<(), String> {
        if state.comparison_scan.lock().map_err(|e| e.to_string())?.as_deref() != Some(id) { return Err("Análisis cancelado".into()); }
        Ok(())
    };
    check()?;
    let result = read_limited(ssh, cmd, 90).await?;
    check()?;
    Ok(result)
}

#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    id: u64,
    source_groups: Vec<String>,
    target_groups: Vec<String>,
    folders: Vec<FolderComparison>,
    changes: Vec<Change>,
}

#[derive(Clone, Debug, Serialize)]
struct Resource {
    read: String,
    before: String,
    commands: Vec<String>,
}

pub struct StoredPlan {
    id: u64,
    connection: String,
    request: CompareRequest,
    resources: Vec<Resource>,
    created: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ace {
    index: usize,
    kind: String,
    name: String,
    access: String,
    permissions: String,
    flags: String,
    level: usize,
}

impl Ace {
    fn value(&self, name: &str) -> String {
        format!("{}:{}:{}:{}:{}", self.kind, name, self.access, self.permissions, self.flags)
    }
    fn signature(&self) -> (&str, &str, &str) {
        (&self.access, &self.permissions, &self.flags)
    }
}

// Deleting by index requires the actual NAS index and inheritance level.
fn parse_aces(raw: &str) -> Result<Vec<Ace>, String> {
    let mut result = Vec::new();
    for line in raw.lines().map(str::trim).filter(|l| l.starts_with('[')) {
        let (index, rest) = line.split_once(']').ok_or("ACL sin índice válido")?;
        let index = index[1..].parse().map_err(|_| "Índice ACL inválido")?;
        let (value, level) = rest.trim().rsplit_once("(level:").ok_or("ACL sin nivel de herencia; operación bloqueada")?;
        let level = level.trim().trim_end_matches(')').parse().map_err(|_| "Nivel ACL inválido")?;
        let p: Vec<_> = value.trim().split(':').collect();
        if p.len() != 5 { return Err("Formato ACL no compatible".into()); }
        result.push(Ace { index, kind: p[0].into(), name: p[1].into(), access: p[2].into(), permissions: p[3].into(), flags: p[4].into(), level });
    }
    if result.is_empty() && !raw.contains("ACL version:") {
        return Err("La carpeta no devuelve una ACL Synology reconocible".into());
    }
    Ok(result)
}

fn acl_changes(aces: &[Ace], source: &str, target: &str, mode: &str) -> (Vec<Ace>, Vec<Ace>) {
    let src: Vec<_> = aces.iter().filter(|a| a.kind == "user" && a.name == source && a.level == 0).cloned().collect();
    let dst: Vec<_> = aces.iter().filter(|a| a.kind == "user" && a.name == target && a.level == 0).cloned().collect();
    let mut desired = if mode == "replace" { Vec::new() } else { dst.clone() };
    if mode == "update" {
        desired.retain(|d| !src.iter().any(|s| s.access == d.access && s.flags == d.flags));
    }
    for ace in src {
        if !desired.iter().any(|d| d.signature() == ace.signature()) { desired.push(ace); }
    }
    let mut matched = vec![false; desired.len()];
    let mut remove = Vec::new();
    for entry in &dst {
        if let Some(i) = desired.iter().enumerate().position(|(i, d)| !matched[i] && d.signature() == entry.signature()) {
            matched[i] = true;
        } else { remove.push(entry.clone()); }
    }
    let add = desired.into_iter().enumerate().filter(|(i, _)| !matched[*i]).map(|(_, a)| a).collect();
    (remove, add)
}

async fn read(ssh: &mut SshClient, cmd: &str) -> Result<String, String> {
    let result = ssh.exec(cmd).await.map_err(|e| e.to_string())?;
    if result.exit_code != 0 { return Err(format!("Comando fallido ({}): {}", result.exit_code, extract_error(&result))); }
    Ok(result.stdout)
}

async fn read_limited(ssh: &mut SshClient, cmd: &str, seconds: u64) -> Result<String, String> {
    let result = ssh.exec_with_timeout(cmd, seconds).await.map_err(|e| e.to_string())?;
    if result.exit_code != 0 { return Err(format!("Comando fallido ({}): {}", result.exit_code, extract_error(&result))); }
    Ok(result.stdout)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('-') && !name.contains(|c: char| c.is_control() || c == ':')
}

fn declared_count(raw: &str, header: &str) -> Option<usize> {
    raw.lines().map(str::trim).find(|line| line.starts_with(header))?
        .split_once(':')?.1.trim().parse().ok()
}

#[tauri::command]
pub async fn compare_users(request: CompareRequest, state: tauri::State<'_, AppState>) -> Result<Preview, String> {
    if !valid_name(&request.source) || !valid_name(&request.target) || request.source == request.target {
        return Err("Selecciona dos usuarios distintos y válidos".into());
    }
    if !["merge", "update", "replace"].contains(&request.mode.as_str()) { return Err("Modo inválido".into()); }
    if request.paths.len() > 100 || (request.paths.is_empty() && !request.groups) { return Err("Selecciona grupos o carpetas (máximo 100 raíces)".into()); }
    *state.comparison_scan.lock().map_err(|e| e.to_string())? = Some(request.scan_id.clone());
    let mut guard = state.ssh.client.lock().await;
    let ssh = guard.as_mut().ok_or("No hay conexión activa")?;
    *state.comparison.lock().map_err(|e| e.to_string())? = None;
    let connection = state.ssh.connection_name.lock().await.clone();
    let mut resources = Vec::new();
    let mut users = Vec::new();
    for name in [&request.source, &request.target] {
        let cmd = format!("{} --get {}", SYNOUSER, shell_escape(name));
        let raw = read_scan(ssh, &cmd, &state, &request.scan_id).await?;
        let user = parser::parse_user_detail(&raw);
        if user.name != *name || user.primary_gid.is_empty() { return Err(format!("No se pudo identificar a {name}")); }
        if declared_count(&raw, "Member Of") != Some(user.member_of.len()) {
            return Err(format!("La lista de grupos de {name} está incompleta o usa un formato no compatible"));
        }
        users.push(user);
        resources.push(Resource { read: cmd, before: raw, commands: Vec::new() });
    }
    let mut changes = Vec::new();
    if request.groups {
        let groups: BTreeSet<_> = users.iter().flat_map(|u| u.member_of.iter().cloned()).collect();
        for group in groups {
            let in_source = users[0].member_of.contains(&group);
            let in_target = users[1].member_of.contains(&group);
            if in_source == in_target || (!in_source && request.mode != "replace") { continue; }
            let cmd = format!("{} --get {}", SYNOGROUP, shell_escape(&group));
            let raw = read_scan(ssh, &cmd, &state, &request.scan_id).await?;
            let detail = parser::parse_group_detail(&raw);
            if detail.name != group || detail.members.contains(&request.target) != in_target {
                return Err(format!("No se pudo verificar la membresía de {group}"));
            }
            if !in_source && declared_count(&raw, "Group Members") != Some(detail.members.len()) {
                return Err(format!("Lista de miembros incompleta para {group}; retirada bloqueada"));
            }
            if !in_source && detail.gid == users[1].primary_gid {
                return Err(format!("No se puede retirar el grupo primario {group}. Usa Añadir/Actualizar."));
            }
            let mut members = detail.members;
            members.retain(|m| m != &request.target);
            if in_source { members.push(request.target.clone()); }
            let command = if in_source {
                format!("{} --memberadd {} {}", SYNOGROUP, shell_escape(&group), shell_escape(&request.target))
            } else {
                format!("{} --member {} {}", SYNOGROUP, shell_escape(&group), members.iter().map(|m| shell_escape(m)).collect::<Vec<_>>().join(" "))
            };
            changes.push(Change { scope: format!("Grupo: {group}"), action: if in_source { "Añadir" } else { "Retirar" }.into(), before: in_target.to_string(), after: in_source.to_string() });
            resources.push(Resource { read: cmd, before: raw, commands: vec![command] });
        }
    }
    let mut paths = BTreeSet::new();
    for root in &request.paths {
        if !root.starts_with('/') || root.contains(|c: char| c.is_control()) || root.split('/').any(|p| p == "..") {
            return Err("Usa rutas absolutas sin '..' ni caracteres de control".into());
        }
        let canonical = read_scan(ssh, &format!("readlink -f {}", shell_escape(root)), &state, &request.scan_id).await?;
        let canonical = canonical.trim_end_matches('\n');
        if !canonical.starts_with('/') || canonical.contains(|c: char| c.is_control()) { return Err(format!("Ruta no compatible: {root}")); }
        let depth = if request.recursive { "" } else { "-maxdepth 0" };
        let output = read_scan(ssh, &format!("find {} {} -type d \\( {} \\) -prune -o -type d -print0", shell_escape(canonical), depth, ssh.exclusion_expression()), &state, &request.scan_id).await?;
        if output.is_empty() { return Err(format!("No es una carpeta analizable: {root}")); }
        for path in output.split('\0').filter(|p| !p.is_empty()) { paths.insert(path.to_string()); }
        if paths.len() > 2500 { return Err("El ámbito supera 2500 carpetas. Selecciona raíces más específicas para evitar bloquear la sesión SSH.".into()); }
    }
    let mut folders = Vec::new();
    // Children first: adding an inheritable ACE on a parent can renumber child ACEs.
    let mut paths: Vec<_> = paths.into_iter().collect();
    paths.sort_by_key(|p| std::cmp::Reverse(p.split('/').count()));
    for path in paths {
        let cmd = format!("{} -get {}", SYNOACLTOOL, shell_escape(&path));
        let raw = read_scan(ssh, &cmd, &state, &request.scan_id).await?;
        let aces = parse_aces(&raw).map_err(|e| format!("{path}: {e}"))?;
        let display = |i: usize| aces.iter().filter(|a| (a.kind == "user" && a.name == users[i].name) || (a.kind == "group" && users[i].member_of.contains(&a.name)) || a.kind == "everyone").map(permission_info).collect();
        folders.push(FolderComparison { path: path.clone(), source: display(0), target: display(1) });
        let (mut remove, add) = acl_changes(&aces, &request.source, &request.target, &request.mode);
        remove.sort_by_key(|a| std::cmp::Reverse(a.index));
        let mut commands = Vec::new();
        for ace in remove {
            commands.push(format!("{} -del {} {}", SYNOACLTOOL, shell_escape(&path), ace.index));
            changes.push(Change { scope: path.clone(), action: "Retirar ACL".into(), before: ace.value(&request.target), after: String::new() });
        }
        for ace in add {
            let value = ace.value(&request.target);
            commands.push(format!("{} -add {} {}", SYNOACLTOOL, shell_escape(&path), shell_escape(&value)));
            changes.push(Change { scope: path.clone(), action: "Añadir ACL".into(), before: String::new(), after: value });
        }
        resources.push(Resource { read: cmd, before: raw, commands });
    }
    let id = NEXT_PLAN.fetch_add(1, Ordering::Relaxed);
    let preview = Preview { id, source_groups: users[0].member_of.clone(), target_groups: users[1].member_of.clone(), folders, changes };
    *state.comparison.lock().map_err(|e| e.to_string())? = Some(StoredPlan { id, connection, request, resources, created: std::time::Instant::now() });
    Ok(preview)
}

#[derive(Serialize)]
pub struct SyncResult {
    completed: usize,
    total: usize,
    errors: Vec<String>,
}

#[tauri::command]
pub async fn apply_user_comparison(id: u64, state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let mut guard = state.ssh.client.lock().await;
    let ssh = guard.as_mut().ok_or("No hay conexión activa")?;
    let plan = state.comparison.lock().map_err(|e| e.to_string())?.take().ok_or("Genera una nueva comparación")?;
    let connection = state.ssh.connection_name.lock().await.clone();
    if id != plan.id || connection != plan.connection || plan.created.elapsed().as_secs() > 900 {
        return Err("La vista previa ha caducado. Vuelve a comparar.".into());
    }
    for resource in &plan.resources {
        if read(ssh, &resource.read).await? != resource.before { return Err("Los permisos o grupos han cambiado desde la comparación. Vuelve a comparar.".into()); }
    }
    let backup = serde_json::to_string(&(&plan.request, &plan.resources)).map_err(|e| e.to_string())?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_audit_log(&conn, &connection, "user_sync_backup", &plan.request.target, &backup, true, "").map_err(|e| e.to_string())?;
        db::clear_acl_cache(&conn, &connection).map_err(|e| e.to_string())?;
    }
    let total = plan.resources.iter().map(|r| r.commands.len()).sum();
    let mut result = SyncResult { completed: 0, total, errors: Vec::new() };
    'apply: for resource in &plan.resources {
        for command in &resource.commands {
            match read(ssh, command).await {
                Ok(_) => result.completed += 1,
                Err(e) => { result.errors.push(e); break 'apply; }
            }
        }
    }
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let details = serde_json::to_string(&result).map_err(|e| e.to_string())?;
        db::insert_audit_log(&conn, &connection, "user_sync", &plan.request.target, &details, result.errors.is_empty(), "").map_err(|e| e.to_string())?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn aces() -> Vec<Ace> {
        parse_aces("ACL version: 1\n[0] user:source:allow:rwx:fd-- (level:0)\n[1] user:target:allow:r--:fd-- (level:0)\n[2] user:target:deny:-w-:---- (level:0)\n[3] user:target:allow:r--:fd-- (level:1)\n[4] group:staff:allow:rwx:fd-- (level:0)").unwrap()
    }
    #[test]
    fn modes_preserve_inheritance_and_other_principals() {
        let a = aces();
        let (remove, add) = acl_changes(&a, "source", "target", "merge");
        assert!(remove.is_empty()); assert_eq!(add.len(), 1);
        let (remove, add) = acl_changes(&a, "source", "target", "update");
        assert_eq!(remove.iter().map(|a| a.index).collect::<Vec<_>>(), vec![1]); assert_eq!(add.len(), 1);
        let (remove, _) = acl_changes(&a, "source", "target", "replace");
        assert_eq!(remove.iter().map(|a| a.index).collect::<Vec<_>>(), vec![1, 2]);
    }
    #[test]
    fn no_source_replace_removes_only_direct_target() {
        let (remove, add) = acl_changes(&aces(), "absent", "target", "replace");
        assert_eq!(remove.len(), 2); assert!(add.is_empty());
    }
    #[test]
    fn refuses_unknown_inheritance() {
        assert!(parse_aces("[0] user:a:allow:rwx:fd--").is_err());
        assert!(parse_aces("permission denied").is_err());
        assert_eq!(declared_count("Member Of           : 2\n(0) users\n(1) staff", "Member Of"), Some(2));
        assert_eq!(declared_count("Group Members: 3\n0:[a]", "Group Members"), Some(3));
        assert_eq!(declared_count("Group Members: unknown", "Group Members"), None);
    }
    #[test]
    fn identical_permissions_are_idempotent() {
        let a = parse_aces("[0] user:source:allow:rwx:fd-- (level:0)\n[1] user:target:allow:rwx:fd-- (level:0)").unwrap();
        for mode in ["merge", "update", "replace"] { let (r, a) = acl_changes(&a, "source", "target", mode); assert!(r.is_empty() && a.is_empty()); }
    }
    #[test]
    fn replacement_removes_duplicate_target_entries() {
        let a = parse_aces("[0] user:source:allow:rwx:fd-- (level:0)\n[1] user:target:allow:rwx:fd-- (level:0)\n[2] user:target:allow:rwx:fd-- (level:0)").unwrap();
        let (remove, add) = acl_changes(&a, "source", "target", "replace");
        assert_eq!(remove.len(), 1); assert!(add.is_empty());
    }
    #[test]
    fn inherited_source_is_never_materialized_as_direct() {
        let a = parse_aces("[0] user:source:allow:rwx:fd-- (level:1)").unwrap();
        let (remove, add) = acl_changes(&a, "source", "target", "merge");
        assert!(remove.is_empty() && add.is_empty());
    }
}
