//! HAL Mock - Platform Abstraction Layer Mock
//!
//! Provides mock implementations for testing without actual hardware.
//! Supports Linux, macOS, and WSL2 platforms per BP-HOST-KERNEL-001.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Linux,
    MacOS,
    Wsl2,
}

impl Platform {
    pub fn detect() -> Self {
        if cfg!(target_os = "linux") {
            if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
                Self::Wsl2
            } else {
                Self::Linux
            }
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else {
            Self::Linux
        }
    }

    pub fn supports_io_uring(&self) -> bool {
        matches!(self, Self::Linux)
    }

    pub fn supports_bubblewrap(&self) -> bool {
        matches!(self, Self::Linux | Self::Wsl2)
    }

    pub fn supports_sandbox_exec(&self) -> bool {
        matches!(self, Self::MacOS)
    }

    pub fn runtime_type(&self) -> &'static str {
        match self {
            Self::Linux => "monoio",
            Self::MacOS | Self::Wsl2 => "tokio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringBackend {
    Libsecret,
    Keychain,
    SecretService,
    Mock,
}

pub trait Keyring: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Vec<u8>, KeyError>;
    fn set(&self, service: &str, account: &str, data: &[u8]) -> Result<(), KeyError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), KeyError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    NotFound,
    StorageError(String),
    InvalidInput,
}

pub struct MockKeyring {
    storage: Arc<Mutex<HashMap<(String, String), Vec<u8>>>>,
}

impl MockKeyring {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl Keyring for MockKeyring {
    fn get(&self, service: &str, account: &str) -> Result<Vec<u8>, KeyError> {
        let storage = self.storage.lock().unwrap();
        storage
            .get(&(service.to_string(), account.to_string()))
            .cloned()
            .ok_or(KeyError::NotFound)
    }

    fn set(&self, service: &str, account: &str, data: &[u8]) -> Result<(), KeyError> {
        let mut storage = self.storage.lock().unwrap();
        storage.insert((service.to_string(), account.to_string()), data.to_vec());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), KeyError> {
        let mut storage = self.storage.lock().unwrap();
        storage
            .remove(&(service.to_string(), account.to_string()))
            .map(|_| ())
            .ok_or(KeyError::NotFound)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemEvent {
    Created,
    Modified,
    Deleted,
}

pub trait FileSystemWatcher: Send + Sync {
    fn watch(&mut self, path: &str) -> Result<(), WatchError>;
    fn unwatch(&mut self, path: &str) -> Result<(), WatchError>;
    fn poll_events(&mut self) -> Vec<FileSystemEvent>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchError {
    PathNotFound,
    AlreadyWatching,
    NotWatching,
    SystemError(String),
}

pub struct MockFileSystemWatcher {
    watched: Vec<String>,
    events: Vec<FileSystemEvent>,
}

impl MockFileSystemWatcher {
    pub fn new() -> Self {
        Self {
            watched: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn inject_event(&mut self, event: FileSystemEvent) {
        self.events.push(event);
    }
}

impl Default for MockFileSystemWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemWatcher for MockFileSystemWatcher {
    fn watch(&mut self, path: &str) -> Result<(), WatchError> {
        if self.watched.contains(&path.to_string()) {
            return Err(WatchError::AlreadyWatching);
        }
        self.watched.push(path.to_string());
        Ok(())
    }

    fn unwatch(&mut self, path: &str) -> Result<(), WatchError> {
        let idx = self
            .watched
            .iter()
            .position(|p| p == path)
            .ok_or(WatchError::NotWatching)?;
        self.watched.remove(idx);
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<FileSystemEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub tier: u8,
    pub memory_limit: u64,
    pub cpu_quota: u64,
    pub mounts: Vec<MountSpec>,
    pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub source: String,
    pub destination: String,
    pub read_only: bool,
}

pub trait SandboxRunner: Send + Sync {
    fn spawn(&mut self, config: &SandboxConfig, command: &str) -> Result<u32, SandboxError>;
    fn wait(&mut self, pid: u32) -> Result<i32, SandboxError>;
    fn kill(&mut self, pid: u32) -> Result<(), SandboxError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    SpawnFailed(String),
    NotFound(u32),
    AlreadyRunning(u32),
    Timeout(u32),
    ResourceLimit,
}

pub struct MockSandboxRunner {
    processes: HashMap<u32, (String, Option<i32>)>,
    next_pid: u32,
}

impl MockSandboxRunner {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_pid: 1,
        }
    }
}

impl Default for MockSandboxRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxRunner for MockSandboxRunner {
    fn spawn(&mut self, _config: &SandboxConfig, command: &str) -> Result<u32, SandboxError> {
        let pid = self.next_pid;
        self.next_pid += 1;
        self.processes.insert(pid, (command.to_string(), None));
        Ok(pid)
    }

    fn wait(&mut self, pid: u32) -> Result<i32, SandboxError> {
        let process = self
            .processes
            .get_mut(&pid)
            .ok_or(SandboxError::NotFound(pid))?;

        process.1 = Some(0);
        Ok(0)
    }

    fn kill(&mut self, pid: u32) -> Result<(), SandboxError> {
        if !self.processes.contains_key(&pid) {
            return Err(SandboxError::NotFound(pid));
        }
        self.processes.remove(&pid);
        Ok(())
    }
}

pub struct Hal {
    platform: Platform,
    keyring: Arc<dyn Keyring>,
    watcher: Mutex<Box<dyn FileSystemWatcher>>,
    sandbox_runner: Mutex<Box<dyn SandboxRunner>>,
}

impl Hal {
    pub fn new(
        platform: Platform,
        keyring: Arc<dyn Keyring>,
        watcher: Box<dyn FileSystemWatcher>,
        sandbox_runner: Box<dyn SandboxRunner>,
    ) -> Self {
        Self {
            platform,
            keyring,
            watcher: Mutex::new(watcher),
            sandbox_runner: Mutex::new(sandbox_runner),
        }
    }

    pub fn mock() -> Self {
        Self::new(
            Platform::Linux,
            Arc::new(MockKeyring::new()),
            Box::new(MockFileSystemWatcher::new()),
            Box::new(MockSandboxRunner::new()),
        )
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn keyring(&self) -> &dyn Keyring {
        self.keyring.as_ref()
    }

    pub fn watcher(&self) -> &Mutex<Box<dyn FileSystemWatcher>> {
        &self.watcher
    }

    pub fn sandbox_runner(&self) -> &Mutex<Box<dyn SandboxRunner>> {
        &self.sandbox_runner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        assert!(matches!(
            platform,
            Platform::Linux | Platform::MacOS | Platform::Wsl2
        ));
    }

    #[test]
    fn test_linux_supports_io_uring() {
        assert!(Platform::Linux.supports_io_uring());
        assert!(!Platform::MacOS.supports_io_uring());
    }

    #[test]
    fn test_bubblewrap_support() {
        assert!(Platform::Linux.supports_bubblewrap());
        assert!(Platform::Wsl2.supports_bubblewrap());
        assert!(!Platform::MacOS.supports_bubblewrap());
    }

    #[test]
    fn test_runtime_selection() {
        assert_eq!(Platform::Linux.runtime_type(), "monoio");
        assert_eq!(Platform::MacOS.runtime_type(), "tokio");
    }

    #[test]
    fn test_mock_keyring_operations() {
        let keyring = MockKeyring::new();

        assert!(matches!(
            keyring.get("test", "user"),
            Err(KeyError::NotFound)
        ));

        keyring.set("test", "user", b"secret").unwrap();
        let data = keyring.get("test", "user").unwrap();
        assert_eq!(data, b"secret");

        keyring.delete("test", "user").unwrap();
        assert!(matches!(
            keyring.get("test", "user"),
            Err(KeyError::NotFound)
        ));
    }

    #[test]
    fn test_mock_filesystem_watcher() {
        let mut watcher = MockFileSystemWatcher::new();

        watcher.watch("/project").unwrap();
        assert!(matches!(
            watcher.watch("/project"),
            Err(WatchError::AlreadyWatching)
        ));

        watcher.inject_event(FileSystemEvent::Modified);
        let events = watcher.poll_events();
        assert_eq!(events.len(), 1);

        watcher.unwatch("/project").unwrap();
        assert!(matches!(
            watcher.unwatch("/project"),
            Err(WatchError::NotWatching)
        ));
    }

    #[test]
    fn test_mock_sandbox_runner() {
        let mut runner = MockSandboxRunner::new();
        let config = SandboxConfig {
            tier: 2,
            memory_limit: 256 * 1024 * 1024,
            cpu_quota: 100,
            mounts: vec![],
            env_vars: HashMap::new(),
        };

        let pid = runner.spawn(&config, "sleep 10").unwrap();
        assert!(pid > 0);

        let exit_code = runner.wait(pid).unwrap();
        assert_eq!(exit_code, 0);

        assert!(matches!(runner.kill(pid), Err(SandboxError::NotFound(_))));
    }

    #[test]
    fn test_hal_mock_creation() {
        let hal = Hal::mock();
        assert_eq!(hal.platform(), Platform::Linux);
    }

    #[test]
    fn test_keyring_arc_sharing() {
        let keyring = Arc::new(MockKeyring::new());
        keyring.set("shared", "test", b"data").unwrap();

        let hal = Hal::new(
            Platform::Linux,
            keyring.clone(),
            Box::new(MockFileSystemWatcher::new()),
            Box::new(MockSandboxRunner::new()),
        );

        let data = hal.keyring().get("shared", "test").unwrap();
        assert_eq!(data, b"data");
    }
}
