use crate::connection_options::SshConfig;
use std::{collections::HashMap, path::PathBuf, process::{Child, Command, Stdio}, sync::{Arc, Weak, Mutex, OnceLock, atomic::{AtomicBool, Ordering}}};
use tokio::sync::Mutex as AsyncMutex;

struct Tunnel {
    child: Mutex<Child>,
    directory: PathBuf,
    port: u16,
    config: SshConfig,
    target_host: String,
    target_port: String,
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() { let _ = child.kill(); let _ = child.wait(); }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
#[derive(Default)]
struct TunnelRegistry {
    active: HashMap<String, Arc<Tunnel>>,
    generations: HashMap<String, u64>,
    processes: Vec<Weak<Tunnel>>,
}
impl TunnelRegistry {
    fn cancel(&mut self, id: &str) {
        *self.generations.entry(id.into()).or_default() += 1;
        self.active.remove(id);
    }
    fn stop_all(&mut self) {
        for tunnel in self.processes.iter().filter_map(Weak::upgrade) {
            let mut child = tunnel.child.lock().unwrap_or_else(|error| error.into_inner());
            let _ = child.kill(); let _ = child.wait();
        }
        self.active.clear(); self.processes.clear();
    }
}
fn tunnels() -> &'static Mutex<TunnelRegistry> {
    static TUNNELS: OnceLock<Mutex<TunnelRegistry>> = OnceLock::new();
    TUNNELS.get_or_init(|| Mutex::new(TunnelRegistry::default()))
}
static CLOSING: AtomicBool = AtomicBool::new(false);
static STARTUP: AsyncMutex<()> = AsyncMutex::const_new(());
// Held by a resolved connection so disconnect/edit never closes a tunnel underneath an active query.
pub struct TunnelLease { _tunnel: Arc<Tunnel> }

pub async fn connect(id: &str, config: &SshConfig, host: &str, port: &str) -> Result<(u16, TunnelLease), String> {
    // ponytail: startup serializes across profiles; use per-profile locks if concurrent startup latency matters.
    let generation = {
        let registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
        if CLOSING.load(Ordering::SeqCst) { return Err("Application is closing".into()); }
        *registry.generations.get(id).unwrap_or(&0)
    };
    let _startup = STARTUP.lock().await;
    let existing = {
        let registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
        if CLOSING.load(Ordering::SeqCst) { return Err("Application is closing".into()); }
        if *registry.generations.get(id).unwrap_or(&0) != generation { return Err("SSH connection was disconnected or changed during startup".into()); }
        registry.active.get(id).cloned()
    };
    if let Some(tunnel) = existing {
        let alive = tunnel.child.lock().map_err(|_| "SSH process lock failed")?.try_wait().map_err(|e| e.to_string())?.is_none();
        if alive && tunnel.config == *config && tunnel.target_host == host && tunnel.target_port == port {
            return Ok((tunnel.port, TunnelLease { _tunnel: tunnel.clone() }));
        }
    }
    tunnels().lock().map_err(|_| "SSH registry lock failed")?.active.remove(id);
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
    let local_port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let directory = std::env::temp_dir().join(format!("qs-ssh-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]));
    std::fs::create_dir(&directory).map_err(|e| format!("Could not create SSH runtime directory: {e}"))?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)) {
            let _ = std::fs::remove_dir(&directory); return Err(error.to_string());
        }
    }
    let control = directory.join("c");
    let log = match std::fs::File::create(directory.join("error")) {
        Ok(log) => log,
        Err(error) => { let _ = std::fs::remove_dir(&directory); return Err(error.to_string()); }
    };
    drop(listener);
    let args = ssh_arguments(config, host, port, local_port, &control);
    let tunnel = {
        // Register every child under the same lock used by shutdown, before any await.
        let mut registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
        if CLOSING.load(Ordering::SeqCst) { let _ = std::fs::remove_dir_all(&directory); return Err("Application is closing".into()); }
        let child = match Command::new("ssh").args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(log).spawn() {
            Ok(child) => child,
            Err(error) => { let _ = std::fs::remove_dir_all(&directory); return Err(format!("Could not start OpenSSH. Install an OpenSSH client: {error}")); }
        };
        let tunnel = Arc::new(Tunnel { child: Mutex::new(child), directory, port: local_port, config: config.clone(), target_host: host.into(), target_port: port.into() });
        registry.processes.retain(|process| process.strong_count() > 0);
        registry.processes.push(Arc::downgrade(&tunnel));
        tunnel
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(12);
    loop {
        { let registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
          if CLOSING.load(Ordering::SeqCst) { return Err("Application is closing".into()); }
          if *registry.generations.get(id).unwrap_or(&0) != generation { return Err("SSH connection was disconnected or changed during startup".into()); }
        }
        let exited = tunnel.child.lock().map_err(|_| "SSH process lock failed")?.try_wait().map_err(|e| e.to_string())?.is_some();
        if exited { return Err(format!("SSH tunnel failed: {}", error_detail(&tunnel.directory))); }
        if tokio::time::Instant::now() >= deadline { return Err(format!("SSH tunnel timed out: {}", error_detail(&tunnel.directory))); }
        let ready = tokio::time::timeout(std::time::Duration::from_secs(1), tokio::process::Command::new("ssh")
            .arg("-S").arg(&control).args(["-O", "check", "-F", "none", "-l", &config.username, "--", &config.host])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).status()).await;
        if matches!(ready, Ok(Ok(status)) if status.success()) {
            let mut registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
            if CLOSING.load(Ordering::SeqCst) { return Err("Application is closing".into()); }
            if *registry.generations.get(id).unwrap_or(&0) != generation { return Err("SSH connection was disconnected or changed during startup".into()); }
            if !id.is_empty() { registry.active.insert(id.into(), tunnel.clone()); }
            return Ok((local_port, TunnelLease { _tunnel: tunnel }));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
fn error_detail(directory: &std::path::Path) -> String {
    use std::io::Read;
    let mut bytes = Vec::new();
    if let Ok(file) = std::fs::File::open(directory.join("error")) { let _ = file.take(4096).read_to_end(&mut bytes); }
    let detail = String::from_utf8_lossy(&bytes).trim().to_string();
    if detail.is_empty() { "Check the SSH host, known_hosts entry, and agent/identity file.".into() } else { detail }
}
fn ssh_arguments(config: &SshConfig, host: &str, port: &str, local: u16, control: &std::path::Path) -> Vec<String> {
    let host = if host.contains(':') { format!("[{host}]") } else { host.into() };
    let mut args: Vec<String> = ["-F", "none", "-N", "-M", "-S"].into_iter().map(String::from).collect();
    args.push(control.to_string_lossy().into_owned());
    for option in ["BatchMode=yes", "StrictHostKeyChecking=yes", "ExitOnForwardFailure=yes", "ConnectTimeout=10", "ServerAliveInterval=15", "ServerAliveCountMax=2", "ForwardAgent=no", "ForwardX11=no", "PermitLocalCommand=no", "ProxyCommand=none", "ProxyJump=none"] { args.extend(["-o".into(), option.into()]); }
    args.extend(["-L".into(), format!("127.0.0.1:{local}:{host}:{port}"), "-p".into(), config.port.to_string(), "-l".into(), config.username.clone()]);
    if let Some(identity) = &config.identity_file { args.extend(["-i".into(), identity.clone()]); }
    args.extend(["--".into(), config.host.clone()]);
    args
}
pub fn disconnect(id: &str) -> Result<(), String> {
    let mut registry = tunnels().lock().map_err(|_| "SSH registry lock failed")?;
    registry.cancel(id);
    Ok(())
}
pub fn close_all() {
    CLOSING.store(true, Ordering::SeqCst);
    tunnels().lock().unwrap_or_else(|error| error.into_inner()).stop_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwards_only_loopback_with_strict_hosts_and_no_shell() {
        let config = SshConfig { host: "bastion.example.com".into(), port: 22, username: "user".into(), identity_file: Some("/tmp/key with spaces".into()) };
        let args = ssh_arguments(&config, "::1", "5432", 12345, std::path::Path::new("/tmp/c"));
        assert!(args.contains(&"127.0.0.1:12345:[::1]:5432".into()));
        for flag in ["StrictHostKeyChecking=yes", "BatchMode=yes", "ExitOnForwardFailure=yes", "ForwardAgent=no", "ProxyCommand=none"] { assert!(args.contains(&flag.into())); }
        assert!(args.contains(&"/tmp/key with spaces".into()));
        assert_eq!(&args[args.len()-2..], ["--", "bastion.example.com"]);
    }
    #[test]
    fn disconnect_invalidates_pending_generation() {
        let mut registry = TunnelRegistry::default();
        let captured = *registry.generations.get("profile").unwrap_or(&0);
        registry.cancel("profile");
        assert_ne!(captured, *registry.generations.get("profile").unwrap());
        assert_eq!(*registry.generations.get("other").unwrap_or(&0), 0);
    }
    #[cfg(unix)]
    #[test]
    fn shutdown_kills_leased_and_unpublished_processes() {
        let mut registry = TunnelRegistry::default();
        let mut leases = Vec::new();
        for registered in [false, true] {
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("ssh-lifecycle-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            let tunnel = Arc::new(Tunnel {
                child: Mutex::new(Command::new("/bin/sleep").arg("60").spawn().unwrap()), directory,
                port: 1, config: SshConfig { host: "fixture".into(), port: 22, username: "fixture".into(), identity_file: None }, target_host: "fixture".into(), target_port: "1".into(),
            });
            registry.processes.push(Arc::downgrade(&tunnel));
            if registered { registry.active.insert("fixture".into(), tunnel.clone()); }
            leases.push(tunnel);
        }
        registry.stop_all();
        for tunnel in &leases { assert!(tunnel.child.lock().unwrap().try_wait().unwrap().is_some()); }
        assert!(registry.active.is_empty());
    }

    #[tokio::test]
    async fn queued_startup_cannot_survive_disconnect() {
        let barrier = STARTUP.lock().await;
        let id = format!("queued-fixture-{}", uuid::Uuid::new_v4());
        let config = SshConfig { host: "127.0.0.1".into(), port: 1, username: "fixture".into(), identity_file: None };
        let mut pending = Box::pin(connect(&id, &config, "127.0.0.1", "5432"));
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending).await.is_err());
        disconnect(&id).unwrap();
        drop(barrier);
        let error = match pending.await { Ok(_) => panic!("Cancelled startup returned a tunnel"), Err(error) => error };
        assert!(error.contains("disconnected or changed"), "{error}");
        assert!(!tunnels().lock().unwrap().active.contains_key(&id));
    }

}
