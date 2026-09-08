#!/usr/bin/env bash
# Publish or update a file on GitHub via Contents API (JSON body; avoids ARG_MAX).
# Usage: publish-github-file.sh OWNER/REPO REMOTE_PATH LOCAL_FILE COMMIT_MESSAGE [BRANCH]
set -euo pipefail
repo="${1:?repo}"
remote_path="${2:?remote_path}"
local_file="${3:?file}"
message="${4:?message}"
branch="${5:-main}"

sha="$(gh api "repos/${repo}/contents/${remote_path}?ref=${branch}" --jq .sha 2>/dev/null || true)"
payload="$(mktemp)"
cleanup() { rm -f "$payload"; }
trap cleanup EXIT

export LOCAL_FILE="$local_file" MESSAGE="$message" BRANCH="$branch" SHA="${sha:-}" PAYLOAD="$payload"
python3 <<'PY'
import base64, json, os, pathlib
local = pathlib.Path(os.environ["LOCAL_FILE"])
content = base64.b64encode(local.read_bytes()).decode("ascii")
body = {
    "message": os.environ["MESSAGE"],
    "content": content,
    "branch": os.environ["BRANCH"],
}
sha = os.environ.get("SHA") or ""
if sha:
    body["sha"] = sha
pathlib.Path(os.environ["PAYLOAD"]).write_text(json.dumps(body), encoding="utf-8")
PY

gh api --method PUT "repos/${repo}/contents/${remote_path}" --input "$payload" \
  --jq ".commit.sha"
