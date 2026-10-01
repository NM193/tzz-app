#!/bin/bash
#
# Cut a release: build the app, sign it, and put it where the app looks.
#
#   bash scripts/release.sh 0.2.0 "What changed, in a sentence or two."
#
# The key is found, not asked for. It lives at ~/.tauri/tzz.key, outside the
# repo, and its password lives in the Keychain -- which is the only arrangement
# where having a password on the key means anything. Neither is ever printed.
#
# The key in use has no password, so there is nothing to remember and nothing
# interactive in a release. A replacement is made like this -- and its public
# half then has to go into src-tauri/tauri.conf.json -> plugins.updater.pubkey,
# because the app trusts that one key and no other:
#
#   npx tauri signer generate -w ~/.tauri/tzz.key -p "" -f --ci
#
# A key WITH a password works too: the first release asks for it once and keeps
# it in the Keychain. It is a password on a file that already sits behind your
# login, guarding updates to an app on two Macs, so it buys little.
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

# An environment that already carries the key wins, so a machine that keeps it
# somewhere else -- a CI runner, say -- needs no change here.
key="$HOME/.tauri/tzz.key"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  if [ ! -f "$key" ]; then
    echo "No signing key at $key. Make one, once:"
    echo "  npx tauri signer generate -w ~/.tauri/tzz.key"
    exit 1
  fi
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$key")"
fi

# A key may have no password at all, and nothing in the file says which. So
# this signs a throwaway file with an empty one and believes the answer -- it
# is a second of certainty instead of a guess that surfaces as a failed build.
#
# When the key does want a password, the first release asks for it rather than
# printing the command that would. `security` does the asking, so the password
# goes from the terminal into the Keychain and through nothing in between: not
# this script, not the shell history, not a shell profile.
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  probe="$(mktemp)"
  echo probe > "$probe"
  if npx tauri signer sign -f "$key" -p "" "$probe" > /dev/null 2>&1; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
  elif security find-generic-password -s tzz-signing > /dev/null 2>&1; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s tzz-signing -w)"
  elif [ -t 0 ]; then
    echo "The key has a password. Once, for the Keychain:"
    security add-generic-password -a "$USER" -s tzz-signing -U -w || exit 1
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s tzz-signing -w)"
  else
    echo "The key has a password and there is no terminal to ask on. Once, by hand:"
    echo "  security add-generic-password -a \"\$USER\" -s tzz-signing -U -w"
    exit 1
  fi
  rm -f "$probe" "$probe.sig"
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
