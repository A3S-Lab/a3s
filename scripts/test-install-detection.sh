#!/bin/sh
# Hermetic tests for installer channel resolution and conflict planning.
# No network; mocks brew via PATH.
set -eu
root=$(CDPATH=; cd -P "$(dirname "$0")/.." && pwd -P)
lib="$root/scripts/a3s-install-lib.sh"
installer="$root/install.sh"
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}
pass() {
    printf 'ok: %s\n' "$*"
}

# shellcheck source=/dev/null
. "$lib"

# --- resolve_channel ---
a3s_resolve_channel auto 1
[ "$a3s_resolved_channel" = brew ] || fail "auto+brew -> brew"
pass "auto prefers brew when present"

a3s_resolve_channel auto 0
[ "$a3s_resolved_channel" = binary ] || fail "auto-no-brew -> binary"
pass "auto falls back to binary without brew"

a3s_resolve_channel binary 1
[ "$a3s_resolved_channel" = binary ] || fail "explicit binary"
pass "explicit binary channel"

a3s_resolve_channel brew 1
[ "$a3s_resolved_channel" = brew ] || fail "explicit brew"
pass "explicit brew channel"

# Subshell so a3s_install_die's exit does not terminate the test runner.
if (a3s_resolve_channel brew 0) 2>/dev/null; then
    fail "brew without brew should die"
else
    pass "brew without Homebrew dies"
fi

# --- dry-run with mocked brew (formulae installed) ---
fake_bin="$tmp/bin"
mkdir -p "$fake_bin" "$tmp/local/bin" "$tmp/home"
cat >"$fake_bin/brew" <<'EOF'
#!/bin/sh
case "$1" in
    list)
        shift
        # brew list --versions a3s-code
        if [ "$1" = --versions ]; then
            case "$2" in
                a3s-code) echo "a3s-code 0.6.0"; exit 0 ;;
                a3s-webview) echo "a3s-webview 0.1.0"; exit 0 ;;
                a3s) exit 1 ;;
                *) exit 1 ;;
            esac
        fi
        ;;
esac
exit 1
EOF
chmod +x "$fake_bin/brew"

# Provide stub uname via overriding? We use real uname — fine on mac/linux CI.
# Force binary path tools and lib.
export PATH="$fake_bin:/usr/bin:/bin:/usr/sbin:/sbin"
export HOME="$tmp/home"
export A3S_INSTALL_LIB="$lib"
# Non-TTY yes is automatic when stdin not tty; ensure dry-run only.
out=$("$installer" --dry-run --channel auto 2>&1) || fail "dry-run auto exited non-zero"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: channel=brew' \
    || fail "dry-run auto should select brew: $out"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: uninstall_formula=a3s-code' \
    || fail "dry-run should plan uninstall a3s-code: $out"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: uninstall_formula=a3s-webview' \
    || fail "dry-run should plan uninstall a3s-webview: $out"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: scrub_local=1' \
    || fail "dry-run should scrub local: $out"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: action=brew_install_a3s' \
    || fail "dry-run should brew_install_a3s: $out"
pass "dry-run auto+brew plans conflict cleanup"

out=$("$installer" --dry-run --channel binary 2>&1) || fail "dry-run binary exited non-zero"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: channel=binary' \
    || fail "dry-run binary channel: $out"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: action=github_binary' \
    || fail "dry-run binary action: $out"
pass "dry-run binary selects github_binary"

# Without brew on PATH
export PATH="/usr/bin:/bin:/usr/sbin:/sbin"
out=$("$installer" --dry-run --channel auto 2>&1) || fail "dry-run no-brew exited non-zero"
printf '%s\n' "$out" | grep -Fq 'a3s-installer-plan: channel=binary' \
    || fail "auto without brew should be binary: $out"
pass "dry-run auto without brew uses binary"

# Local shadows reported
mkdir -p "$HOME/.local/bin"
touch "$HOME/.local/bin/a3s" "$HOME/.local/bin/a3s-webview"
export PATH="$fake_bin:/usr/bin:/bin"
out=$("$installer" --dry-run --channel auto 2>&1) || fail "dry-run local shadows exited non-zero"
printf '%s\n' "$out" | grep -Fq "a3s-installer-plan: local_file=$HOME/.local/bin/a3s" \
    || fail "should report local a3s: $out"
pass "dry-run reports ~/.local/bin shadows"

printf '\nAll installer detection tests passed.\n'
