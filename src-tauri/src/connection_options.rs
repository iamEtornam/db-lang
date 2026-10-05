use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ConnectionOptions {
    pub group: String,
    pub environment: String,
    pub read_only: bool,
    pub ssh: Option<SshConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub identity_file: Option<String>,
}

impl ConnectionOptions {
    pub fn validate(&self, engine: &str, host: &str, port: &str) -> Result<(), String> {
        if self.group.len() > 80 || self.group.chars().any(char::is_control) {
            return Err("Connection group must contain at most 80 bytes and no control characters".into());
        }
        if !["", "development", "staging", "production"].contains(&self.environment.as_str()) {
            return Err("Choose development, staging, or production for the environment".into());
        }
        if let Some(ssh) = &self.ssh {
            if !["postgres", "mysql", "mariadb", "mongodb", "redis"].contains(&engine) {
                return Err("SSH tunnels require a PostgreSQL, MySQL, MariaDB, MongoDB, or Redis connection".into());
            }
            validate_host(host)?;
            if port.parse::<u16>().ok().filter(|p| *p > 0).is_none() {
                return Err("The database port must be between 1 and 65535 for SSH forwarding".into());
            }
            validate_host(&ssh.host)?;
            if ssh.port == 0 || ssh.username.is_empty() || ssh.username.starts_with('-') || !ssh.username.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
                return Err("Provide a valid SSH port and username".into());
            }
            if let Some(path) = &ssh.identity_file {
                if !std::path::Path::new(path).is_absolute() || path.chars().any(char::is_control) {
                    return Err("SSH identity file must be an absolute path without control characters".into());
                }
            }
        }
        Ok(())
    }
}

fn validate_host(host: &str) -> Result<(), String> {
    // Forwarding syntax accepts only a single hostname/IP, never a URI or options.
    if host.is_empty() || host.len() > 253 || host.starts_with('-') || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_:".contains(&b)) {
        return Err("SSH forwarding requires a single hostname or IP address, not a connection URI".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_defaults_and_tunnel_validation() {
        assert_eq!(serde_json::from_str::<ConnectionOptions>("{}").unwrap(), ConnectionOptions::default());
        let mut options = ConnectionOptions { group: "Analytics".into(), environment: "production".into(), read_only: true, ssh: Some(SshConfig { host: "bastion.example.com".into(), port: 22, username: "analyst".into(), identity_file: None }) };
        assert!(options.validate("postgres", "db.internal", "5432").is_ok());
        for (engine, host, port) in [("sqlite", "file.db", "5432"), ("postgres", "db.internal", "0"), ("mongodb", "mongodb+srv://host", "27017"), ("postgres", "-bad", "5432")] { assert!(options.validate(engine, host, port).is_err()); }
        options.ssh.as_mut().unwrap().username = "-oProxyCommand=bad".into();
        assert!(options.validate("postgres", "db.internal", "5432").is_err());
    }
}
