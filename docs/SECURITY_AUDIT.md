# Security Audit Preparation

## Checklist

### Input Validation
- [ ] All CLI inputs validated
- [ ] File paths sanitized
- [ ] RPC messages validated

### Sandbox Security
- [ ] bubblewrap configured correctly
- [ ] sandbox-exec profiles tested
- [ ] Resource limits enforced

### Dependency Security
- [ ] All dependencies audited
- [ ] No known CVEs
- [ ] Supply chain verified

### Data Protection
- [ ] API keys encrypted at rest
- [ ] Session data protected
- [ ] Audit logs secured
