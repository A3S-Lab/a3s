---
name: a3s-box
description: Manage MicroVM containers with A3S Box — create, run, inspect, and clean up isolated sandboxes via the a3s-box CLI
allowed-tools: "bash(*)"
tags:
  - sandbox
  - microvm
  - containers
version: "1.0.0"
---
# A3S Box — MicroVM Sandbox

`a3s-box` is available on this system. Use the `bash` tool to run `a3s-box` commands for creating and managing hardware-isolated MicroVM containers.

## Core Commands

### Run a container
```bash
# Foreground (interactive)
a3s-box run --name <name> <image>

# Detached
a3s-box run -d --name <name> <image>

# With port mapping
a3s-box run -d --name <name> -p <host>:<container> <image>

# With environment variables
a3s-box run -d --name <name> -e KEY=VALUE <image>

# With volume mount
a3s-box run -d --name <name> -v /host/path:/container/path <image>
```

### List containers
```bash
a3s-box ps          # running only
a3s-box ps -a       # all (including stopped)
```

### Container lifecycle
```bash
a3s-box start <name|id>
a3s-box stop <name|id>
a3s-box restart <name|id>
a3s-box rm <name|id>          # remove stopped container
a3s-box rm -f <name|id>       # force remove running container
```

### Execute commands in a running container
```bash
a3s-box exec <name|id> <command>
a3s-box exec -it <name|id> bash    # interactive shell
```

### Inspect and logs
```bash
a3s-box inspect <name|id>
a3s-box logs <name|id>
a3s-box logs -f <name|id>     # follow
```

## Image Management

```bash
a3s-box pull <image>           # pull an image
a3s-box images                 # list local images
a3s-box rmi <image>            # remove an image
a3s-box image-prune            # remove unused images
```

## Network Management

```bash
a3s-box network ls
a3s-box network create --driver bridge <name>
a3s-box network rm <name>
```

## Volume Management

```bash
a3s-box volume ls
a3s-box volume create <name>
a3s-box volume rm <name>
a3s-box volume prune
```

## System

```bash
a3s-box info                   # system info (version, resources)
a3s-box df                     # disk usage
a3s-box system-prune --force   # remove all stopped containers, unused images and volumes
```

## Best Practices

1. **Always name containers** with `--name` for easy reference later.
2. **Use detached mode** (`-d`) for services that should keep running.
3. **Check if a container exists** with `a3s-box ps -a` before creating it.
4. **Clean up** with `a3s-box rm` after ephemeral containers are no longer needed.
5. **Prefer specific image tags** over `latest` for reproducibility.
