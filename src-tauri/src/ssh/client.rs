use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use russh::client::{self, Handle};
use russh::keys::{ssh_key, load_secret_key, PrivateKeyWithHashAlg};
use russh::{ChannelMsg, Disconnect};

struct ClientHandler {
    known_host_key: Option<String>,
    received_fingerprint: Arc<Mutex<Option<String>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let key_fp = server_public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();
        *self.received_fingerprint.lock().await = Some(key_fp.clone());
        match &self.known_host_key {
            Some(known) => Ok(key_fp == *known),
            None => Ok(true),
        }
    }
}

pub struct SshClient {
    pub nas: crate::nas_config::NasConfig,
    handle: Handle<ClientHandler>,
    use_sudo: bool,
    password: String,
    host: String,
    port: u16,
    username: String,
    host_fingerprint: Option<String>,
    key_path: Option<String>,
    key_passphrase: Option<String>,
}

pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub enum AuthMethod {
    Password(String),
    Key { key_path: String, passphrase: Option<String> },
}

impl SshClient {
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        auth: &AuthMethod,
        use_sudo: bool,
        known_host_key: Option<&str>,
        nas: crate::nas_config::NasConfig,
    ) -> anyhow::Result<Self> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(nas.ssh_timeout_secs)),
            keepalive_interval: Some(Duration::from_secs(nas.keepalive_secs)),
            ..Default::default()
        };
        let config = Arc::new(config);
        let fp_slot = Arc::new(Mutex::new(None::<String>));
        let handler = ClientHandler {
            known_host_key: known_host_key.map(|s| s.to_string()),
            received_fingerprint: fp_slot.clone(),
        };

        let mut handle = client::connect(config, (host, port), handler).await?;

        let (password_for_sudo, key_path, key_passphrase) = match auth {
            AuthMethod::Password(pwd) => {
                let auth_res = handle.authenticate_password(username, pwd).await?;
                if !auth_res.success() {
                    anyhow::bail!("Autenticacion con password fallida para '{}'", username);
                }
                (pwd.clone(), None, None)
            }
            AuthMethod::Key { key_path, passphrase } => {
                let key_pair = load_secret_key(key_path, passphrase.as_deref())
                    .map_err(|e| anyhow::anyhow!("No se pudo cargar la clave SSH: {}", e))?;

                let rsa_hash = handle.best_supported_rsa_hash().await?.flatten();
                let auth_res = handle
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                    )
                    .await?;

                if !auth_res.success() {
                    anyhow::bail!("Autenticacion con clave SSH fallida para '{}'", username);
                }
                (String::new(), Some(key_path.clone()), passphrase.clone())
            }
        };

        let fp = fp_slot.lock().await.clone();

        Ok(Self {
            nas,
            handle,
            use_sudo,
            password: password_for_sudo,
            host: host.to_string(),
            port,
            username: username.to_string(),
            host_fingerprint: fp,
            key_path,
            key_passphrase,
        })
    }

    pub fn get_host_fingerprint(&self) -> Option<&str> {
        self.host_fingerprint.as_deref()
    }

    pub async fn init_sudo_cache(&mut self) -> anyhow::Result<()> {
        if !self.use_sudo || self.password.is_empty() {
            return Ok(());
        }
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, "sudo -S -v").await?;
        channel.data(self.password.as_bytes()).await?;
        channel.data(&b"\n"[..]).await?;
        channel.eof().await?;

        loop {
            let Some(msg) = channel.wait().await else { break };
            match msg {
                ChannelMsg::ExitStatus { .. } => break,
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn reconnect(&mut self) -> anyhow::Result<()> {
        let auth = if let Some(kp) = &self.key_path {
            AuthMethod::Key {
                key_path: kp.clone(),
                passphrase: self.key_passphrase.clone(),
            }
        } else {
            AuthMethod::Password(self.password.clone())
        };
        let new_client = Self::connect(
            &self.host,
            self.port,
            &self.username,
            &auth,
            self.use_sudo,
            self.host_fingerprint.as_deref(),
            self.nas.clone(),
        )
        .await?;
        self.handle = new_client.handle;
        Ok(())
    }

    async fn exec_raw(&mut self, command: &str) -> anyhow::Result<CommandResult> {
        let mut channel = match self.handle.channel_open_session().await {
            Ok(ch) => ch,
            Err(_) => {
                self.reconnect().await?;
                self.handle.channel_open_session().await?
            }
        };

        channel.exec(true, command).await?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = -1i32;

        loop {
            let Some(msg) = channel.wait().await else {
                break;
            };
            match msg {
                ChannelMsg::Data { ref data } => {
                    stdout.extend_from_slice(data);
                }
                ChannelMsg::ExtendedData { ref data, ext } => {
                    if ext == 1 {
                        let text = String::from_utf8_lossy(data);
                        if text.contains("[sudo]") || text.contains("Could not chdir") {
                            continue;
                        }
                        stderr.extend_from_slice(data);
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = exit_status as i32;
                }
                _ => {}
            }
        }

        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code,
        })
    }

    pub async fn exec(&mut self, command: &str) -> anyhow::Result<CommandResult> {
        self.exec_with_timeout(command, self.nas.ssh_timeout_secs).await
    }

    pub async fn exec_with_timeout(&mut self, command: &str, timeout_secs: u64) -> anyhow::Result<CommandResult> {
        let rewritten = self.rewrite_command(command);
        let timeout = timeout_secs.clamp(5, self.nas.ssh_timeout_secs.max(5));
        tokio::time::timeout(Duration::from_secs(timeout), self.exec_once(&rewritten)).await
            .map_err(|_| anyhow::anyhow!("El comando SSH superó el tiempo máximo de {} segundos", timeout))?
    }

    pub fn binary(&self, name: &str) -> String {
        let path = match name {
            "/usr/syno/bin/synoacltool" => &self.nas.synoacltool_path,
            "/usr/syno/sbin/synoshare" => &self.nas.synoshare_path,
            "/usr/syno/sbin/synouser" => &self.nas.synouser_path,
            "/usr/syno/sbin/synogroup" => &self.nas.synogroup_path,
            "/bin/find" | "find" => &self.nas.find_path,
            _ => name,
        };
        crate::syno::shell_escape(path)
    }

    fn rewrite_command(&self, command: &str) -> String {
        let (first, rest) = command.split_once(' ').unwrap_or((command, ""));
        if ["/usr/syno/bin/synoacltool", "/usr/syno/sbin/synoshare", "/usr/syno/sbin/synouser", "/usr/syno/sbin/synogroup", "/bin/find", "find"].contains(&first) {
            format!("{} {}", self.binary(first), rest)
        } else { command.to_string() }
    }

    pub fn exclusion_expression(&self) -> String {
        let mut patterns = vec!["-name '@*'".to_string(), "-name '#recycle'".to_string()];
        patterns.extend(self.nas.excluded_folders.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("-name {}", crate::syno::shell_escape(s))));
        patterns.join(" -o ")
    }

    async fn exec_once(&mut self, command: &str) -> anyhow::Result<CommandResult> {
        if self.use_sudo {
            if self.password.is_empty() {
                return self.exec_raw(&format!("sudo -n {}", command)).await;
            }
            let fallback_cmd = format!("sudo -S -p '' {}", command);
            let mut channel = match self.handle.channel_open_session().await {
                Ok(channel) => channel,
                Err(_) => {
                    self.reconnect().await?;
                    self.handle.channel_open_session().await?
                }
            };
            channel.exec(true, fallback_cmd.as_str()).await?;
            channel.data(self.password.as_bytes()).await?;
            channel.data(&b"\n"[..]).await?;
            channel.eof().await?;

            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut exit_code = -1i32;

            loop {
                let Some(msg) = channel.wait().await else { break };
                match msg {
                    ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                    ChannelMsg::ExtendedData { ref data, ext } => {
                        if ext == 1 {
                            let text = String::from_utf8_lossy(data);
                            if text.contains("[sudo]") || text.contains("Could not chdir") {
                                continue;
                            }
                            stderr.extend_from_slice(data);
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit_code = exit_status as i32,
                    _ => {}
                }
            }

            Ok(CommandResult {
                stdout: String::from_utf8_lossy(&stdout).to_string(),
                stderr: String::from_utf8_lossy(&stderr).to_string(),
                exit_code,
            })
        } else {
            self.exec_raw(command).await
        }
    }

    pub async fn exec_script(&mut self, script: &str) -> anyhow::Result<CommandResult> {
        self.exec(&format!("sh -c {}", crate::syno::shell_escape(script))).await
    }

    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.handle
            .disconnect(Disconnect::ByApplication, "", "Spanish")
            .await?;
        Ok(())
    }
}

pub struct SshState {
    pub client: Mutex<Option<SshClient>>,
    pub connection_name: Mutex<String>,
}
