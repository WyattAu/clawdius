# OS Compatibility Analysis

**Document ID:** CP-OS-001  
**Version:** 1.0.0  
**Phase:** 4.5 (Cross-Platform Compatibility)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Trace To:** REQ-6.4, hal_platform.md

---

## 1. Overview

### 1.1 Purpose

This document specifies OS-specific implementations, abstractions, and limitations for Clawdius across Linux, macOS, and Windows (via WSL2).

### 1.2 Platform Support Tiers

| Tier | Platform | Support Level | Use Case |
|------|----------|---------------|----------|
| 1 | Linux x86_64 (glibc) | Full Production | Primary deployment |
| 2 | macOS ARM64/x86_64 | Full Production | Developer workstations |
| 3 | Windows via WSL2 | Partial (Dev Only) | Windows developers |
| 4 | Linux ARM64 | Experimental | ARM servers |

---

## 2. Platform-Specific Features

### 2.1 Linux (Tier 1)

#### 2.1.1 Async Runtime: monoio with io_uring

```rust
#[cfg(all(target_os = "linux", feature = "io_uring"))]
mod linux_runtime {
    use monoio::IoUringDriver;
    
    pub fn create_runtime() -> monoio::Runtime<IoUringDriver> {
        monoio::RuntimeBuilder::<IoUringDriver>::new()
            .enable_all()
            .build()
            .expect("Failed to create io_uring runtime")
    }
}
```

**Requirements:**
- Linux kernel 5.1+ (5.6+ recommended for full feature set)
- io_uring syscalls available
- `CONFIG_IO_URING` enabled in kernel

**Fallback:** If io_uring unavailable, use tokio:
```rust
#[cfg(all(target_os = "linux", not(feature = "io_uring")))]
mod linux_fallback {
    pub fn create_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }
}
```

#### 2.1.2 Sandbox: bubblewrap (bwrap)

```rust
#[cfg(target_os = "linux")]
mod linux_sandbox {
    pub struct BubblewrapBackend;
    
    impl SandboxBackend for BubblewrapBackend {
        fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
            let mut cmd = Command::new("bwrap");
            
            cmd.arg("--ro-bind").arg("/usr").arg("/usr");
            cmd.arg("--ro-bind").arg("/lib").arg("/lib");
            cmd.arg("--ro-bind").arg("/lib64").arg("/lib64");
            cmd.arg("--dir").arg("/tmp");
            cmd.arg("--proc").arg("/proc");
            cmd.arg("--dev").arg("/dev");
            
            for mount in &config.mounts {
                let flag = if mount.read_only { "--ro-bind" } else { "--bind" };
                cmd.arg(flag).arg(&mount.source).arg(&mount.target);
            }
            
            if !config.capabilities.has(Permission::NET_TCP) {
                cmd.arg("--unshare-net");
            }
            
            cmd.arg("--die-with-parent");
            cmd.arg("--new-session");
            cmd.arg("--").arg(&config.command.program);
            
            let child = cmd.spawn()?;
            Ok(SandboxHandle::from(child))
        }
    }
}
```

**bubblewrap Features:**
- User namespaces (no root required)
- PID namespaces
- Network namespaces
- Mount namespaces
- Seccomp filters

**Limitations:**
- Requires `/proc/sys/kernel/unprivileged_userns_clone` enabled on older kernels
- Some distributions require specific package installation

#### 2.1.3 Keyring: Secret Service / libsecret

```rust
#[cfg(target_os = "linux")]
mod linux_keyring {
    use keyring::Entry;
    
    pub struct LibsecretBackend {
        collection: String,
    }
    
    impl KeyringBackend for LibsecretBackend {
        fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError> {
            let entry = Entry::new(service, account)?;
            let password = entry.get_password()?;
            Ok(Secret::new(password))
        }
        
        fn set(&self, service: &str, account: &str, password: &str) -> Result<(), KeyringError> {
            Entry::new(service, account)?.set_password(password)?;
            Ok(())
        }
    }
}
```

**Dependencies:**
- `libsecret-1-dev` package
- D-Bus Secret Service (GNOME Keyring or KDE Wallet)

#### 2.1.4 File System Events: inotify

```rust
#[cfg(target_os = "linux")]
mod linux_fs_watcher {
    use notify::{RecommendedWatcher, Watcher, RecursiveMode};
    
    pub struct InotifyWatcher {
        watcher: RecommendedWatcher,
        rx: Receiver<Result<Event, notify::Error>>,
    }
    
    impl FsWatcherBackend for InotifyWatcher {
        fn watch(&mut self, path: &Path) -> Result<(), WatchError> {
            self.watcher.watch(path, RecursiveMode::Recursive)?;
            Ok(())
        }
    }
}
```

**inotify Limits:**
- Default max watches: 8192 (configurable via `/proc/sys/fs/inotify/max_user_watches`)
- Default max instances: 128

---

### 2.2 macOS (Tier 2)

#### 2.2.1 Async Runtime: tokio (no io_uring)

```rust
#[cfg(target_os = "macos")]
mod macos_runtime {
    pub fn create_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("clawdius-worker")
            .build()
            .expect("Failed to create tokio runtime")
    }
}
```

**Note:** macOS does not support io_uring. Use kqueue-based tokio.

#### 2.2.2 Sandbox: sandbox-exec

```rust
#[cfg(target_os = "macos")]
mod macos_sandbox {
    pub struct SandboxExecBackend;
    
    impl SandboxBackend for SandboxExecBackend {
        fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
            let profile = self.generate_sbpl(&config);
            let profile_path = self.write_profile(&profile)?;
            
            let mut cmd = Command::new("sandbox-exec");
            cmd.arg("-p").arg(&profile_path);
            cmd.arg("-D").arg(&format!("TEMP_DIR={}", std::env::temp_dir().display()));
            
            for (key, value) in &config.environment {
                cmd.env(key, value);
            }
            
            cmd.arg(&config.command.program);
            cmd.args(&config.command.args);
            
            let child = cmd.spawn()?;
            Ok(SandboxHandle::from(child))
        }
        
        fn generate_sbpl(&self, config: &SandboxConfig) -> String {
            let mut sbpl = String::from("(version 1)\n(deny default)\n");
            
            sbpl.push_str("(allow process-exec (literal \"/usr/bin/env\"))\n");
            sbpl.push_str("(allow sysctl-read)\n");
            
            for mount in &config.mounts {
                let ops = if mount.read_only { 
                    "file-read*" 
                } else { 
                    "file-read* file-write*" 
                };
                sbpl.push_str(&format!(
                    "(allow {} (subpath \"{}\"))\n",
                    ops, mount.source.display()
                ));
            }
            
            if config.capabilities.has(Permission::NET_TCP) {
                sbpl.push_str("(allow network-outbound (remote tcp))\n");
            }
            
            sbpl
        }
    }
}
```

**sandbox-exec Features:**
- Built into macOS
- Seatbelt profile language (SBPL)
- No root required
- Process-level isolation

**Limitations:**
- Apple may deprecate sandbox-exec in future releases
- Documentation is minimal
- Some profiles require entitlements

#### 2.2.3 Keyring: Apple Keychain

```rust
#[cfg(target_os = "macos")]
mod macos_keyring {
    use keyring::Entry;
    use security_framework::passwords;
    
    pub struct KeychainBackend;
    
    impl KeyringBackend for KeychainBackend {
        fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError> {
            let entry = Entry::new(service, account)?;
            let password = entry.get_password()?;
            Ok(Secret::new(password))
        }
        
        fn set(&self, service: &str, account: &str, password: &str) -> Result<(), KeyringError> {
            Entry::new(service, account)?.set_password(password)?;
            Ok(())
        }
        
        fn delete(&self, service: &str, account: &str) -> Result<(), KeyringError> {
            Entry::new(service, account)?.delete_password()?;
            Ok(())
        }
    }
}
```

**Keychain Features:**
- Secure Enclave integration (Apple Silicon)
- iCloud Keychain sync
- Access control lists
- Touch ID / Face ID integration

#### 2.2.4 File System Events: FSEvents

```rust
#[cfg(target_os = "macos")]
mod macos_fs_watcher {
    use notify::{RecommendedWatcher, Watcher, RecursiveMode, Config};
    
    pub struct FseventsWatcher {
        watcher: RecommendedWatcher,
        rx: Receiver<Result<Event, notify::Error>>,
    }
    
    impl FsWatcherBackend for FseventsWatcher {
        fn watch(&mut self, path: &Path) -> Result<(), WatchError> {
            let config = Config::default()
                .with_compare_contents(true);
            self.watcher.configure(config)?;
            self.watcher.watch(path, RecursiveMode::Recursive)?;
            Ok(())
        }
    }
}
```

**FSEvents Features:**
- Kernel-level notification
- Historical events support
- Low latency
- Coalescing of rapid events

---

### 2.3 Windows via WSL2 (Tier 3)

#### 2.3.1 Architecture

WSL2 runs a full Linux kernel in a lightweight VM. Clawdius uses the Linux implementation within WSL2.

```
+------------------+
| Windows Host     |
| +--------------+ |
| | WSL2 VM      | |
| | +----------+ | |
| | | Linux    | | |
| | | Clawdius | | |
| | +----------+ | |
| +--------------+ |
+------------------+
```

#### 2.3.2 Detection

```rust
#[cfg(target_os = "linux")]
fn is_wsl2() -> bool {
    std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
        || std::fs::read_to_string("/proc/version")
            .map(|v| v.contains("microsoft") || v.contains("WSL"))
            .unwrap_or(false)
}

pub fn detect_platform() -> Platform {
    #[cfg(target_os = "linux")]
    {
        if is_wsl2() {
            Platform::WindowsWSL2
        } else {
            Platform::Linux
        }
    }
    #[cfg(target_os = "macos")]
    {
        Platform::MacOS
    }
}
```

#### 2.3.3 Limitations

| Feature | Linux | WSL2 | Workaround |
|---------|-------|------|------------|
| bubblewrap | ✓ | Partial | Enable systemd or use --privileged |
| io_uring | ✓ | ✓ | Full support in recent WSL2 |
| inotify | ✓ | ✓ | Works with /mnt/wsl paths |
| Keyring | Secret Service | N/A | Use file-based or Windows interop |

#### 2.3.4 Windows Credential Manager Interop

```rust
#[cfg(all(target_os = "linux", feature = "wsl2"))]
mod wsl2_keyring {
    pub struct WindowsCredentialBackend;
    
    impl KeyringBackend for WindowsCredentialBackend {
        fn get(&self, service: &str, account: &str) -> Result<Secret<String>, KeyringError> {
            let output = Command::new("cmd.exe")
                .args(["/c", "cmdkey", "/generic:", &format!("{}:{}", service, account)])
                .output()?;
            
            if !output.status.success() {
                return Err(KeyringError::NotFound);
            }
            
            let credential = String::from_utf8_lossy(&output.stdout);
            Ok(Secret::new(credential.to_string()))
        }
    }
}
```

---

## 3. Platform Abstraction Layer (PAL) Trait Interface

### 3.1 Core HAL Trait

```rust
pub trait Hal: Send + Sync {
    fn platform(&self) -> Platform;
    fn sandbox(&self) -> &dyn SandboxBackend;
    fn keyring(&self) -> &dyn KeyringBackend;
    fn fs_watcher(&self) -> &dyn FsWatcherBackend;
    fn runtime(&self) -> &dyn RuntimeBackend;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Linux,
    MacOS,
    WindowsWSL2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeType {
    IoUring,
    Tokio,
}
```

### 3.2 Platform Factory

```rust
pub fn create_hal() -> Box<dyn Hal> {
    #[cfg(target_os = "linux")]
    {
        if is_wsl2() {
            Box::new(Wsl2Hal::new())
        } else {
            Box::new(LinuxHal::new())
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsHal::new())
    }
}

#[cfg(target_os = "linux")]
struct LinuxHal {
    sandbox: BubblewrapBackend,
    keyring: LibsecretBackend,
    fs_watcher: InotifyWatcher,
    runtime: IoUringRuntime,
}

#[cfg(target_os = "macos")]
struct MacOsHal {
    sandbox: SandboxExecBackend,
    keyring: KeychainBackend,
    fs_watcher: FseventsWatcher,
    runtime: TokioRuntime,
}
```

---

## 4. Feature Detection

### 4.1 Runtime Feature Detection

```rust
pub struct PlatformFeatures {
    pub io_uring: bool,
    pub user_namespaces: bool,
    pub seccomp: bool,
    pub keyring: bool,
}

#[cfg(target_os = "linux")]
pub fn detect_features() -> PlatformFeatures {
    PlatformFeatures {
        io_uring: detect_io_uring(),
        user_namespaces: detect_user_namespaces(),
        seccomp: detect_seccomp(),
        keyring: detect_secret_service(),
    }
}

fn detect_io_uring() -> bool {
    use std::fs;
    
    fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| {
            let parts: Vec<&str> = v.split_whitespace().nth(2)?.split('.').collect();
            let major: u32 = parts.first()?.parse().ok()?;
            let minor: u32 = parts.get(1)?.parse().ok()?;
            Some(major > 5 || (major == 5 && minor >= 1))
        })
        .unwrap_or(false)
}

fn detect_user_namespaces() -> bool {
    std::path::Path::new("/proc/self/ns/user").exists()
}
```

### 4.2 Graceful Degradation

```rust
impl LinuxHal {
    pub fn new() -> Self {
        let features = detect_features();
        
        let sandbox = if features.user_namespaces {
            BubblewrapBackend::new()
        } else {
            log::warn!("User namespaces unavailable, using restricted mode");
            BubblewrapBackend::new_restricted()
        };
        
        let runtime = if features.io_uring {
            IoUringRuntime::new()
        } else {
            log::info!("io_uring unavailable, falling back to tokio");
            TokioRuntime::new()
        };
        
        Self { sandbox, runtime, ... }
    }
}
```

---

## 5. OS-Specific Dependencies

### 5.1 Cargo.toml Platform-Specific Dependencies

```toml
[target.'cfg(target_os = "linux")'.dependencies]
monoio = { version = "0.2", optional = true }
libloading = "0.8"

[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "2.9"
core-foundation = "0.9"

[target.'cfg(all(target_os = "linux", feature = "wsl2"))'.dependencies]
winapi = { version = "0.3", optional = true }

[features]
default = []
io_uring = ["monoio"]
wsl2 = ["winapi"]
```

### 5.2 System Package Dependencies

| Platform | Packages |
|----------|----------|
| Linux (Debian/Ubuntu) | `libsecret-1-dev`, `bubblewrap`, `pkg-config` |
| Linux (Fedora/RHEL) | `libsecret-devel`, `bubblewrap`, `pkgconfig` |
| Linux (Arch) | `libsecret`, `bubblewrap` |
| macOS | Xcode Command Line Tools |
| WSL2 | Same as Linux, plus WSL2 kernel 5.15+ |

---

## 6. Limitations Summary

| Feature | Linux | macOS | WSL2 |
|---------|-------|-------|------|
| io_uring | ✓ (5.1+) | ✗ | ✓ |
| bubblewrap | ✓ | ✗ | Partial |
| sandbox-exec | ✗ | ✓ | ✗ |
| Native Keyring | Secret Service | Keychain | Interop |
| inotify | ✓ | ✗ | ✓ |
| fsevents | ✗ | ✓ | ✗ |
| Performance | Best | Good | Good |
| Sandbox Isolation | Strong | Medium | Medium |

---

## 7. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| IEEE 1003.1 (POSIX) | Base | Full (Linux/macOS) |
| NIST SP 800-53 | SC-3 | Full |
| REQ-6.4 | PAL | Full |

---

**Document Status:** APPROVED  
**Next Review:** After implementation testing
