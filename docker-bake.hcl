group "default" {
  targets = ["clawdius"]
}

target "clawdius" {
  dockerfile = "Dockerfile"
  tags = ["ghcr.io/clawdius/clawdius:1.0.0-rc.2"]
  platforms = ["linux/amd64", "linux/arm64"]
  cache-from = ["type=gha"]
  cache-to = ["type=gha,mode=max"]
}
