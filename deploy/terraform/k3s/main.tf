# Clawdius K3s Terraform Module
#
# Deploys K3s on a Hetzner server for running Clawdius
# in a Kubernetes environment.
#
# Usage:
#   module "k3s" {
#     source       = "./terraform/k3s"
#     hcloud_token = var.hcloud_token
#     server_name  = "clawdius-k3s"
#     server_type  = "cax21"  # 4 vCPU, 8GB RAM
#   }

variable "hcloud_token" {
  description = "Hetzner Cloud API token"
  type        = string
  sensitive   = true
}

variable "server_name" {
  description = "Server hostname"
  type        = string
  default     = "clawdius-k3s"
}

variable "server_type" {
  description = "Hetzner server type (recommend cax21 for K3s)"
  type        = string
  default     = "cax21"
}

variable "location" {
  description = "Hetzner datacenter location"
  type        = string
  default     = "fsn1"
}

variable "ssh_keys" {
  description = "SSH key names for server access"
  type        = list(string)
  default     = []
}

variable "k3s_version" {
  description = "K3s version to install"
  type        = string
  default     = "v1.31.2+k3s1"
}

variable "cluster_token" {
  description = "K3s cluster token (auto-generated if empty)"
  type        = string
  default     = ""
}

terraform {
  required_version = ">= 1.5"
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = "~> 1.45"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
  }
}

provider "hcloud" {
  token = var.hcloud_token
}

resource "random_password" "cluster_token" {
  length  = 32
  special = false
}

locals {
  cluster_token = var.cluster_token != "" ? var.cluster_token : random_password.cluster_token.result
}

# ─── K3s Server ──────────────────────────────────────────
resource "hcloud_server" "k3s_server" {
  name        = var.server_name
  server_type = var.server_type
  location    = var.location
  image       = "debian-12"
  ssh_keys    = var.ssh_keys

  labels = {
    app  = "clawdius"
    role = "k3s-server"
  }

  user_data = <<-EOF
    #!/bin/bash
    set -euo pipefail

    # Install K3s server
    curl -sfL https://get.k3s.io | INSTALL_K3S_VERSION=${var.k3s_version} K3S_TOKEN=${local.cluster_token} sh -

    # Wait for K3s to be ready
    for i in $(seq 1 30); do
      if kubectl get nodes >/dev/null 2>&1; then
        echo "K3s is ready"
        break
      fi
      echo "Waiting for K3s... ($i/30)"
      sleep 2
    done

    # Label node
    kubectl label node $(hostname) app=clawdius role=server --overwrite

    echo "K3s server installed successfully"
    echo "Cluster token: ${local.cluster_token}"
  EOF
}

# ─── Outputs ─────────────────────────────────────────────
output "server_ip" {
  description = "K3s server IPv4 address"
  value       = hcloud_server.k3s_server.ipv4_address
}

output "cluster_token" {
  description = "K3s cluster token for joining agents"
  value       = local.cluster_token
  sensitive   = true
}

output "kubeconfig_command" {
  description = "Command to get kubeconfig"
  value       = "ssh root@${hcloud_server.k3s_server.ipv4_address} 'cat /etc/rancher/k3s/k3s.yaml'"
}

output "deploy_clawdius" {
  description = "Command to deploy Clawdius via Helm"
  value       = "kubectl apply -f deploy/helm/clawdius/"
}
