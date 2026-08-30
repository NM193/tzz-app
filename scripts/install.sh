#!/usr/bin/env bash
# Build the app and replace the copy in /Applications.
#
# Local-only updating: no signing keys, no hosting, no release process. Run it
# after any change and the installed app is current.
set -euo pipefail

APP="Tzz App.app"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILT="$ROOT/src-tauri/target/release/bundle/macos/$APP"

# Refuse to interrupt work in progress: a killed whisper run wastes the whole
# transcription, and a killed ffmpeg leaves an unplayable recording.
if pgrep -x whisper-cli >/dev/null || pgrep -x yt-dlp >/dev/null; then
  echo "A job is still running. Let it finish, then run this again." >&2
  exit 1
fi

echo "Building..."
cd "$ROOT"
npm run build

echo "Replacing /Applications/$APP..."
# The app was called YT MP3 until it was renamed; clear the old copy out.
osascript -e "quit app \"YT MP3\"" 2>/dev/null || true
rm -rf "/Applications/YT MP3.app"
osascript -e "quit app \"Tzz App\"" 2>/dev/null || true
sleep 1
rm -rf "/Applications/$APP"
cp -R "$BUILT" /Applications/

echo "Launching..."
open "/Applications/$APP"
echo "Done. Version $(plutil -extract CFBundleShortVersionString raw "/Applications/$APP/Contents/Info.plist")."
