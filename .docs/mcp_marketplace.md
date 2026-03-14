# MCP Server Marketplace Discovery

This document describes the Model Context Protocol (MCP) Server marketplace discovery feature for Clawdius.

## Overview

The MCP Server Marketplace allows users to discover, install, and manage Model Context Protocol servers that extend Clawdius's capabilities with external tools and services.

## Architecture

### Components

1. **MarketplaceClient** - HTTP client for communicating with MCP registries
2. **ServerManifest** - Manifest format for MCP servers
3. **ServerRegistry** - Local registry of installed servers
4. **ServerRunner** - Process manager for running MCP servers

### Discovery Sources

1. **Official Clawdius Registry** - https://registry.clawdius.dev/mcp
2. **Community Registries** - User-configured additional sources
3. **GitHub Topics** - Search GitHub for `mcp-server` tagged repositories
4. **npm Registry** - Search for `@clawdius/mcp-*` packages

## Configuration

Add to `.clawdius/config.toml`:

```toml
[mcp]
# Enable MCP marketplace
enabled = true

# Official registry (default)
registry_url = "https://registry.clawdius.dev/mcp"

# Additional community registries
community_registries = [
    "https://mcp-community.example.com/registry",
    "https://my-company.com/mcp-registry"
]

# Auto-update servers
auto_update = false

# Cache TTL in seconds
cache_ttl = 3600

[mcp.security]
# Allowed server origins (for CORS)
allowed_origins = ["*"]

# Require signature verification
require_signature = false

# Public key for signature verification
# public_key = "..."
```

## CLI Commands

### List Available MCP Servers

```bash
clawdius mcp list
```

Output:
```
NAME                    DESCRIPTION                          VERSION    AUTHOR
github-mcp              GitHub API integration               1.2.0      Clawdius Team
slack-mcp               Slack API integration               0.9.1      Community
postgres-mcp            PostgreSQL database access          2.0.0      Database Tools
kubernetes-mcp          Kubernetes cluster management       1.0.0      DevOps Tools
filesystem-mcp          File system operations              1.1.0      Clawdius Team
```

### Search MCP Servers

```bash
clawdius mcp search "github"
```

Output:
```
NAME           DESCRIPTION                     RELEVANCE
github-mcp     GitHub API integration          0.95
gitlab-mcp     GitLab API integration          0.78
```

### Install MCP Server

```bash
clawdius mcp install github-mcp
```

### Update MCP Server

```bash
clawdius mcp update github-mcp
```

### Remove MCP Server

```bash
clawdius mcp remove github-mcp
```

### Show Server Details

```bash
clawdius mcp show github-mcp
```

Output:
```
Server: github-mcp
Version: 1.2.0
Author: Clawdius Team
Description: GitHub API integration for Clawdius

Tools:
  - search_repositories: Search GitHub repositories
  - get_repository: Get repository details
  - create_issue: Create a new issue
  - create_pull_request: Create a new pull request
  - get_pull_request: Get pull request details
  - list_commits: List commits in a repository

Resources:
  - repo: Repository access
  - issue: Issue management
  - pr: Pull request management

Configuration:
  GITHUB_TOKEN: GitHub personal access token (required)
  GITHUB_API_URL: GitHub API URL (optional, default: https://api.github.com)
```

## Server Manifest Format

```yaml
# mcp-server.yaml
name: github-mcp
version: 1.2.0
description: GitHub API integration for Clawdius
author: Clawdius Team
repository: https://github.com/clawdius/github-mcp
license: MIT

# Runtime configuration
runtime:
  type: nodejs  # or python, binary
  entrypoint: dist/index.js
  node_version: "18"

# Capabilities
tools:
  - name: search_repositories
    description: Search GitHub repositories
    parameters:
      query:
        type: string
        description: Search query
        required: true
      limit:
        type: integer
        description: Maximum results
        default: 10

  - name: get_repository
    description: Get repository details
    parameters:
      owner:
        type: string
        required: true
      repo:
        type: string
        required: true

resources:
  - type: repo
    operations: [read, write]
    description: Repository access

# Configuration schema
config_schema:
  GITHUB_TOKEN:
    type: string
    required: true
    description: GitHub personal access token
    secret: true
  GITHUB_API_URL:
    type: string
    default: https://api.github.com
    description: GitHub API URL

# Permissions required
permissions:
  - network: outbound  # Requires outbound network access
  - filesystem: read    # May need to read config files
```

## Integration with Clawdius

### Tool Invocation

When an MCP server is installed, its tools become available as Clawdius tools:

```json
{
  "tool": "mcp.github-mcp.search_repositories",
  "parameters": {
    "query": "clawdius",
    "limit": 5
  }
}
```

### Resource Access
MCP resources are accessed through the context system:

```json
{
  "resource": "mcp.github-mcp.repo",
  "uri": "clawdius/clawdius",
  "operation": "read"
}
```

## Security Model

### Sandboxing
MCP servers run in isolated processes with:
- No direct filesystem access (except configured paths)
- Network access only to configured endpoints
- No access to Clawdius internals

### Configuration Secrets
Secrets (like API tokens) are:
- Stored in system keyring
- Passed via environment variables
- Never logged or persisted in plaintext

### Permission Prompts
When an MCP server requests a new permission:
```
⚠️ github-mcp is requesting:
  - Network access to api.github.com

Allow? [y/N/a(always)]
```

## Development

### Creating an MCP Server

1. **Create the manifest:**
   ```bash
   clawdius mcp init my-server
   ```

2. **Implement the server:**
   ```typescript
   // src/index.ts
   import { MCPServer } from '@clawdius/mcp-sdk';
   
   const server = new MCPServer({
     name: 'my-server',
     version: '1.0.0',
   });
   
   server.tool('my_tool', {
     description: 'My custom tool',
     parameters: {
       input: { type: 'string', description: 'Input parameter' }
     }
   }, async (params) => {
     return { result: `Processed: ${params.input}` };
   });
   
   server.start();
   ```

3. **Test locally:**
   ```bash
   clawdius mcp dev ./my-server
   ```

4. **Package and publish:**
   ```bash
   clawdius mcp pack ./my-server
   clawdius mcp publish ./my-server
   ```

## API Reference

### MarketplaceClient

```rust
let client = MarketplaceClient::new(config);

// Search servers
let results = client.search("github").await?;

// Get server details
let server = client.get_server("github-mcp").await?;

// Install server
let install_result = client.install("github-mcp", version).await?;
```

### ServerRegistry

```rust
let registry = ServerRegistry::new(&config);

// List installed servers
let servers = registry.list().await?;

// Get server process
let process = registry.get_process("github-mcp").await?;

// Start/stop server
registry.start("github-mcp").await?;
registry.stop("github-mcp").await?;
```

## Troubleshooting

### Server Won't Start
1. Check logs: `clawdius mcp logs github-mcp`
2. Verify configuration: `clawdius mcp config github-mcp`
3. Check permissions: `clawdius mcp permissions github-mcp`

### Connection Issues
1. Verify network access is allowed
2. Check API endpoint is reachable
3. Verify authentication credentials

### Performance Issues
1. Check server resource usage: `clawdius mcp stats github-mcp`
2. Consider caching responses
3. Adjust timeout settings

## Future Enhancements
- [ ] WebAssembly-based MCP servers
- [ ] Hot-reload for development
- [ ] Distributed MCP server support
- [ ] Built-in telemetry and metrics
- [ ] Automatic schema generation
