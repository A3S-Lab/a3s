#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
    echo "This helper must run as root." >&2
    exit 1
fi

# Propagate cpuset through the existing cgroup v2 hierarchy. Delegate= is not
# a runtime-settable systemd property on all supported releases, while the
# root helper can safely enable this controller before handing the subtree to
# the unprivileged executor.
required_controllers="cpu cpuset memory pids"

for controller_dir in \
    /sys/fs/cgroup \
    /sys/fs/cgroup/user.slice \
    /sys/fs/cgroup/user.slice/user-1000.slice \
    /sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service; do
    available=$(cat "${controller_dir}/cgroup.controllers")
    case " ${available} " in
        *" cpuset "*) ;;
        *) echo "Missing cpuset at ${controller_dir}; available: ${available}" >&2; exit 1 ;;
    esac
    case " $(cat "${controller_dir}/cgroup.subtree_control") " in
        *" cpuset "*) ;;
        *) printf '%s\n' '+cpuset' > "${controller_dir}/cgroup.subtree_control" ;;
    esac
done

enable_required_controllers() {
    controller_dir=$1
    available=$(cat "${controller_dir}/cgroup.controllers")
    for controller in $required_controllers; do
        case " ${available} " in
            *" ${controller} "*) ;;
            *) echo "Missing ${controller} at ${controller_dir}; available: ${available}" >&2; exit 1 ;;
        esac
    done
    printf '%s\n' '+cpu +cpuset +memory +pids' > "${controller_dir}/cgroup.subtree_control"
    enabled=$(cat "${controller_dir}/cgroup.subtree_control")
    for controller in $required_controllers; do
        case " ${enabled} " in
            *" ${controller} "*) ;;
            *) echo "Controller ${controller} was not enabled at ${controller_dir}" >&2; exit 1 ;;
        esac
    done
}

user_cgroup=/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service
enable_required_controllers "$user_cgroup"

install_snapshot() {
    source=$1
    destination=$2
    mode=$3
    if [ -f "$destination" ] && cmp -s "$source" "$destination"; then
        chown root:root "$destination"
        chmod "$mode" "$destination"
    else
        install -o root -g root -m "$mode" "$source" "$destination"
    fi
}

# A privileged launcher must never execute a user-writable runtime. Install an
# administrator-owned snapshot before enabling the restricted setuid entrypoint.
launcher_src=/home/roylin/桌面/code/a3s/a3s-box-sandbox-oci-launcher
launcher_dst=/usr/local/libexec/a3s-box-sandbox-oci-launcher
# Prefer the freshly built runtime in the checkout; fall back to the installed
# artifact so the helper remains usable after a clean checkout.
runtime_src=/home/roylin/桌面/code/a3s/crates/oci-runtime/target/release/a3s-oci
if [ ! -x "${runtime_src}" ]; then
    runtime_src=/home/roylin/.local/share/a3s-box/a3s-oci
fi
agent_src=/home/roylin/.local/share/a3s-box/a3s-oci-agent
shim_src=/home/roylin/.local/share/a3s-box/a3s-box-shim
runtime_dir=/usr/local/libexec/a3s-box-sandbox
box_src=/home/roylin/桌面/code/a3s/crates/box/src/target/release/a3s-box
box_dst=/usr/local/libexec/a3s-box-sandbox-cli
if [ ! -x "${launcher_src}" ] || [ ! -x "${runtime_src}" ] || [ ! -x "${agent_src}" ] || [ ! -x "${shim_src}" ] || [ ! -x "${box_src}" ]; then
    echo "missing Sandbox launcher, runtime, or CLI artifact" >&2
    exit 1
fi
mkdir -p "$(dirname "${launcher_dst}")"
install -d -o root -g root -m 0755 "$runtime_dir"
# Older helper revisions copied the shim beside the protected CLI. Remove that
# copy so the loader can use the packaged shim together with its matching
# libkrun directory.
rm -f /usr/local/libexec/a3s-box-shim
install_snapshot "$runtime_src" "$runtime_dir/a3s-oci" 0755
install_snapshot "$agent_src" "$runtime_dir/a3s-oci-agent" 0755
install_snapshot "$box_src" "$box_dst" 0755
install_snapshot "$launcher_src" "$launcher_dst" 4755

# The system service cgroup is owned by systemd. Move the launcher into the
# user's already delegated cgroup tree before dropping privileges.
cgroup_dir=/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/a3s-box-delegated
mkdir -p "${cgroup_dir}"
# The delegation root itself must stay empty. Place the host-service launcher
# in a child so the runtime can create its execution cgroups alongside it.
launcher_dir="${cgroup_dir}/launcher-$$"
mkdir "${launcher_dir}"
printf '%s\n' "$$" > "${launcher_dir}/cgroup.procs"

# Refuse to launch if any required controller is unavailable or not enabled.
enable_required_controllers "$cgroup_dir"
echo "sandbox helper cgroup=${cgroup_dir} controllers=${available} enabled=${enabled}" >&2

# systemd owns the service cgroup initially; transfer it before dropping
# privileges so the rootless runtime can create its private manager below it.
chown roylin:roylin \
    "${cgroup_dir}" \
    "${cgroup_dir}/cgroup.procs" \
    "${cgroup_dir}/cgroup.subtree_control" \
    "${launcher_dir}" \
    "${launcher_dir}/cgroup.procs" \
    "${launcher_dir}/cgroup.subtree_control"

exec /usr/bin/setpriv \
    --reuid=roylin \
    --regid=roylin \
    --clear-groups \
    /usr/bin/env \
    A3S_BOX_OCI_RUNTIME_PATH="$runtime_dir/a3s-oci" \
    A3S_BOX_OCI_AGENT_PATH="$runtime_dir/a3s-oci-agent" \
    PATH="/home/roylin/.local/share/a3s-box:/usr/local/libexec:/usr/sbin:/usr/bin:/sbin:/bin" \
    "$box_dst" "$@"
