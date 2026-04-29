# Clawdius Hetzner Cloud Terraform Module
#
# Provisions infrastructure on Hetzner Cloud for running Clawdius.
# Creates a server, firewall, and optional floating IP.
#
# Usage:
#   module "clawdius" {
#     source       = "./terraform/hetzner"
#     hcloud_token = var.hcloud_token
#     server_name  = "clawdius"
#     server_type  = "cax11"
#     location     = "fsn1"
#   }

variable "hcloud_token" {
  description = "Hetzner Cloud API token"
  type        = string
  sensitive   = true
}

variable "server_name" {
  description = "Server hostname"
  type        = string
  default     = "clawdius"
}

variable "server_type" {
  description = "Hetzner server type (cax11 = 2 vCPU, 4GB RAM)"
  type        = string
  default     = "cax11"
}

variable "location" {
  description = "Hetzner datacenter location"
  type        = string
  default     = "fsn1"
}

variable "image" {
  description = "Server OS image"
  type        = string
  default     = "debian-12"
}

variable "ssh_keys" {
  description = "List of SSH key names to deploy"
  type        = list(string)
  default     = []
}

variable "enable_firewall" {
  description = "Create firewall rules"
  type        = bool
  default     = true
}

variable "allowed_ssh_cidrs" {
  description = "CIDRs allowed SSH access"
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "allowed_http_cidrs" {
  description = "CIDRs allowed HTTP/HTTPS access"
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "floating_ip" {
  description = "Create a floating IP and attach to server"
  type        = bool
  default     = false
}

variable "labels" {
  description = "Resource labels"
  type        = map(string)
  default     = { app = "clawdius", env = "production" }
}

terraform {
  required_version = ">= 1.5"
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = "~> 1.45"
    }
  }
}

provider "hcloud" {
  token = var.hcloud_token
}

# ─── SSH Key (if provided) ────────────────────────────────
# Note: SSH keys must be created in the Hetzner console first

# ─── Firewall ─────────────────────────────────────────────
resource "hcloud_firewall" "clawdius" {
  count = var.enable_firewall ? 1 : 0
  name  = "${var.server_name}-firewall"

  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "22"
    source_ips = var.allowed_ssh_cidrs
    description = "SSH"
  }

  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "80"
    source_ips = var.allowed_http_cidrs
    description = "HTTP (health/webhook)"
  }

  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "443"
    source_ips = var.allowed_http_cidrs
    description = "HTTPS"
  }

  rule {
    direction = "out"
    protocol  = "tcp"
    port      = "any"
    source_ips = ["0.0.0.0/0"]
    description = "Outbound TCP"
  }

  rule {
    direction = "out"
    protocol  = "udp"
    port      = "any"
    source_ips = ["0.0.0.0/0"]
    description = "Outbound UDP"
  }

  rule {
    direction = "out"
    protocol  = "icmp"
    source_ips = ["0.0.0.0/0"]
    description = "Outbound ICMP"
  }
}

# ─── Server ──────────────────────────────────────────────
resource "hcloud_server" "clawdius" {
  name        = var.server_name
  server_type = var.server_type
  location    = var.location
  image       = var.image
  ssh_keys    = var.ssh_keys

  labels = var.labels

  firewall_ids = var.enable_firewall ? [hcloud_firewall.clawdius[0].id] : []

  user_data = <<-EOF
    #!/bin/bash
    set -euo pipefail

    # Install Docker
    apt-get update
    apt-get install -y ca-certificates curl gnupg
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

    # Create clawdius user
    useradd -r -m -s /bin/bash clawdius
    usermod -aG docker clawdius

    # Create directories
    mkdir -p /opt/clawdius /home/clawdius/.clawdius
    chown -R clawdius:clawdius /home/clawdius/.clawdius /opt/clawdius

    # Install kubectl (for K3s)
    curl -fsSL https://raw.githubusercontent.com/kubernetes/release/v1.31.0/bin/linux/amd64/kubectl -o /usr/local/bin/kubectl
    chmod +x /usr/local/bin/kubectl

    echo "Clawdius server initialized successfully"
  EOF

  # Prevent destruction of server (use lifecycle block to handle)
  lifecycle {
    prevent_destroy = false
  }
}

# ─── Floating IP ─────────────────────────────────────────
resource "hcloud_floating_ip" "clawdius" {
  count          = var.floating_ip ? 1 : 0
  name           = "${var.server_name}-ip"
  server_id      = hcloud_server.clawdius.id
  type           = "ipv4"
  home_location = var.location
}

# ─── Outputs ─────────────────────────────────────────────
output "server_ip" {
  description = "Server IPv4 address"
  value       = hcloud_server.clawdius.ipv4_address
}

output "floating_ip" {
  description = "Floating IP (if created)"
  value       = var.floating_ip ? hcloud_floating_ip.clawdius[0].ip_address : null
}

output "server_name" {
  description = "Server hostname"
  value       = hcloud_server.clawdius.name
}

output "ssh_command" {
  description = "SSH connection command"
  value       = "ssh root@${hcloud_server.clawdius.ipv4_address}"
}
