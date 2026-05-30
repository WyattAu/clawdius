group "default" {
  targets = ["clawdius", "clawdius-gateway", "clawdius-cli"]
}

target "clawdius" {
  dockerfile = "Dockerfile"
  tags = ["ghcr.io/clawdius/clawdius:1.0.0-rc.2"]
  platforms = ["linux/amd64", "linux/arm64"]
  cache-from = ["type=gha"]
  cache-to = ["type=gha,mode=max"]
}

target "clawdius-gateway" {
  dockerfile = "Dockerfile.gateway"
  tags = ["ghcr.io/clawdius/clawdius-gateway:1.0.0-rc.2"]
  platforms = ["linux/amd64", "linux/arm64"]
  cache-from = ["type=gha"]
  cache-to = ["type=gha,mode=max"]
}

target "clawdius-cli" {
  dockerfile = "Dockerfile.cli"
  tags = ["ghcr.io/clawdius/clawdius-cli:1.0.0-rc.2"]
  platforms = ["linux/amd64", "linux/arm64"]
  cache-from = ["type=gha"]
  cache-to = ["type=gha,mode=max"]
}
