#!/bin/sh
set -eu

# Run a3s-box's native sandbox inside a delegated system service.  The
# system-level manager is required because an ordinary user cannot clear the
# supplementary groups assigned by the login session.
REPO_ROOT=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
OPERATOR_NAME=${SUDO_USER:-$(id -un)}
OPERATOR_HOME=$(getent passwd "${OPERATOR_NAME}" | cut -d: -f6)
HELPER="${REPO_ROOT}/a3s-box-sandbox-root-helper.sh"

if [ -z "${OPERATOR_HOME}" ] || [ "${OPERATOR_HOME#/}" = "${OPERATOR_HOME}" ]; then
    echo "Could not resolve an absolute home directory for ${OPERATOR_NAME}." >&2
    exit 1
fi

if [ "$(id -u)" -ne 0 ]; then
    exec sudo \
        --preserve-env=A3S_BOX_SANDBOX_REPO_ROOT \
        A3S_BOX_SANDBOX_REPO_ROOT="${REPO_ROOT}" \
        "$0" "$@"
fi

if ! command -v newuidmap >/dev/null 2>&1 || ! command -v newgidmap >/dev/null 2>&1; then
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y uidmap
    else
        echo "Missing newuidmap/newgidmap; install the distribution's uidmap package first." >&2
        exit 1
    fi
fi

exec systemd-run \
    --system \
    --wait \
    --collect \
    --pipe \
    --property=Delegate=yes \
    --property=CPUAccounting=yes \
    --property=MemoryAccounting=yes \
    --property=TasksAccounting=yes \
    --property=WorkingDirectory="${REPO_ROOT}" \
    --property=Environment=HOME="${OPERATOR_HOME}" \
    --property=Environment=USER="${OPERATOR_NAME}" \
    --property=Environment=LOGNAME="${OPERATOR_NAME}" \
    --property=Environment=A3S_BOX_SANDBOX_REPO_ROOT="${REPO_ROOT}" \
    "${HELPER}" "$@"
