# A3sfile.hcl — deploy a3s-gateway + whoami to OrbStack k8s
#
# Usage:
#   cd crates/dev/demo
#   a3s up

dev {
  proxy_port = 7080
  log_level  = "info"
}

# ── whoami ────────────────────────────────────────────────────────────────────
service "whoami" {
  cmd  = "kubectl rollout status deployment/whoami -n default --timeout=60s"
  port = 80

  health {
    type    = "tcp"
    timeout = "2s"
    retries = 10
  }
}

# ── a3s-gateway ───────────────────────────────────────────────────────────────
service "gateway" {
  cmd        = "kubectl rollout status deployment/a3s-gateway -n default --timeout=60s"
  port       = 30080
  subdomain  = "gateway"
  depends_on = ["whoami"]

  health {
    type    = "http"
    path    = "/"
    timeout = "2s"
    retries = 10
  }
}
