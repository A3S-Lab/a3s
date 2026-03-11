#!/usr/bin/env bash
# deploy-openclaw.sh — One-click OpenClaw deployment via a3s box
#
# Usage:
#   ./scripts/deploy-openclaw.sh            # Deploy (onboard if first time, then start)
#   ./scripts/deploy-openclaw.sh --start    # Start gateway only (skip onboarding)
#   ./scripts/deploy-openclaw.sh --stop     # Stop gateway
#   ./scripts/deploy-openclaw.sh --clean    # Stop and remove container + image
#   ./scripts/deploy-openclaw.sh --status   # Show running status
#   ./scripts/deploy-openclaw.sh --logs     # Tail live application logs
#
# Requirements:
#   - a3s-box installed (brew tap a3s-lab/tap && brew install a3s-box)
#
# Networking note (macOS):
#   a3s-box TSI mode (macOS default) only configures the loopback interface
#   inside the MicroVM — there is no eth0. This means `-p` port mapping does
#   NOT create a host-side listener on macOS. The gateway IS running inside
#   the VM and can make outbound connections (LLM API calls work fine), but
#   it is not reachable from the host via TCP.
#
#   Bridge networking (which would expose the port) requires `passt`, a
#   Linux-only tool. Until a3s-box ships macOS bridge support, the gateway
#   is accessed indirectly (e.g. via the SafeClaw desktop app's vsock
#   channel, or by running on Linux where passt is available).
#
#   This script still correctly starts the container and verifies that the
#   OpenClaw Node.js process has fully initialised by watching the VM's
#   console log (not by polling the HTTP endpoint).

set -euo pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

CONTAINER_NAME="openclaw-gateway"
IMAGE="ghcr.io/openclaw/openclaw:latest"
GATEWAY_PORT=18789
OPENCLAW_HOME="${HOME}/.openclaw"
WORKSPACE="${HOME}/openclaw/workspace"

# Sentinel string emitted by OpenClaw when the gateway is fully ready
READY_MARKER="host mounted at"

STARTUP_TIMEOUT=120  # seconds to wait for the ready marker
STARTUP_INTERVAL=2   # log polling interval in seconds

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_step()  { echo -e "\n${BOLD}▶ $*${NC}"; }

# ---------------------------------------------------------------------------
# Prerequisites
# ---------------------------------------------------------------------------

check_prereqs() {
    log_step "Checking prerequisites"

    if ! command -v a3s-box &>/dev/null; then
        log_error "a3s-box not found."
        echo "  Install: brew tap a3s-lab/tap && brew install a3s-box"
        exit 1
    fi
    log_ok "a3s-box: $(a3s-box version 2>/dev/null | head -1 || echo 'found')"
}

# ---------------------------------------------------------------------------
# Image
# ---------------------------------------------------------------------------

pull_image() {
    log_step "Pulling OpenClaw image"
    log_info "Image: ${IMAGE}"
    a3s-box pull "${IMAGE}"
    log_ok "Image ready"
}

# ---------------------------------------------------------------------------
# Directories
# ---------------------------------------------------------------------------

setup_dirs() {
    log_step "Setting up data directories"
    mkdir -p "${OPENCLAW_HOME}" "${WORKSPACE}"
    log_ok "Config:    ${OPENCLAW_HOME}"
    log_ok "Workspace: ${WORKSPACE}"
}

# ---------------------------------------------------------------------------
# Onboarding (first-time interactive setup)
# ---------------------------------------------------------------------------

needs_onboarding() {
    [[ -z "$(ls -A "${OPENCLAW_HOME}" 2>/dev/null)" ]]
}

run_onboarding() {
    log_step "First-time onboarding"
    log_info "OpenClaw will ask for your LLM API key and channel configuration."
    log_info "Container: openclaw-onboard (removed automatically when done)"
    echo ""

    a3s-box run -it --rm \
        --name "openclaw-onboard" \
        -v "${OPENCLAW_HOME}:/home/node/.openclaw" \
        -v "${WORKSPACE}:/home/node/workspace" \
        "${IMAGE}" -- openclaw onboard

    log_ok "Onboarding complete. Config saved to ${OPENCLAW_HOME}"
}

# ---------------------------------------------------------------------------
# Container lifecycle
# ---------------------------------------------------------------------------

container_exists() {
    a3s-box ps --all 2>/dev/null | grep -qw "${CONTAINER_NAME}"
}

container_running() {
    a3s-box ps 2>/dev/null | grep -qw "${CONTAINER_NAME}"
}

stop_existing() {
    if container_running; then
        log_info "Stopping existing container: ${CONTAINER_NAME}"
        a3s-box stop "${CONTAINER_NAME}"
    fi
    if container_exists; then
        log_info "Removing existing container: ${CONTAINER_NAME}"
        a3s-box rm "${CONTAINER_NAME}"
    fi
}

# Return the path to this container's console.log (written by the VM host process)
console_log_path() {
    local box_dir
    box_dir=$(a3s-box inspect "${CONTAINER_NAME}" 2>/dev/null | \
        python3 -c "import sys,json; print(json.load(sys.stdin)['box_dir'])" 2>/dev/null || true)
    echo "${box_dir}/logs/console.log"
}

start_gateway() {
    log_step "Starting OpenClaw gateway"

    # TSI mode (macOS default): only loopback is configured inside the VM.
    # The -p flag is kept for Linux compatibility (where passt enables bridge
    # networking and inbound port mapping works). On macOS it is a no-op for
    # inbound traffic but harmless to include.
    a3s-box run -d \
        --name "${CONTAINER_NAME}" \
        --cpus 2 \
        --memory 1g \
        -p "${GATEWAY_PORT}:${GATEWAY_PORT}" \
        -v "${OPENCLAW_HOME}:/home/node/.openclaw" \
        -v "${WORKSPACE}:/home/node/workspace" \
        -e "OPENCLAW_ALLOW_INSECURE_PRIVATE_WS=1" \
        --restart on-failure:3 \
        --no-healthcheck \
        "${IMAGE}"

    log_ok "Container started: ${CONTAINER_NAME}"
}

# ---------------------------------------------------------------------------
# Startup verification — watch console.log for the ready marker
# ---------------------------------------------------------------------------

wait_ready() {
    log_step "Waiting for OpenClaw to initialise"

    local log_path elapsed
    log_path=$(console_log_path)
    elapsed=0

    if [[ -z "${log_path}" || "${log_path}" == "/logs/console.log" ]]; then
        log_warn "Cannot determine console.log path — skipping startup check"
        return 0
    fi

    log_info "Watching: ${log_path}"
    printf "  Starting"

    while (( elapsed < STARTUP_TIMEOUT )); do
        if ! container_running; then
            echo ""
            log_error "Container exited unexpectedly"
            [[ -f "${log_path}" ]] && tail -30 "${log_path}" | grep -v "a3s_box_guest_init" || true
            exit 1
        fi

        if [[ -f "${log_path}" ]] && grep -q "${READY_MARKER}" "${log_path}" 2>/dev/null; then
            echo ""
            log_ok "Gateway is ready (${elapsed}s)"
            return 0
        fi

        printf "."
        sleep "${STARTUP_INTERVAL}"
        (( elapsed += STARTUP_INTERVAL ))
    done

    echo ""
    log_warn "Ready marker not seen within ${STARTUP_TIMEOUT}s — container may still be starting"
    [[ -f "${log_path}" ]] && grep -v "a3s_box_guest_init" "${log_path}" | tail -10 || true
}

# ---------------------------------------------------------------------------
# Status summary
# ---------------------------------------------------------------------------

show_status() {
    local log_path
    log_path=$(console_log_path)

    echo ""
    echo -e "${GREEN}${BOLD}  OpenClaw is running via a3s box${NC}"
    echo "  ─────────────────────────────────────────────"
    echo ""
    a3s-box ps 2>/dev/null | grep -E "NAME|${CONTAINER_NAME}" || true
    echo ""
    echo -e "  ${BOLD}Application logs (VM console):${NC}"
    echo "    ${log_path}"
    echo ""
    echo -e "  ${BOLD}Networking note (macOS):${NC}"
    echo "    TSI mode — only loopback is configured inside the MicroVM."
    echo "    Port ${GATEWAY_PORT} is NOT reachable from the host on macOS."
    echo "    On Linux (where passt is available), bridge mode enables"
    echo "    inbound port mapping and http://127.0.0.1:${GATEWAY_PORT}/ works."
    echo ""
    echo -e "  ${BOLD}Useful commands:${NC}"
    echo "    $0 --logs              # tail live application logs"
    echo "    $0 --stop              # stop gateway"
    echo "    $0 --clean             # stop and remove container + image"
    echo "    a3s-box ps             # list all containers"
    echo "    a3s-box stats ${CONTAINER_NAME}   # resource usage"
    echo ""
}

show_app_logs() {
    log_step "OpenClaw application logs"
    local log_path
    log_path=$(console_log_path)

    if [[ -z "${log_path}" || ! -f "${log_path}" ]]; then
        log_error "Console log not found. Is the container running?"
        exit 1
    fi

    log_info "Source: ${log_path}"
    echo ""
    # Strip guest-init lines; show only OpenClaw app output
    grep -v "a3s_box_guest_init" "${log_path}" || true
    echo ""
    log_info "Tailing… (Ctrl-C to stop)"
    tail -f "${log_path}" | grep --line-buffered -v "a3s_box_guest_init"
}

show_running_status() {
    log_step "OpenClaw status"
    if container_running; then
        log_ok "Container is running"
        a3s-box ps 2>/dev/null | grep -E "NAME|${CONTAINER_NAME}" || true
        echo ""
        local log_path
        log_path=$(console_log_path)
        if [[ -f "${log_path}" ]]; then
            log_info "Recent application output:"
            grep -v "a3s_box_guest_init" "${log_path}" | tail -15 || true
        fi
    else
        log_warn "Container is not running"
        if container_exists; then
            log_info "Container exists but is stopped. Run: $0 --start"
        fi
    fi
}

# ---------------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------------

do_clean() {
    log_step "Cleaning up"
    stop_existing
    log_info "Removing image: ${IMAGE}"
    a3s-box rmi "${IMAGE}" 2>/dev/null || log_warn "Image already removed or not found"
    log_ok "Cleanup complete"
    log_warn "Config data preserved at: ${OPENCLAW_HOME}"
    log_warn "To also remove config: rm -rf ${OPENCLAW_HOME} ${WORKSPACE}"
}

# ---------------------------------------------------------------------------
# Entrypoint
# ---------------------------------------------------------------------------

print_banner() {
    echo ""
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║   OpenClaw × a3s box  ·  deploy.sh   ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo ""
}

main() {
    local mode="${1:-deploy}"

    print_banner

    case "${mode}" in
        --stop)
            check_prereqs
            log_step "Stopping OpenClaw gateway"
            if container_running; then
                a3s-box stop "${CONTAINER_NAME}"
                log_ok "Stopped: ${CONTAINER_NAME}"
            else
                log_warn "Container is not running"
            fi
            ;;

        --clean)
            check_prereqs
            do_clean
            ;;

        --status)
            check_prereqs
            show_running_status
            ;;

        --logs)
            check_prereqs
            show_app_logs
            ;;

        --start)
            check_prereqs
            setup_dirs
            pull_image
            stop_existing
            start_gateway
            wait_ready
            show_status
            ;;

        deploy|"")
            check_prereqs
            setup_dirs
            pull_image
            stop_existing

            if needs_onboarding; then
                run_onboarding
            else
                log_info "Existing config detected — skipping onboarding"
            fi

            start_gateway
            wait_ready
            show_status
            ;;

        *)
            echo "Usage: $0 [--start|--stop|--clean|--status|--logs]"
            echo ""
            echo "  (no flag)   Full deploy: onboard if needed, then start"
            echo "  --start     Start gateway only (skip onboarding)"
            echo "  --stop      Stop running gateway"
            echo "  --clean     Stop, remove container and image"
            echo "  --status    Show status and recent app output"
            echo "  --logs      Tail live application logs"
            exit 1
            ;;
    esac
}

main "${@:-}"
