#!/usr/bin/env bash

set -euo pipefail

repo_root=$(CDPATH=; cd -- "$(dirname -- "$0")/.." && pwd -P)
installer="$repo_root/install.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/a3s-installer-test.XXXXXX")

cleanup() {
    case "$test_root" in
        "${TMPDIR:-/tmp}"/a3s-installer-test.*) rm -rf -- "$test_root" ;;
        *) printf 'refusing to remove unexpected test directory: %s\n' "$test_root" >&2 ;;
    esac
}
trap cleanup EXIT

fail() {
    printf 'installer test failed: %s\n' "$*" >&2
    exit 1
}

assert_file() {
    [[ -f "$1" ]] || fail "expected file $1"
}

assert_content() {
    local expected=$1
    local path=$2
    [[ "$(cat "$path")" == "$expected" ]] \
        || fail "unexpected content in $path"
}

assert_no_generated_paths() {
    local root=$1
    local leftovers
    leftovers=$(find "$root" \( -name '.a3s.*' -o -name '.a3s-webview.*' -o -name '.a3s-moli.*' \))
    [[ -z "$leftovers" ]] || fail "installer left temporary paths: $leftovers"
}

expect_failure() {
    local description=$1
    shift
    if "$@" >"$test_root/failure.stdout" 2>"$test_root/failure.stderr"; then
        fail "$description unexpectedly succeeded"
    fi
}

mock_bin="$test_root/mock-bin"
fixture_root="$test_root/fixture"
mkdir -p "$mock_bin" "$fixture_root"
real_mv=$(command -v mv)

cat >"$mock_bin/uname" <<'EOF'
#!/bin/sh
case "${1:-}" in
    -s) printf '%s\n' "${MOCK_UNAME_S:-Linux}" ;;
    -m) printf '%s\n' "${MOCK_UNAME_M:-x86_64}" ;;
    *) printf '%s\n' "${MOCK_UNAME_S:-Linux}" ;;
esac
EOF

cat >"$mock_bin/getconf" <<'EOF'
#!/bin/sh
if [ "${MOCK_GLIBC:-1}" = 1 ] && [ "${1:-}" = GNU_LIBC_VERSION ]; then
    printf 'glibc 2.36\n'
    exit 0
fi
exit 1
EOF

cat >"$mock_bin/ldd" <<'EOF'
#!/bin/sh
printf 'musl libc (test fixture)\n' >&2
exit 1
EOF

cat >"$mock_bin/curl" <<'EOF'
#!/bin/sh
set -eu
: "${MOCK_CURL_CALLED:?}"
: >"$MOCK_CURL_CALLED"
destination=
request_url=
write_format=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o)
            destination=$2
            shift 2
            ;;
        -w)
            write_format=$2
            shift 2
            ;;
        https://*)
            request_url=$1
            shift
            ;;
        *) shift ;;
    esac
done
emit_status() {
    if [ -n "$write_format" ]; then
        printf '__A3S_HTTP_STATUS__%s' "$1"
    fi
}
if [ -n "$destination" ]; then
    cp "$MOCK_ARCHIVE" "$destination"
elif printf '%s' "$request_url" | grep -F '/repos/A3S-Lab/CLI/' >/dev/null &&
    ! printf '%s' "$request_url" | grep -F 'releases?per_page=100' >/dev/null &&
    [ "${MOCK_PRIMARY_RELEASE_NOT_FOUND:-0}" = 1 ]; then
    emit_status 404
    [ -n "$write_format" ] && exit 0
    exit 22
elif printf '%s' "$request_url" | grep -F '/repos/A3S-Lab/CLI/' >/dev/null &&
    [ -n "${MOCK_PRIMARY_HTTP_STATUS:-}" ]; then
    printf '{"message":"mock HTTP error"}'
    emit_status "$MOCK_PRIMARY_HTTP_STATUS"
    [ -n "$write_format" ] && exit 0
    exit 22
elif printf '%s' "$request_url" | grep -F '/repos/A3S-Lab/a3s/' >/dev/null &&
    printf '%s' "$request_url" | grep -F 'releases?per_page=100' >/dev/null &&
    [ -n "${MOCK_LEGACY_RELEASE_LIST_JSON:-}" ]; then
    cat "$MOCK_LEGACY_RELEASE_LIST_JSON"
elif printf '%s' "$request_url" | grep -F '/repos/A3S-Lab/a3s/' >/dev/null &&
    [ -n "${MOCK_LEGACY_RELEASE_JSON:-}" ]; then
    cat "$MOCK_LEGACY_RELEASE_JSON"
elif [ -n "${MOCK_RELEASE_LIST_JSON:-}" ] &&
    printf '%s' "$request_url" | grep -F 'releases?per_page=100' >/dev/null; then
    cat "$MOCK_RELEASE_LIST_JSON"
else
    cat "$MOCK_RELEASE_JSON"
fi
emit_status 200
EOF

cat >"$mock_bin/mv" <<'EOF'
#!/bin/sh
set -eu

source_path=${1:-}
destination_path=${2:-}
if [ "$source_path" = -f ]; then
    source_path=${2:-}
    destination_path=${3:-}
fi

"$REAL_MV" "$@"

if [ -z "${MOCK_MV_FAULT:-}" ] || [ -e "${MOCK_MV_FAULT_MARKER:-}" ]; then
    exit 0
fi

source_leaf=${source_path##*/}
destination_leaf=${destination_path##*/}
inject=0
case "$MOCK_MV_FAULT" in
    binary-activate)
        case "$source_leaf:$destination_leaf" in
            .a3s.new.*:a3s) inject=1 ;;
        esac
        ;;
    webview-activate)
        case "$source_leaf:$destination_leaf" in
            .a3s-webview.new.*:a3s-webview) inject=1 ;;
        esac
        ;;
    moli-activate)
        case "$source_leaf:$destination_leaf" in
            .a3s-moli.new.*:moli) inject=1 ;;
        esac
        ;;
esac

if [ "$inject" -eq 1 ]; then
    : >"$MOCK_MV_FAULT_MARKER"
    kill -TERM "$PPID"
fi
EOF
chmod +x "$mock_bin/uname" "$mock_bin/getconf" "$mock_bin/ldd" "$mock_bin/curl" "$mock_bin/mv"

base_path=$PATH
export PATH="$mock_bin:$base_path"
export MOCK_CURL_CALLED="$test_root/curl.called"
export REAL_MV="$real_mv"
export MOCK_MV_FAULT_MARKER="$test_root/mv-fault.triggered"
unset A3S_VERSION A3S_INSTALL_DIR A3S_MODIFY_PATH A3S_GITHUB_TOKEN
unset MOCK_MV_FAULT MOCK_MV_FAULT_VERSION
unset MOCK_RELEASE_LIST_JSON MOCK_LEGACY_RELEASE_LIST_JSON MOCK_LEGACY_RELEASE_JSON
unset MOCK_PRIMARY_RELEASE_NOT_FOUND
unset MOCK_PRIMARY_HTTP_STATUS

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{ print $1 }'
    else
        shasum -a 256 "$1" | awk '{ print $1 }'
    fi
}

make_fixture() {
    local version=$1
    local target=$2
    local include_webview=${3:-1}
    local release_repository=${4:-A3S-Lab/CLI}
    local include_legacy_payload=${5:-0}
    local include_moli=${6:-1}
    local release_repo_slug=${release_repository##*/}
    local payload="$fixture_root/payload"
    local archive="$fixture_root/a3s-v${version}-${target}.tar.gz"
    local asset_name="a3s-v${version}-${target}.tar.gz"
    local archive_members=(a3s)
    local digest

    rm -rf -- "$payload"
    mkdir -p "$payload"
    printf '#!/bin/sh\nprintf "a3s %s\\n"\n' "$version" >"$payload/a3s"
    chmod +x "$payload/a3s"
    if [ "$include_webview" -eq 1 ]; then
        printf '#!/bin/sh\nif [ "${1:-}" = "--agent-island" ]; then\n  printf "%%s\\n" "usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>" >&2\n  exit 2\nfi\nprintf "a3s-webview %s\\n"\n' \
            "$version" >"$payload/a3s-webview"
        chmod +x "$payload/a3s-webview"
        archive_members+=(a3s-webview)
    fi
    if [ "$include_moli" -eq 1 ]; then
        mkdir -p "$payload/moli"
        printf '#!/bin/sh\nprintf "moli %s\\n"\n' "$version" >"$payload/moli/moli"
        chmod +x "$payload/moli/moli"
        printf '{"schema":"a3s-code/moli-runtime-package/v1","version":"1.1.1","target":"%s"}\n' "$target" \
            >"$payload/moli/moli-runtime.json"
        archive_members+=(moli)
    fi
    if [ "$include_legacy_payload" -eq 1 ]; then
        mkdir -p "$payload/support" "$payload/release-compat"
        printf 'legacy runtime payload\n' >"$payload/support/legacy-runtime.txt"
        printf 'legacy release marker\n' >"$payload/release-compat/README.md"
        archive_members+=(support release-compat)
    fi
    tar -czf "$archive" -C "$payload" "${archive_members[@]}"
    digest=$(sha256_file "$archive")

    MOCK_ARCHIVE=$archive
    MOCK_RELEASE_JSON="$fixture_root/release-${version}-${release_repo_slug}.json"
    export MOCK_ARCHIVE MOCK_RELEASE_JSON
    printf '%s' \
        "{\"tag_name\":\"v${version}\",\"draft\":false,\"prerelease\":false,\"assets\":[{\"url\":\"https://api.github.com/repos/${release_repository}/releases/assets/1\",\"name\":\"unrelated.tar.gz\",\"uploader\":{\"login\":\"bot\"},\"state\":\"uploaded\",\"digest\":\"sha256:$(printf '0%.0s' {1..64})\",\"browser_download_url\":\"https://example.invalid/unrelated\"},{\"url\":\"https://api.github.com/repos/${release_repository}/releases/assets/2\",\"name\":\"${asset_name}\",\"uploader\":{\"login\":\"bot\",\"following_url\":\"https://api.github.com/users/bot/following{/other_user}\"},\"state\":\"uploaded\",\"digest\":\"sha256:${digest}\",\"browser_download_url\":\"https://github.com/${release_repository}/releases/download/v${version}/${asset_name}\"}]}" \
        >"$MOCK_RELEASE_JSON"
}

run_install() {
    local version=$1
    local install_dir=$2
    shift 2
    HOME="$test_root/home" \
    A3S_INSTALL_DIR="$install_dir" \
    MOCK_GLIBC=1 \
    sh "$installer" --version "$version" --no-modify-path "$@"
}

mkdir -p "$test_root/home"

# Stable archives published before the companion bundle remain installable;
# Code owns their verified WebView first-use setup.
export MOCK_UNAME_S=Linux MOCK_UNAME_M=x86_64
make_fixture 1.2.2 x86_64-unknown-linux-gnu 0 A3S-Lab/CLI 0 0
legacy_root="$test_root/legacy-without-webview"
run_install 1.2.2 "$legacy_root/bin"
assert_file "$legacy_root/bin/a3s"
[[ ! -e "$legacy_root/bin/a3s-webview" ]] \
    || fail 'legacy release unexpectedly installed a WebView companion'
[[ ! -e "$legacy_root/bin/moli" ]] \
    || fail 'legacy release unexpectedly installed a Moli runtime'
assert_no_generated_paths "$legacy_root"

# Historical CLI archives may contain runtime payloads that are no longer
# supported. The installer validates their paths, extracts only the binaries,
# and never copies the legacy payload into the installation directory.
make_fixture 1.2.9 x86_64-unknown-linux-gnu 1 A3S-Lab/CLI 1 0
legacy_payload_root="$test_root/legacy-runtime-payload"
run_install 1.2.9 "$legacy_payload_root/bin"
assert_file "$legacy_payload_root/bin/a3s"
assert_file "$legacy_payload_root/bin/a3s-webview"
[[ ! -e "$legacy_payload_root/bin/support" ]] \
    || fail 'legacy runtime payload was installed'
[[ ! -e "$legacy_payload_root/bin/release-compat" ]] \
    || fail 'legacy release marker was installed'
assert_no_generated_paths "$legacy_payload_root"

# `latest` ignores unrelated product tags and prereleases in the release feed.
make_fixture 1.2.2 x86_64-unknown-linux-gnu 0
latest_list="$fixture_root/releases.json"
printf '%s' \
    '[{"tag_name":"a3s-code-v9.0.0","draft":false,"prerelease":false},{"tag_name":"v9.0.0","draft":false,"prerelease":true},{"tag_name":"v1.2.2","draft":false,"prerelease":false}]' \
    >"$latest_list"
export MOCK_RELEASE_LIST_JSON="$latest_list"
latest_root="$test_root/latest-stable-cli"
run_install latest "$latest_root/bin"
assert_file "$latest_root/bin/a3s"
assert_no_generated_paths "$latest_root"
unset MOCK_RELEASE_LIST_JSON

# During the release transition, latest selects the higher stable version from
# the canonical CLI repository and the former monorepo release source.
primary_list="$fixture_root/releases-primary.json"
legacy_list="$fixture_root/releases-legacy.json"
printf '%s' \
    '[{"tag_name":"v1.2.2","draft":false,"prerelease":false}]' \
    >"$primary_list"
printf '%s' \
    '[{"tag_name":"v1.3.0","draft":false,"prerelease":false}]' \
    >"$legacy_list"
make_fixture 1.3.0 x86_64-unknown-linux-gnu 0 A3S-Lab/a3s
export MOCK_RELEASE_LIST_JSON="$primary_list"
export MOCK_LEGACY_RELEASE_LIST_JSON="$legacy_list"
export MOCK_LEGACY_RELEASE_JSON="$MOCK_RELEASE_JSON"
transition_root="$test_root/release-transition"
run_install latest "$transition_root/bin"
[[ "$("$transition_root/bin/a3s" --version)" == 'a3s 1.3.0' ]] \
    || fail 'latest did not select the newer former-monorepo stable release'
assert_no_generated_paths "$transition_root"

export MOCK_PRIMARY_RELEASE_NOT_FOUND=1
explicit_transition_root="$test_root/explicit-release-transition"
run_install 1.3.0 "$explicit_transition_root/bin"
[[ "$("$explicit_transition_root/bin/a3s" --version)" == 'a3s 1.3.0' ]] \
    || fail 'an explicit version did not fall back to the former-monorepo release'
assert_no_generated_paths "$explicit_transition_root"
unset MOCK_PRIMARY_RELEASE_NOT_FOUND
unset MOCK_RELEASE_LIST_JSON MOCK_LEGACY_RELEASE_LIST_JSON MOCK_LEGACY_RELEASE_JSON

# An API rate-limit or server error must not be reported as a missing release
# or silently fall through to an unrelated repository.
make_fixture 1.3.1 x86_64-unknown-linux-gnu
export MOCK_PRIMARY_HTTP_STATUS=403
expect_failure 'GitHub API rate limit' run_install 1.3.1 "$test_root/api-rate-limit/bin"
grep -F 'HTTP 403' "$test_root/failure.stderr" >/dev/null \
    || fail 'API rate-limit failure did not expose the HTTP status'
unset MOCK_PRIMARY_HTTP_STATUS

# Every published Unix target maps to the exact release asset name.
for target_case in \
    'Linux x86_64 x86_64-unknown-linux-gnu' \
    'Linux aarch64 aarch64-unknown-linux-gnu' \
    'Darwin x86_64 x86_64-apple-darwin' \
    'Darwin arm64 aarch64-apple-darwin'; do
    read -r mock_os mock_arch target <<<"$target_case"
    make_fixture 1.2.3 "$target"
    export MOCK_UNAME_S=$mock_os MOCK_UNAME_M=$mock_arch
    case_root="$test_root/targets/$target"
    run_install 1.2.3 "$case_root/bin"
    assert_file "$case_root/bin/a3s"
    assert_file "$case_root/bin/a3s-webview"
    assert_file "$case_root/bin/moli/moli"
    [[ "$("$case_root/bin/a3s" --version)" == 'a3s 1.2.3' ]] \
        || fail "wrong installed version for $target"
    [[ "$("$case_root/bin/a3s-webview")" == 'a3s-webview 1.2.3' ]] \
        || fail "wrong installed WebView companion for $target"
    [[ "$("$case_root/bin/moli/moli")" == 'moli 1.2.3' ]] \
        || fail "wrong installed Moli runtime for $target"
    assert_no_generated_paths "$case_root"
done

# Upgrade replaces the binary and companion payloads without leaving staging files.
export MOCK_UNAME_S=Linux MOCK_UNAME_M=x86_64
upgrade_root="$test_root/upgrade 用户 space"
make_fixture 1.2.3 x86_64-unknown-linux-gnu
run_install 1.2.3 "$upgrade_root/bin"
make_fixture 1.2.4 x86_64-unknown-linux-gnu
run_install 1.2.4 "$upgrade_root/bin"
[[ "$("$upgrade_root/bin/a3s" --version)" == 'a3s 1.2.4' ]] || fail 'upgrade did not replace binary'
[[ "$("$upgrade_root/bin/a3s-webview")" == 'a3s-webview 1.2.4' ]] \
        || fail 'upgrade did not replace WebView companion'
    [[ "$("$upgrade_root/bin/moli/moli")" == 'moli 1.2.4' ]] \
        || fail 'upgrade did not replace Moli runtime'
assert_no_generated_paths "$upgrade_root"

# A legacy release without a Moli sidecar does not destroy an already
# installed runtime; the existing verified sidecar remains available to Code.
make_fixture 1.2.4 x86_64-unknown-linux-gnu 1 A3S-Lab/CLI 0 0
run_install 1.2.4 "$upgrade_root/bin"
[[ "$("$upgrade_root/bin/moli/moli")" == 'moli 1.2.4' ]] \
    || fail 'a release without Moli removed the existing runtime'
assert_no_generated_paths "$upgrade_root"

# A partial Moli directory is rejected before activation and leaves the old
# complete runtime and binaries untouched.
incomplete_payload="$fixture_root/incomplete-moli-payload"
rm -rf -- "$incomplete_payload"
mkdir -p "$incomplete_payload/moli"
printf '#!/bin/sh\nprintf "a3s 1.2.5\\n"\n' >"$incomplete_payload/a3s"
chmod +x "$incomplete_payload/a3s"
printf '#!/bin/sh\nprintf "a3s-webview 1.2.5\\n"\n' >"$incomplete_payload/a3s-webview"
chmod +x "$incomplete_payload/a3s-webview"
printf '{"schema":"a3s-code/moli-runtime-package/v1"}\n' >"$incomplete_payload/moli/moli-runtime.json"
incomplete_archive="$fixture_root/a3s-v1.2.5-x86_64-unknown-linux-gnu.tar.gz"
tar -czf "$incomplete_archive" -C "$incomplete_payload" a3s a3s-webview moli
MOCK_ARCHIVE="$incomplete_archive"
export MOCK_ARCHIVE
incomplete_digest=$(sha256_file "$MOCK_ARCHIVE")
printf '%s' \
    "{\"tag_name\":\"v1.2.5\",\"draft\":false,\"prerelease\":false,\"assets\":[{\"url\":\"https://api.github.com/repos/A3S-Lab/CLI/releases/assets/6\",\"name\":\"a3s-v1.2.5-x86_64-unknown-linux-gnu.tar.gz\",\"state\":\"uploaded\",\"digest\":\"sha256:${incomplete_digest}\",\"browser_download_url\":\"https://github.com/A3S-Lab/CLI/releases/download/v1.2.5/a3s-v1.2.5-x86_64-unknown-linux-gnu.tar.gz\"}]}" \
    >"$fixture_root/incomplete-moli.json"
MOCK_RELEASE_JSON="$fixture_root/incomplete-moli.json"
export MOCK_RELEASE_JSON
expect_failure 'incomplete Moli runtime' run_install 1.2.5 "$upgrade_root/bin"
[[ "$("$upgrade_root/bin/a3s" --version)" == 'a3s 1.2.4' ]] \
    || fail 'incomplete Moli archive changed the installed binary'
[[ "$("$upgrade_root/bin/moli/moli")" == 'moli 1.2.4' ]] \
    || fail 'incomplete Moli archive changed the installed runtime'
assert_no_generated_paths "$upgrade_root"

# Existing Moli symlinks are never followed or replaced during an upgrade.
make_fixture 1.2.5 x86_64-unknown-linux-gnu
MOCK_RELEASE_JSON="$fixture_root/release-1.2.5-CLI.json"
export MOCK_RELEASE_JSON
moli_saved="$upgrade_root/moli-saved"
mv "$upgrade_root/bin/moli" "$moli_saved"
ln -s "$moli_saved" "$upgrade_root/bin/moli"
expect_failure 'Moli symlink replacement' run_install 1.2.5 "$upgrade_root/bin"
[[ -L "$upgrade_root/bin/moli" ]] || fail 'Moli symlink was replaced'
[[ "$("$upgrade_root/bin/a3s" --version)" == 'a3s 1.2.4' ]] \
    || fail 'Moli symlink rejection changed the installed binary'
rm -f -- "$upgrade_root/bin/moli"
mv "$moli_saved" "$upgrade_root/bin/moli"
assert_no_generated_paths "$upgrade_root"

# A digest mismatch fails before activation and preserves the installed version.
make_fixture 1.2.5 x86_64-unknown-linux-gnu
sed 's/"digest":"sha256:[0-9a-f]*"/"digest":"sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"/2' \
    "$MOCK_RELEASE_JSON" >"$fixture_root/bad-digest.json"
MOCK_RELEASE_JSON="$fixture_root/bad-digest.json"
export MOCK_RELEASE_JSON
expect_failure 'digest mismatch' run_install 1.2.5 "$upgrade_root/bin"
[[ "$("$upgrade_root/bin/a3s" --version)" == 'a3s 1.2.4' ]] || fail 'digest failure changed old binary'
    [[ "$("$upgrade_root/bin/a3s-webview")" == 'a3s-webview 1.2.4' ]] \
        || fail 'digest failure changed old WebView companion'
    [[ "$("$upgrade_root/bin/moli/moli")" == 'moli 1.2.4' ]] \
        || fail 'digest failure changed old Moli runtime'

# A missing target digest cannot borrow the following asset's digest.
make_fixture 1.2.6 x86_64-unknown-linux-gnu
asset_name='a3s-v1.2.6-x86_64-unknown-linux-gnu.tar.gz'
actual_digest=$(sha256_file "$MOCK_ARCHIVE")
printf '%s' \
    "{\"tag_name\":\"v1.2.6\",\"draft\":false,\"prerelease\":false,\"assets\":[{\"url\":\"https://api.github.com/repos/A3S-Lab/CLI/releases/assets/3\",\"name\":\"${asset_name}\",\"state\":\"uploaded\",\"browser_download_url\":\"https://github.com/A3S-Lab/CLI/releases/download/v1.2.6/${asset_name}\"},{\"url\":\"https://api.github.com/repos/A3S-Lab/CLI/releases/assets/4\",\"name\":\"other.tar.gz\",\"state\":\"uploaded\",\"digest\":\"sha256:${actual_digest}\",\"browser_download_url\":\"https://example.invalid/other\"}]}" \
    >"$fixture_root/missing-digest.json"
MOCK_RELEASE_JSON="$fixture_root/missing-digest.json"
export MOCK_RELEASE_JSON
expect_failure 'missing target digest' run_install 1.2.6 "$upgrade_root/bin"

# Unexpected archive members are rejected before activation.
payload="$fixture_root/unsafe-payload"
rm -rf -- "$payload"
mkdir -p "$payload"
printf '#!/bin/sh\nprintf "a3s 1.2.7\\n"\n' >"$payload/a3s"
chmod +x "$payload/a3s"
printf '#!/bin/sh\nprintf "a3s-webview 1.2.7\\n"\n' >"$payload/a3s-webview"
chmod +x "$payload/a3s-webview"
printf 'unexpected\n' >"$payload/escape"
MOCK_ARCHIVE="$fixture_root/a3s-v1.2.7-x86_64-unknown-linux-gnu.tar.gz"
tar -czf "$MOCK_ARCHIVE" -C "$payload" a3s a3s-webview escape
export MOCK_ARCHIVE
unsafe_digest=$(sha256_file "$MOCK_ARCHIVE")
unsafe_asset=$(basename "$MOCK_ARCHIVE")
printf '%s' \
    "{\"tag_name\":\"v1.2.7\",\"draft\":false,\"prerelease\":false,\"assets\":[{\"url\":\"https://api.github.com/repos/A3S-Lab/CLI/releases/assets/5\",\"name\":\"${unsafe_asset}\",\"state\":\"uploaded\",\"digest\":\"sha256:${unsafe_digest}\",\"browser_download_url\":\"https://github.com/A3S-Lab/CLI/releases/download/v1.2.7/${unsafe_asset}\"}]}" \
    >"$fixture_root/unsafe.json"
MOCK_RELEASE_JSON="$fixture_root/unsafe.json"
export MOCK_RELEASE_JSON
expect_failure 'unsafe archive member' run_install 1.2.7 "$upgrade_root/bin"
[[ "$("$upgrade_root/bin/a3s" --version)" == 'a3s 1.2.4' ]] || fail 'unsafe archive changed old binary'
    [[ "$("$upgrade_root/bin/a3s-webview")" == 'a3s-webview 1.2.4' ]] \
        || fail 'unsafe archive changed old WebView companion'
    [[ "$("$upgrade_root/bin/moli/moli")" == 'moli 1.2.4' ]] \
        || fail 'unsafe archive changed old Moli runtime'

# Unsupported and non-glibc hosts fail before making a network request.
rm -f "$MOCK_CURL_CALLED"
export MOCK_UNAME_S=Linux MOCK_UNAME_M=riscv64
expect_failure 'unsupported architecture' run_install 1.2.4 "$test_root/unsupported/bin"
[[ ! -e "$MOCK_CURL_CALLED" ]] || fail 'unsupported architecture reached the network'

rm -f "$MOCK_CURL_CALLED"
export MOCK_UNAME_S=Linux MOCK_UNAME_M=x86_64 MOCK_GLIBC=0
expect_failure 'musl host' env \
    HOME="$test_root/home" A3S_INSTALL_DIR="$test_root/musl/bin" \
    MOCK_GLIBC=0 sh "$installer" --version 1.2.4 --no-modify-path
[[ ! -e "$MOCK_CURL_CALLED" ]] || fail 'non-glibc host reached the network'

# PATH modification is opt-in and idempotent.
export MOCK_UNAME_S=Linux MOCK_UNAME_M=x86_64 MOCK_GLIBC=1
make_fixture 1.2.8 x86_64-unknown-linux-gnu
profile_home="$test_root/profile-home"
mkdir -p "$profile_home"
HOME="$profile_home" SHELL=/bin/sh \
    sh "$installer" --version 1.2.8
[[ ! -e "$profile_home/.profile" ]] || fail 'default install modified a shell profile'
HOME="$profile_home" SHELL=/bin/sh \
    sh "$installer" --version 1.2.8 --modify-path
HOME="$profile_home" SHELL=/bin/sh \
    sh "$installer" --version 1.2.8 --modify-path
[[ "$(grep -Fxc 'export PATH="$HOME/.local/bin:$PATH"' "$profile_home/.profile")" -eq 1 ]] \
    || fail 'PATH profile entry is not idempotent'

# Interruptions after filesystem mutations restore the prior disk state even
# before the installer can update its in-memory activation flags.
fault_root="$test_root/fault-injection"
make_fixture 4.0.0 x86_64-unknown-linux-gnu
run_install 4.0.0 "$fault_root/bin"
old_webview_sha=$(sha256_file "$fault_root/bin/a3s-webview")
old_moli_sha=$(sha256_file "$fault_root/bin/moli/moli")

make_fixture 4.0.1 x86_64-unknown-linux-gnu
export MOCK_MV_FAULT=moli-activate
rm -f "$MOCK_MV_FAULT_MARKER"
expect_failure 'interruption after Moli runtime activation' \
    run_install 4.0.1 "$fault_root/bin"
[[ -e "$MOCK_MV_FAULT_MARKER" ]] || fail 'Moli runtime fault was not injected'
[[ "$(sha256_file "$fault_root/bin/moli/moli")" == "$old_moli_sha" ]] \
    || fail 'Moli activation interruption did not restore the previous runtime'
[[ "$($fault_root/bin/a3s --version)" == 'a3s 4.0.0' ]] \
    || fail 'Moli activation interruption changed the installed binary'
assert_no_generated_paths "$fault_root"

make_fixture 4.0.0 x86_64-unknown-linux-gnu
export MOCK_MV_FAULT=webview-activate
rm -f "$MOCK_MV_FAULT_MARKER"
expect_failure 'interruption after WebView companion activation' \
    run_install 4.0.0 "$fault_root/bin"
[[ -e "$MOCK_MV_FAULT_MARKER" ]] || fail 'WebView companion fault was not injected'
[[ "$(sha256_file "$fault_root/bin/a3s-webview")" == "$old_webview_sha" ]] \
    || fail 'WebView activation interruption did not restore the previous companion'
[[ "$(sha256_file "$fault_root/bin/moli/moli")" == "$old_moli_sha" ]] \
    || fail 'WebView activation interruption changed the installed Moli runtime'
[[ "$("$fault_root/bin/a3s" --version)" == 'a3s 4.0.0' ]] \
    || fail 'WebView activation interruption changed the installed binary'
assert_no_generated_paths "$fault_root"

make_fixture 4.0.1 x86_64-unknown-linux-gnu
export MOCK_MV_FAULT=binary-activate MOCK_MV_FAULT_VERSION=4.0.1
rm -f "$MOCK_MV_FAULT_MARKER"
expect_failure 'interruption after binary activation' \
    run_install 4.0.1 "$fault_root/bin"
[[ -e "$MOCK_MV_FAULT_MARKER" ]] || fail 'binary activation fault was not injected'
[[ "$("$fault_root/bin/a3s" --version)" == 'a3s 4.0.0' ]] \
    || fail 'binary activation interruption did not restore the previous binary'
[[ "$(sha256_file "$fault_root/bin/a3s-webview")" == "$old_webview_sha" ]] \
    || fail 'binary activation interruption did not restore the previous WebView companion'
[[ "$(sha256_file "$fault_root/bin/moli/moli")" == "$old_moli_sha" ]] \
    || fail 'binary activation interruption changed the installed Moli runtime'
assert_no_generated_paths "$fault_root"
unset MOCK_MV_FAULT MOCK_MV_FAULT_VERSION

printf 'install.sh tests passed\n'
