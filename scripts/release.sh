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
ours=no
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  if [ ! -f "$key" ]; then
    echo "No signing key at $key. Make one, once:"
    echo "  npx tauri signer generate -w ~/.tauri/tzz.key -p \"\" -f --ci"
    exit 1
  fi
  ours=yes
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$key")"
fi

# A key may have no password at all, and nothing in the file says which. So
# this signs a throwaway file with an empty one and believes the answer -- it
# is a second of certainty instead of a guess that surfaces as a failed build.
# Only for our own key: one handed over in the environment comes with its own
# password, or none, and is not ours to interrogate.
#
# When the key does want a password, the first release asks for it rather than
# printing the command that would. `security` does the asking, so the password
# goes from the terminal into the Keychain and through nothing in between: not
# this script, not the shell history, not a shell profile.
if [ "$ours" = yes ] && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  probe="$(mktemp)"
  echo probe > "$probe"
  # `env -u` is not decoration: with the key in the environment the CLI stops
  # honouring -f and asks for a flag that is not this one.
  if env -u TAURI_SIGNING_PRIVATE_KEY \
      npx tauri signer sign -f "$key" -p "" "$probe" > /dev/null 2>&1; then
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

echo "== release v$version"
git add -A
git commit -qm "Version $version" || true
git tag -f "v$version"
git push -q origin HEAD
git push -q -f origin "v$version"

# Re-runnable on purpose. A release that got as far as uploading and then fell
# over -- a bad manifest, a wrong signature -- is mended by running this again,
# not by deleting it and hoping. The archive and its signature go up together
# or the manifest would describe bytes nobody has.
if gh release view "v$version" > /dev/null 2>&1; then
  echo "   v$version exists; replacing what it holds"
  gh release edit "v$version" --title "Tzz App $version" --notes "$notes" > /dev/null
  gh release upload "v$version" \
    "$archive" "$archive.sig" "$bundle"/dmg/*.dmg --clobber
else
  gh release create "v$version" \
    --title "Tzz App $version" \
    --notes "$notes" \
    "$archive" "$archive.sig" "$bundle"/dmg/*.dmg
fi

# The manifest goes up second, because its url has to name the archive exactly
# as GitHub serves it -- and GitHub renames what it is given. "Tzz App.app.tar.gz"
# arrives as "Tzz.App.app.tar.gz", so a name taken from the file on disk points
# at nothing: the app announces an update and then cannot fetch it. The only
# reliable source for that name is GitHub itself, after the upload.
url="$(gh release view "v$version" --json assets \
  -q '.assets[] | select(.name | endswith(".app.tar.gz")) | .url')"
if [ -z "$url" ]; then
  echo "The release has no .app.tar.gz asset. Nothing to point the manifest at."
  exit 1
fi

python3 - "$version" "$notes" "$url" "$archive" <<'MANIFEST'
import datetime, json, sys
version, notes, url, archive = sys.argv[1:5]
json.dump({
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "darwin-aarch64": {
            "signature": open(archive + ".sig").read().strip(),
            "url": url,
        }
    },
}, open("latest.json", "w"), indent=2)
MANIFEST

gh release upload "v$version" latest.json --clobber
rm -f latest.json

echo
echo "== check"

# This release's own manifest, not whatever /latest happens to be pointing at.
# Asking /latest straight after a release gets the release before it for a few
# seconds, which is how a check can read a stale file and still say 200.
here="$(mktemp -d)"
gh release download "v$version" -p latest.json -D "$here" --clobber > /dev/null

got="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["platforms"]["darwin-aarch64"]["url"])' "$here/latest.json")"
code="$(curl -sIL -o /dev/null -w '%{http_code}' "$got")"
echo "   archive:  $got"
echo "   fetching it says: $code"
[ "$code" = 200 ] || { echo "   that is not a download. The update would fail."; exit 1; }

# The signature has to be the one made for these bytes. A manifest carrying
# some other release's signature downloads fine and is then rejected, which
# looks like a broken app rather than a broken release.
if ! python3 -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1]))["platforms"]["darwin-aarch64"]["signature"] == open(sys.argv[2]).read().strip() else 1)' \
    "$here/latest.json" "$archive.sig"; then
  echo "   the manifest's signature is not this build's. The app would refuse it."
  exit 1
fi
echo "   signature:  matches this build"
rm -rf "$here"

# And finally the address the app is actually given, once GitHub agrees that
# this is the newest release.
manifest="https://github.com/$repo/releases/latest/download/latest.json"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  serving="$(curl -fsSL "$manifest" 2> /dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])' 2> /dev/null || true)"
  [ "$serving" = "$version" ] && break
  sleep 3
done
echo "   $manifest serves: ${serving:-nothing yet}"
if [ "$serving" != "$version" ]; then
  echo "   GitHub still calls an older release the newest. It usually catches up;"
  echo "   if it does not, the release is there but the app will not see it."
  exit 1
fi

echo
echo "Done. The app will see it the next time it starts."
