#!/bin/sh
# Shared detection helpers for install.sh (sourced by the installer and tests).
# Keep this file POSIX sh and free of network I/O.

a3s_install_info() {
    printf 'a3s installer: %s\n' "$*"
}

a3s_install_warn() {
    printf 'a3s installer: warning: %s\n' "$*" >&2
}

a3s_install_die() {
    printf 'a3s installer: error: %s\n' "$*" >&2
    exit 1
}

a3s_install_plan() {
    printf 'a3s-installer-plan: %s\n' "$*"
}

# Sets: a3s_os a3s_arch a3s_target
a3s_detect_platform() {
    case "$(uname -s)" in
        Darwin) a3s_os=apple-darwin ;;
        Linux)
            a3s_os=unknown-linux-gnu
            glibc_detected=0
            if command -v getconf >/dev/null 2>&1 \
                && getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
                glibc_detected=1
            elif command -v ldd >/dev/null 2>&1 \
                && ldd --version 2>&1 | grep -Eqi '(glibc|gnu libc)'; then
                glibc_detected=1
            fi
            [ "$glibc_detected" -eq 1 ] \
                || a3s_install_die "the published Linux CLI requires glibc, which could not be verified on this host"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            a3s_install_die "use install.ps1 from PowerShell to install A3S on Windows"
            ;;
        *) a3s_install_die "unsupported operating system: $(uname -s)" ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64) a3s_arch=x86_64 ;;
        arm64|aarch64) a3s_arch=aarch64 ;;
        *) a3s_install_die "unsupported CPU architecture: $(uname -m)" ;;
    esac
    a3s_target="$a3s_arch-$a3s_os"
}

# Sets: a3s_brew_present (0|1)
a3s_detect_brew() {
    a3s_brew_present=0
    if command -v brew >/dev/null 2>&1; then
        a3s_brew_present=1
    fi
}

# brew_list_formula <name> -> 0 if installed
a3s_brew_formula_installed() {
    formula=$1
    [ "$a3s_brew_present" -eq 1 ] || return 1
    brew list --versions "$formula" >/dev/null 2>&1
}

# Resolve requested channel -> a3s_resolved_channel (brew|binary)
# Args: requested_channel (auto|brew|binary), brew_present (0|1)
a3s_resolve_channel() {
    requested=$1
    brew_present=$2
    case "$requested" in
        auto)
            if [ "$brew_present" -eq 1 ]; then
                a3s_resolved_channel=brew
            else
                a3s_resolved_channel=binary
            fi
            ;;
        brew)
            [ "$brew_present" -eq 1 ] \
                || a3s_install_die "--channel brew requires Homebrew on PATH"
            a3s_resolved_channel=brew
            ;;
        binary)
            a3s_resolved_channel=binary
            ;;
        *)
            a3s_install_die "invalid channel '$requested' (expected auto, brew, or binary)"
            ;;
    esac
}

# Print human inventory + machine-readable plan lines.
# Uses globals: a3s_brew_present, a3s_resolved_channel, HOME, DEFAULT_INSTALL_DIR optional
a3s_print_inventory_and_plan() {
    local_bin="${HOME:+$HOME/.local/bin}"
    a3s_install_info "platform target: $a3s_target"
    a3s_install_info "Homebrew present: $a3s_brew_present"
    a3s_install_plan "brew_present=$a3s_brew_present"
    a3s_install_plan "channel=$a3s_resolved_channel"

    if command -v a3s >/dev/null 2>&1; then
        a3s_install_info "existing a3s on PATH: $(command -v a3s)"
        a3s_install_plan "path_a3s=$(command -v a3s)"
    fi
    if command -v a3s-code >/dev/null 2>&1; then
        a3s_install_warn "legacy a3s-code on PATH: $(command -v a3s-code) (not the umbrella CLI)"
        a3s_install_plan "path_a3s_code=$(command -v a3s-code)"
    fi
    if command -v a3s-webview >/dev/null 2>&1; then
        a3s_install_info "existing a3s-webview on PATH: $(command -v a3s-webview)"
        a3s_install_plan "path_a3s_webview=$(command -v a3s-webview)"
    fi

    if [ "$a3s_brew_present" -eq 1 ]; then
        for formula in a3s a3s-code a3s-webview; do
            if a3s_brew_formula_installed "$formula"; then
                versions=$(brew list --versions "$formula" 2>/dev/null || true)
                a3s_install_info "Homebrew formula installed: $versions"
                a3s_install_plan "brew_formula=$formula"
            fi
        done
    fi

    if [ -n "$local_bin" ]; then
        for name in a3s a3s-webview a3s-code; do
            if [ -e "$local_bin/$name" ] || [ -L "$local_bin/$name" ]; then
                a3s_install_info "standalone file: $local_bin/$name"
                a3s_install_plan "local_file=$local_bin/$name"
            fi
        done
        if [ -d "$local_bin/moli" ]; then
            a3s_install_info "standalone moli dir: $local_bin/moli"
            a3s_install_plan "local_moli=$local_bin/moli"
        fi
    fi

    if command -v cargo >/dev/null 2>&1; then
        cargo_bin=$(cargo env 2>/dev/null | awk -F= '/CARGO_HOME/{gsub(/"/,""); print $2"/bin"}')
        if [ -z "$cargo_bin" ] && [ -n "${CARGO_HOME:-}" ]; then
            cargo_bin="$CARGO_HOME/bin"
        fi
        if [ -z "$cargo_bin" ] && [ -n "${HOME:-}" ]; then
            cargo_bin="$HOME/.cargo/bin"
        fi
        if [ -n "$cargo_bin" ]; then
            for name in a3s a3s-code; do
                if [ -e "$cargo_bin/$name" ] || [ -L "$cargo_bin/$name" ]; then
                    a3s_install_warn "Cargo install present: $cargo_bin/$name"
                    a3s_install_plan "cargo_bin=$cargo_bin/$name"
                fi
            done
        fi
    fi

    if [ "$a3s_resolved_channel" = brew ]; then
        a3s_install_plan "action=brew_install_a3s"
        if a3s_brew_formula_installed a3s-code; then
            a3s_install_plan "uninstall_formula=a3s-code"
        fi
        if a3s_brew_formula_installed a3s-webview; then
            a3s_install_plan "uninstall_formula=a3s-webview"
        fi
        a3s_install_plan "scrub_local=1"
        a3s_install_info "plan: clean legacy Homebrew conflicts, install a3s-lab/tap/a3s, scrub ~/.local shadows"
        a3s_install_info "interactive Code TUI is launched with: a3s code"
    else
        a3s_install_plan "action=github_binary"
        a3s_install_info "plan: install GitHub release binary into the configured install directory"
        a3s_install_info "interactive Code TUI is launched with: a3s code"
    fi
}

# Non-interactive brew install + conflict cleanup.
# Args: yes (0|1) — cargo uninstall when shadowing only if yes=1
a3s_install_via_brew() {
    yes=$1
    a3s_install_info "installing via Homebrew (a3s-lab/tap/a3s)"

    brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap \
        || a3s_install_die "failed to tap a3s-lab/tap"

    # Legacy product and old companion formula fight the umbrella keg.
    brew uninstall a3s-code >/dev/null 2>&1 || true
    brew uninstall a3s-webview >/dev/null 2>&1 || true

    if a3s_brew_formula_installed a3s; then
        a3s_install_info "reinstalling a3s to refresh links and companions"
        brew reinstall a3s-lab/tap/a3s \
            || a3s_install_die "brew reinstall a3s failed"
    else
        brew install a3s-lab/tap/a3s \
            || a3s_install_die "brew install a3s failed"
    fi

    a3s_scrub_local_shadows
    if [ "$yes" -eq 1 ]; then
        a3s_scrub_cargo_shadows
    fi
    a3s_verify_brew_install
}

a3s_scrub_local_shadows() {
    local_bin="${HOME:+$HOME/.local/bin}"
    [ -n "$local_bin" ] || return 0
    for name in a3s a3s-webview a3s-code; do
        if [ -e "$local_bin/$name" ] || [ -L "$local_bin/$name" ]; then
            a3s_install_info "removing shadowing standalone file $local_bin/$name"
            rm -f -- "$local_bin/$name" \
                || a3s_install_warn "could not remove $local_bin/$name"
        fi
    done
    if [ -d "$local_bin/moli" ]; then
        a3s_install_info "removing shadowing standalone moli at $local_bin/moli"
        rm -rf -- "$local_bin/moli" \
            || a3s_install_warn "could not remove $local_bin/moli"
    fi
}

a3s_scrub_cargo_shadows() {
    if ! command -v cargo >/dev/null 2>&1; then
        return 0
    fi
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    cargo_bin="$cargo_home/bin"
    if [ -e "$cargo_bin/a3s" ] || [ -L "$cargo_bin/a3s" ]; then
        a3s_install_info "uninstalling Cargo a3s that can shadow Homebrew ($cargo_bin/a3s)"
        cargo uninstall a3s >/dev/null 2>&1 \
            || a3s_install_warn "cargo uninstall a3s failed; remove $cargo_bin/a3s manually"
    fi
    if [ -e "$cargo_bin/a3s-code" ] || [ -L "$cargo_bin/a3s-code" ]; then
        a3s_install_info "uninstalling legacy Cargo crate a3s-code"
        cargo uninstall a3s-code >/dev/null 2>&1 \
            || a3s_install_warn "cargo uninstall a3s-code failed; remove $cargo_bin/a3s-code manually"
    fi
}

a3s_verify_brew_install() {
    hash -r 2>/dev/null || true
    active=$(command -v a3s 2>/dev/null || true)
    [ -n "$active" ] || a3s_install_die "a3s not found on PATH after Homebrew install"
    prefix=$(brew --prefix a3s 2>/dev/null || true)
    if [ -n "$prefix" ]; then
        expected="$prefix/bin/a3s"
        if [ "$active" != "$expected" ]; then
            a3s_install_warn "a3s resolves to $active (expected $expected); fix PATH order"
        fi
        companion_dir="$prefix/bin"
        [ -x "$companion_dir/a3s-webview" ] \
            || a3s_install_die "Homebrew a3s is missing a3s-webview beside the binary"
        if [ "$(uname -s)" = Darwin ]; then
            [ -f "$companion_dir/libzvec_c_api.dylib" ] \
                || a3s_install_die "Homebrew a3s is missing libzvec_c_api.dylib beside the binary"
        else
            [ -f "$companion_dir/libzvec_c_api.so" ] \
                || a3s_install_die "Homebrew a3s is missing libzvec_c_api.so beside the binary"
        fi
    fi
    if ! "$active" --version >/dev/null 2>&1; then
        a3s_install_die "a3s --version failed after Homebrew install ($active)"
    fi
    if ! "$active" code --help >/dev/null 2>&1; then
        a3s_install_warn "a3s code --help failed; ensure the CLI release includes the Code TUI entry"
    fi
    a3s_install_info "Homebrew install verified: $("$active" --version 2>/dev/null || true)"
    a3s_install_info "active binary: $active"
    a3s_install_info "launch the interactive Code TUI with: a3s code"
}
