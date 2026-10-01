#!/bin/bash
#
# Cut a release: build the app, sign it, and put it where the app looks.
#
#   bash scripts/release.sh 0.2.0 "What changed, in a sentence or two."
#
# The private key never lives in this repo. It comes from the environment, and
# without it the build produces no signature -- which the app would refuse,
# that being the whole point of signing. Set it up once:
#
#   npx tauri signer generate -w ~/.tauri/tzz.key
#   # then put the public half in src-tauri/tauri.conf.json -> plugins.updater.pubkey
#   # and in your shell profile:
#   export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/tzz.key)"
#   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="…"
#
# Only Apple Silicon is built, because the ffmpeg this app carries is arm64.

set -euo pipefail
cd "$(dirname "$0")/.."

version="${1:-}"
notes="${2:-A new version.}"

if [ -z "$version" ]; then
  echo "usage: bash scripts/release.sh <version> [notes]"
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "TAURI_SIGNING_PRIVATE_KEY is not set -- see the comment at the top of this script."
  exit 1
fi

if ! python3 -c 'import json,sys; sys.exit(0 if json.load(open("src-tauri/tauri.conf.json"))["plugins"]["updater"]["pubkey"] else 1)'; then
  echo "plugins.updater.pubkey is empty in src-tauri/tauri.conf.json."
  echo "Paste the public half of the key you generated there, then run this again."
  exit 1
fi

repo="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)"
if [ -z "$repo" ]; then
  echo "This checkout has no GitHub repo. Create one, push, then run this again."
  exit 1
fi

# The version the app reports, and the one an update is compared against.
python3 - "$version" <<'PY'
import collections, json, sys
path = "src-tauri/tauri.conf.json"
conf = json.load(open(path), object_pairs_hook=collections.OrderedDict)
conf["version"] = sys.argv[1]
json.dump(conf, open(path, "w"), indent=2)
open(path, "a").write("\n")
PY

echo "== tools"
bash scripts/fetch-tools.sh > /dev/null

echo "== build"
npm run build

bundle="src-tauri/target/release/bundle"
archive="$(ls "$bundle"/macos/*.app.tar.gz 2>/dev/null | head -1)"
if [ -z "$archive" ] || [ ! -f "$archive.sig" ]; then
  echo "No signed updater archive was produced. Is the signing key right?"
  exit 1
fi

# The manifest the app reads. Its url has to name the asset exactly as GitHub
# will serve it, so the name is taken from the file itself.
python3 - "$version" "$notes" "$repo" "$archive" <<'PY'
import json, os, sys, datetime
version, notes, repo, archive = sys.argv[1:5]
name = os.path.basename(archive)
json.dump({
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "darwin-aarch64": {
            "signature": open(archive + ".sig").read().strip(),
            "url": f"https://github.com/{repo}/releases/download/v{version}/{name}",
        }
    },
}, open("latest.json", "w"), indent=2)
PY

echo "== release v$version"
git add -A
git commit -qm "Version $version" || true
git tag -f "v$version"
git push -q origin HEAD
git push -q -f origin "v$version"

gh release create "v$version" \
  --title "Tzz App $version" \
  --notes "$notes" \
  "$archive" "$archive.sig" latest.json "$bundle"/dmg/*.dmg

rm -f latest.json
echo
echo "Done. The app will see it the next time it starts."
