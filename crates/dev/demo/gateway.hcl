entrypoints "web" {
  address = "0.0.0.0:8080"
}

routers "whoami" {
  rule    = "PathPrefix(`/`)"
  service = "whoami-backend"
}

services "whoami-backend" {
  load_balancer {
    strategy = "round-robin"
    servers = [
      { url = "http://whoami:80" }
    ]
  }
}
