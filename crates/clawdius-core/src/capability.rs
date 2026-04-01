use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static CAP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceScope {
    pub paths: Vec<String>,
    pub hosts: Vec<String>,
    pub env_vars: Vec<String>,
}

impl Default for ResourceScope {
    fn default() -> Self {
        Self {
            paths: vec![],
            hosts: vec![],
            env_vars: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    FsRead,
    FsWrite,
    NetTcp,
    NetUdp,
    ExecSpawn,
    SecretAccess,
    EnvRead,
    EnvWrite,
}

#[derive(Debug, Clone)]
pub struct CapabilityToken {
    id: u64,
    resource: ResourceScope,
    permissions: HashSet<Permission>,
    signature: [u8; 32],
    expires_at: Option<Instant>,
}

impl CapabilityToken {
    pub fn new(resource: ResourceScope, permissions: HashSet<Permission>) -> Self {
        let id = CAP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let signature = Self::compute_signature(&resource, &permissions, id);
        Self {
            id,
            resource,
            permissions,
            signature,
            expires_at: None,
        }
    }

    pub fn with_expiry(mut self, duration: Duration) -> Self {
        self.expires_at = Some(Instant::now() + duration);
        self
    }

    pub fn verify(&self) -> bool {
        if self.is_expired() {
            return false;
        }
        let expected = Self::compute_signature(&self.resource, &self.permissions, self.id);
        self.signature == expected
    }

    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    pub fn permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    pub fn resource(&self) -> &ResourceScope {
        &self.resource
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn derive(&self, subset: HashSet<Permission>) -> Option<Self> {
        if !subset.is_subset(&self.permissions) {
            return None;
        }
        let id = CAP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let signature = Self::compute_signature(&self.resource, &subset, id);
        Some(Self {
            id,
            resource: self.resource.clone(),
            permissions: subset,
            signature,
            expires_at: self.expires_at,
        })
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(t) => Instant::now() >= t,
            None => false,
        }
    }

    fn compute_signature(
        resource: &ResourceScope,
        permissions: &HashSet<Permission>,
        id: u64,
    ) -> [u8; 32] {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        hasher.update(b"sentinel-hmac-key-prototype-32b!");
        hasher.update(id.to_le_bytes());
        for path in &resource.paths {
            hasher.update(path.as_bytes());
        }
        for host in &resource.hosts {
            hasher.update(host.as_bytes());
        }
        for var in &resource.env_vars {
            hasher.update(var.as_bytes());
        }
        for perm in permissions {
            hasher.update([*perm as u8]);
        }
        hasher.finalize().into()
    }
}
