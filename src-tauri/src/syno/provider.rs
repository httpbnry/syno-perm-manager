use crate::models::{AclDiff, AclEntry, DirNode, ShareFolder};
use crate::ssh::client::SshClient;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait AclProvider: Send + Sync {
    async fn list_shares(&self, ssh: &mut SshClient) -> Result<Vec<ShareFolder>>;
    async fn list_users(&self, ssh: &mut SshClient) -> Result<Vec<String>>;
    async fn list_groups(&self, ssh: &mut SshClient) -> Result<Vec<String>>;
    async fn list_dirs(&self, ssh: &mut SshClient, path: &str) -> Result<Vec<DirNode>>;
    async fn get_acl(&self, ssh: &mut SshClient, path: &str) -> Result<Vec<AclEntry>>;
    async fn dry_run(
        &self,
        ssh: &mut SshClient,
        paths: &[String],
        entries: &[AclEntry],
        recursive: bool,
    ) -> Result<Vec<AclDiff>>;
    async fn apply_acl(
        &self,
        ssh: &mut SshClient,
        paths: &[String],
        entries: &[AclEntry],
        recursive: bool,
    ) -> Result<crate::models::ApplyResult>;
    async fn snapshot_acl(
        &self,
        ssh: &mut SshClient,
        path: &str,
        recursive: bool,
    ) -> Result<String>;
    async fn restore_acl(
        &self,
        ssh: &mut SshClient,
        snapshot_content: &str,
    ) -> Result<crate::models::ApplyResult>;
}
