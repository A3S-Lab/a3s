#!/bin/sh
set -eu

# Run a3s-box's native sandbox inside a delegated system service.  The
# system-level manager is required because an ordinary user cannot clear the
# supplementary groups assigned by the login session.
if [ "$(id -u)" -ne 0 ]; then
    exec sudo "$0" "$@"
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
    --property=WorkingDirectory=/home/roylin/桌面/code/a3s \
    --property=Environment=HOME=/home/roylin \
    --property=Environment=USER=roylin \
    --property=Environment=LOGNAME=roylin \
    /home/roylin/桌面/code/a3s/a3s-box-sandbox-root-helper.sh "$@"
