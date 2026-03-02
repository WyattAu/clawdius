# Platform Abstraction Layer (HAL) Specification

**Document ID:** HAL-PLATFORM-001  
**Version:** 1.0.0  
**Phase:** 2 (Architecture Refinement)  
**Status:** APPROVED  
**Created:** 2026-03-01  

---

## 1. Overview

### 1.1 Purpose

The Platform Abstraction Layer (HAL) provides a unified interface for platform-specific functionality, enabling Clawdius to run consistently across Linux, macOS, and Windows (via WSL2).

### 1.2 Scope

This specification covers:
- Sandbox backend abstraction
- Keyring/credential storage abstraction
- Filesystem watcher abstraction
- Platform detection

---

## 2. Trait Definitions

### 2.1 Core HAL Trait

```rust
pub trait Hal: Send + Sync {
    fn platform(&self) -> Platform;
    fn sandbox(&self) -> Box<dyn SandboxBackend>;
    fn keyring(&self) -> Box<dyn KeyringBackend>;
    fn fs_watcher(&self) -> Box<dyn FsWatcherBackend>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Linux,
    MacOS,
    WindowsWSL2,
}

pub fn detect_platform() -> Platform {
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
            Platform::WindowsWSL2
        } else {
            Platform::Linux
        }
    }
    #[cfg(target_os = "macos")]
    {
        Platform::MacOS
    }
    #[cfg(target_os = "windows")]
    {
        compile_error!("Windows native not supported; use WSL2")
    }
}
```

### 2.2 Sandbox Backend Trait

```rust
pub trait SandboxBackend: Send + Sync {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError>;
    fn execute(&self, handle: &SandboxHandle, cmd: CommandSpec) -> Result<ExitStatus, SandboxError>;
    fn kill(&self, handle: &SandboxHandle) -> Result<(), SandboxError>;
    fn stats(&self, handle: &SandboxHandle) -> Result<SandboxStats, SandboxError>;
    fn tier(&self) -> SandboxTier;
}

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub tier: SandboxTier,
    pub command: CommandSpec,
    pub mounts: Vec<MountSpec>,
    pub environment: HashMap<String, String>,
    pub capabilities: PermissionSet,
    pub timeout: Duration,
    pub memory_limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub source: PathBuf,
    pub target: PathBuf,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

#[derive(Debug)]
pub struct SandboxHandle {
    pub id: Uuid,
    pub pid: Option<u32>,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct SandboxStats {
    pub memory_used: usize,
    pub cpu_time: Duration,
    pub status: SandboxStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus {
    Running,
    Exited,
    Killed,
    Timeout,
}
```

### 2.3 Keyring Backend Trait

```rust
pub trait KeyringBackend: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError>;
    fn set(&self, service: &str, account: &str, password: &str) -> Result<(), KeyringError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), KeyringError>;
    fn list(&self, service: &str) -> Result<Vec<String>, KeyringError>;
}

#[derive(Debug, Clone)]
pub enum KeyringError {
    NotFound,
    AccessDenied,
    BackendError(String),
}
```

### 2.4 Filesystem Watcher Trait

```rust
pub trait FsWatcherBackend: Send + Sync {
    fn watch(&mut self, path: &Path) -> Result<(), WatchError>;
    fn unwatch(&mut self, path: &Path) -> Result<(), WatchError>;
    fn recv(&mut self) -> Option<FsEvent>;
}

#[derive(Debug, Clone)]
pub struct FsEvent {
    pub path: PathBuf,
    pub kind: FsEventKind,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}
```

---

## 3. Platform Implementations

### 3.1 Linux Implementation

#### Sandbox Backend: bubblewrap

```rust
pub struct LinuxSandboxBackend;

impl SandboxBackend for LinuxSandboxBackend {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        let mut cmd = Command::new("bwrap");
        
        // Add mounts
        for mount in &config.mounts {
            if mount.read_only {
                cmd.arg("--ro-bind").arg(&mount.source).arg(&mount.target);
            } else {
                cmd.arg("--bind").arg(&mount.source).arg(&mount.target);
            }
        }
        
        // Network isolation based on tier
        if !config.capabilities.has(Permission::NET_TCP) {
            cmd.arg("--unshare-net");
        }
        
        // Set environment
        for (key, value) in &config.environment {
            cmd.arg("--setenv").arg(key).arg(value);
        }
        
        // Execute command
        cmd.arg("--")
           .arg(&config.command.program)
           .args(&config.command.args);
        
        let child = cmd.spawn()
            .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        
        Ok(SandboxHandle {
            id: Uuid::new_v4(),
            pid: Some(child.id()),
            started_at: Instant::now(),
        })
    }
    
    fn tier(&self) -> SandboxTier {
        SandboxTier::Tier2
    }
}
```

#### Keyring Backend: libsecret

```rust
pub struct LinuxKeyringBackend {
    collection: keyring::Entry,
}

impl KeyringBackend for LinuxKeyringBackend {
    fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeyringError::BackendError(e.to_string()))?;
        
        let password = entry.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeyringError::NotFound,
                _ => KeyringError::BackendError(e.to_string()),
            })?;
        
        Ok(Secret::new(password))
    }
    
    fn set(&self, service: &str, account: &str, password: &str) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeyringError::BackendError(e.to_string()))?;
        
        entry.set_password(password)
            .map_err(|e| KeyringError::BackendError(e.to_string()))
    }
    
    fn delete(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeyringError::BackendError(e.to_string()))?;
        
        entry.delete_password()
            .map_err(|e| KeyringError::BackendError(e.to_string()))
    }
    
    fn list(&self, service: &str) -> Result<Vec<String>, KeyringError> {
        // libsecret doesn't support listing directly
        // Would need to use secret-service crate for full functionality
        Err(KeyringError::BackendError("Listing not supported".into()))
    }
}
```

#### FS Watcher: inotify

```rust
pub struct LinuxFsWatcher {
    watcher: notify::RecommendedWatcher,
    receiver: crossbeam_channel::Receiver<FsEvent>,
}

impl FsWatcherBackend for LinuxFsWatcher {
    fn watch(&mut self, path: &Path) -> Result<(), WatchError> {
        self.watcher.watch(path, notify::RecursiveMode::Recursive)
            .map_err(|e| WatchError::BackendError(e.to_string()))
    }
    
    fn unwatch(&mut self, path: &Path) -> Result<(), WatchError> {
        self.watcher.unwatch(path)
            .map_err(|e| WatchError::BackendError(e.to_string()))
    }
    
    fn recv(&mut self) -> Option<FsEvent> {
        self.receiver.try_recv().ok()
    }
}
```

### 3.2 macOS Implementation

#### Sandbox Backend: sandbox-exec

```rust
pub struct MacOsSandboxBackend;

impl SandboxBackend for MacOsSandboxBackend {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        // Generate sandbox profile
        let profile = self.generate_profile(&config);
        
        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-p").arg(&profile);
        
        // Set environment
        for (key, value) in &config.environment {
            cmd.env(key, value);
        }
        
        cmd.arg("--")
           .arg(&config.command.program)
           .args(&config.command.args);
        
        let child = cmd.spawn()
            .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        
        Ok(SandboxHandle {
            id: Uuid::new_v4(),
            pid: Some(child.id()),
            started_at: Instant::now(),
        })
    }
    
    fn generate_profile(&self, config: &SandboxConfig) -> String {
        let mut profile = String::from("(version 1)\n(deny default)\n");
        
        // Allow mounts
        for mount in &config.mounts {
            if mount.read_only {
                profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", mount.source.display()));
            } else {
                profile.push_str(&format!("(allow file-read* file-write* (subpath \"{}\"))\n", mount.source.display()));
            }
        }
        
        // Network permissions
        if config.capabilities.has(Permission::NET_TCP) {
            profile.push_str("(allow network-outbound)\n");
        }
        
        profile
    }
    
    fn tier(&self) -> SandboxTier {
        SandboxTier::Tier2
    }
}
```

#### Keyring Backend: Apple Keychain

```rust
pub struct MacOsKeyringBackend;

impl KeyringBackend for MacOsKeyringBackend {
    fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeyringError::BackendError(e.to_string()))?;
        
        let password = entry.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeyringError::NotFound,
                _ => KeyringError::BackendError(e.to_string()),
            })?;
        
        Ok(Secret::new(password))
    }
    
    // ... similar to Linux implementation
}
```

#### FS Watcher: fsevents

```rust
pub struct MacOsFsWatcher {
    // Uses notify crate with fsevents backend
    watcher: notify::RecommendedWatcher,
    receiver: crossbeam_channel::Receiver<FsEvent>,
}

// Implementation similar to Linux
```

### 3.3 Windows/WSL2 Implementation

Windows native is not supported. WSL2 uses Linux implementation via interop.

```rust
pub struct Wsl2SandboxBackend {
    linux_backend: LinuxSandboxBackend,
}

impl SandboxBackend for Wsl2SandboxBackend {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        // Delegate to Linux backend
        self.linux_backend.spawn(config)
    }
    
    fn tier(&self) -> SandboxTier {
        SandboxTier::Tier2
    }
}
```

---

## 4. Error Handling

### 4.1 Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
    
    #[error("Execute failed: {0}")]
    ExecuteFailed(String),
    
    #[error("Timeout exceeded")]
    Timeout,
    
    #[error("Capability denied: {0}")]
    CapabilityDenied(String),
    
    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    #[error("Backend error: {0}")]
    BackendError(String),
}
```

---

## 5. Configuration

### 5.1 Platform Detection

```toml
[platform]
auto_detect = true
fallback = "linux"

[platform.linux]
sandbox_backend = "bubblewrap"
keyring_backend = "libsecret"
fs_watcher = "inotify"

[platform.macos]
sandbox_backend = "sandbox-exec"
keyring_backend = "keychain"
fs_watcher = "fsevents"

[platform.wsl2]
sandbox_backend = "bubblewrap"
keyring_backend = "libsecret"
fs_watcher = "inotify"
```

---

## 6. Testing Strategy

### 6.1 Platform-Specific Tests

| Test | Linux | macOS | WSL2 |
|------|-------|-------|------|
| Sandbox spawn | bubblewrap | sandbox-exec | bubblewrap |
| Capability enforcement | ✓ | ✓ | ✓ |
| Keyring store/retrieve | libsecret | Keychain | libsecret |
| FS event detection | inotify | fsevents | inotify |

### 6.2 Mock Backend

```rust
pub struct MockSandboxBackend {
    spawned: Vec<SandboxConfig>,
}

impl SandboxBackend for MockSandboxBackend {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        self.spawned.push(config);
        Ok(SandboxHandle {
            id: Uuid::new_v4(),
            pid: Some(12345),
            started_at: Instant::now(),
        })
    }
    
    // ... mock implementations
}
```

---

## 7. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| IEEE 1016 | 10.2 | Full |
| NIST SP 800-53 | AC-3 | Full |
| OWASP ASVS | V5.3 | Full |

---

**Document Status:** APPROVED  
**Next Review:** After implementation (Phase 3)
