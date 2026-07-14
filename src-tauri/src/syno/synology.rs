use crate::models::{AclDiff, AclEntry, ApplyResult, DirNode, PathSnapshot, ShareFolder, SnapshotData};
use crate::ssh::client::{CommandResult, SshClient};
use crate::syno::parser::{
    build_acl_entry_string, parse_acl_output, parse_dir_list, parse_group_list,
    parse_share_list, parse_user_list,
};
use crate::syno::utils::{extract_error, shell_escape, FIND, SYNOACLTOOL, SYNOGROUP, SYNOUSER, SYNOSHARE};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

pub struct SynologyProvider;

impl SynologyProvider {
    pub fn new() -> Self {
        Self
    }

    fn check_result(res: &CommandResult, context: &str) -> Result<()> {
        if res.exit_code != 0 {
            let msg = extract_error(res);
            if msg.is_empty() {
                Ok(())
            } else {
                Err(anyhow!("{}: {}", context, msg))
            }
        } else {
            Ok(())
        }
    }

    async fn find_dirs(&self, ssh: &mut SshClient, path: &str) -> Result<Vec<String>> {
        let cmd = format!("{} {} -type d 2>/dev/null", FIND, shell_escape(path));
        let res = ssh.exec(&cmd).await?;
        let mut dirs = Vec::new();
        for line in res.stdout.lines() {
            let p = line.trim();
            if p.is_empty() {
                continue;
            }
            let name = p.rsplit('/').next().unwrap_or(p);
            if name == "@eaDir" || name == "#recycle" || name.starts_with('@') {
                continue;
            }
            dirs.push(p.to_string());
        }
        Ok(dirs)
    }
}

#[async_trait]
impl crate::syno::provider::AclProvider for SynologyProvider {
    async fn list_shares(&self, ssh: &mut SshClient) -> Result<Vec<ShareFolder>> {
        let res = ssh.exec(&format!("{} --enum ALL", SYNOSHARE)).await?;
        Self::check_result(&res, "list_shares")?;
        let shares = parse_share_list(&res.stdout);
        Ok(shares)
    }

    async fn list_users(&self, ssh: &mut SshClient) -> Result<Vec<String>> {
        let res = ssh.exec(&format!("{} --enum local", SYNOUSER)).await?;
        Self::check_result(&res, "list_users")?;
        Ok(parse_user_list(&res.stdout))
    }

    async fn list_groups(&self, ssh: &mut SshClient) -> Result<Vec<String>> {
        let res = ssh.exec(&format!("{} --enum", SYNOGROUP)).await?;
        Self::check_result(&res, "list_groups")?;
        Ok(parse_group_list(&res.stdout))
    }

    async fn list_dirs(&self, ssh: &mut SshClient, path: &str) -> Result<Vec<DirNode>> {
        let cmd = format!(
            "{} {} -mindepth 1 -maxdepth 1 -type d 2>/dev/null",
            FIND, shell_escape(path)
        );
        let res = ssh.exec(&cmd).await?;
        Self::check_result(&res, "list_dirs")?;
        Ok(parse_dir_list(&res.stdout))
    }

    async fn get_acl(&self, ssh: &mut SshClient, path: &str) -> Result<Vec<AclEntry>> {
        let cmd = format!("{} -get {}", SYNOACLTOOL, shell_escape(path));
        let res = ssh.exec(&cmd).await?;
        if res.exit_code != 0 {
            let msg = extract_error(&res);
            if msg.is_empty() {
                return Ok(Vec::new());
            }
            return Err(anyhow!("get_acl: {}", msg));
        }
        Ok(parse_acl_output(&res.stdout))
    }

    async fn dry_run(
        &self,
        ssh: &mut SshClient,
        paths: &[String],
        entries: &[AclEntry],
        recursive: bool,
    ) -> Result<Vec<AclDiff>> {
        let mut all_paths = Vec::new();

        for path in paths {
            if recursive {
                all_paths.extend(self.find_dirs(ssh, path).await?);
            } else {
                all_paths.push(path.clone());
            }
        }

        let mut diffs = Vec::new();

        for path in &all_paths {
            let current = self.get_acl(ssh, path).await.unwrap_or_default();

            for entry in entries {
                let key = format!("{}:{}", entry.principal_type, entry.name);
                let existing = current
                    .iter()
                    .find(|e| format!("{}:{}", e.principal_type, e.name) == key);

                let action = if existing.is_none() {
                    "add"
                } else {
                    let ex = existing.unwrap();
                    if ex.permissions != entry.permissions || ex.flags != entry.flags {
                        "modify"
                    } else {
                        continue;
                    }
                };

                diffs.push(AclDiff {
                    path: path.clone(),
                    principal_type: entry.principal_type.clone(),
                    name: entry.name.clone(),
                    old_permissions: existing
                        .map(|e| format!("{}:{}", e.permissions, e.flags)),
                    new_permissions: Some(format!("{}:{}", entry.permissions, entry.flags)),
                    action: action.to_string(),
                });
            }
        }

        Ok(diffs)
    }

    async fn apply_acl(
        &self,
        ssh: &mut SshClient,
        paths: &[String],
        entries: &[AclEntry],
        recursive: bool,
    ) -> Result<ApplyResult> {
        let mut all_paths = Vec::new();

        for path in paths {
            if recursive {
                all_paths.extend(self.find_dirs(ssh, path).await?);
            } else {
                all_paths.push(path.clone());
            }
        }

        let mut modified = 0usize;
        let mut errors = Vec::new();

        for path in &all_paths {
            let mut path_ok = true;

            for entry in entries {
                let entry_str = build_acl_entry_string(entry);
                let cmd = format!("{} -add {} {}", SYNOACLTOOL, shell_escape(path), entry_str);
                match ssh.exec(&cmd).await {
                    Ok(res) => {
                        if res.exit_code != 0 {
                            path_ok = false;
                            errors.push(format!("{} (add {}): {}", path, entry.name, extract_error(&res)));
                        }
                    }
                    Err(e) => {
                        path_ok = false;
                        errors.push(format!("{} (add {}): {}", path, entry.name, e));
                    }
                }
            }

            if path_ok {
                modified += 1;
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            paths_modified: modified,
            errors,
        })
    }

    async fn snapshot_acl(
        &self,
        ssh: &mut SshClient,
        path: &str,
        recursive: bool,
    ) -> Result<String> {
        let mut snapshots = Vec::new();

        if recursive {
            let dirs = self.find_dirs(ssh, path).await?;
            for dir in &dirs {
                let entries = self.get_acl(ssh, dir).await.unwrap_or_default();
                snapshots.push(PathSnapshot {
                    path: dir.clone(),
                    entries,
                });
            }
        } else {
            let entries = self.get_acl(ssh, path).await?;
            snapshots.push(PathSnapshot {
                path: path.to_string(),
                entries,
            });
        }

        let data = SnapshotData { snapshots };
        Ok(serde_json::to_string(&data)?)
    }

    async fn restore_acl(
        &self,
        ssh: &mut SshClient,
        snapshot_content: &str,
    ) -> Result<ApplyResult> {
        let data: SnapshotData = serde_json::from_str(snapshot_content)?;

        let mut modified = 0usize;
        let mut errors = Vec::new();

        for snap in &data.snapshots {
            let valid_entries: Vec<&AclEntry> = snap
                .entries
                .iter()
                .filter(|e| !e.name.is_empty() && !e.permissions.is_empty())
                .collect();

            // Paso 1: borrar todas las ACLs actuales
            let del_cmd = format!("{} -del {}", SYNOACLTOOL, shell_escape(&snap.path));
            let del_res = ssh.exec(&del_cmd).await;
            if let Err(e) = &del_res {
                errors.push(format!("{} (del): {}", snap.path, e));
            }

            // Paso 2: añadir las entradas del snapshot una a una
            if valid_entries.is_empty() {
                modified += 1;
                continue;
            }

            let mut all_ok = true;
            for entry in &valid_entries {
                let entry_str = build_acl_entry_string(entry);
                let add_cmd = format!("{} -add {} {}", SYNOACLTOOL, shell_escape(&snap.path), entry_str);
                match ssh.exec(&add_cmd).await {
                    Ok(res) => {
                        if res.exit_code != 0 {
                            all_ok = false;
                            errors.push(format!("{} (add {}): {}", snap.path, entry.name, extract_error(&res)));
                        }
                    }
                    Err(e) => {
                        all_ok = false;
                        errors.push(format!("{} (add {}): {}", snap.path, entry.name, e));
                    }
                }
            }

            if all_ok {
                modified += 1;
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            paths_modified: modified,
            errors,
        })
    }
}
