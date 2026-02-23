# A3sfile.hcl — a3s local development orchestration
# Run: a3s up

dev {
  proxy_port = 7080
  log_level  = "info"
}

# ── Event bus (no dependencies) ──────────────────────────────────────────────
service "event" {
  cmd       = "cargo run -p a3s-event"
  dir       = "crates/event"
  port      = 4222
  subdomain = "event"

  health {
    type     = "tcp"
    interval = "2s"
    timeout  = "1s"
    retries  = 5
  }
}

# ── LLM inference (no dependencies) ──────────────────────────────────────────
service "power" {
  cmd       = "cargo run -p a3s-power"
  dir       = "crates/power"
  port      = 11434
  subdomain = "power"

  env = {
    RUST_LOG = "info"
  }

  watch {
    paths   = ["crates/power/src"]
    ignore  = ["target"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/api/health"
    interval = "3s"
    timeout  = "2s"
    retries  = 5
  }
}

# ── Search engine (no dependencies) ──────────────────────────────────────────
service "search" {
  cmd       = "cargo run -p a3s-search"
  dir       = "crates/search"
  port      = 8081
  subdomain = "search"

  watch {
    paths   = ["crates/search/src"]
    ignore  = ["target"]
    restart = true
  }

  health {
    type     = "tcp"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

# ── Lane scheduler (depends on event) ────────────────────────────────────────
service "lane" {
  cmd        = "cargo run -p a3s-lane"
  dir        = "crates/lane"
  port       = 8082
  subdomain  = "lane"
  depends_on = ["event"]

  watch {
    paths   = ["crates/lane/src"]
    ignore  = ["target"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/health"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

# ── SafeClaw security proxy (depends on power) ────────────────────────────────
service "safeclaw" {
  cmd        = "cargo run -p a3s-safeclaw"
  dir        = "crates/safeclaw"
  port       = 8083
  subdomain  = "safeclaw"
  depends_on = ["power"]

  watch {
    paths   = ["crates/safeclaw/src"]
    ignore  = ["target"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/health"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}
