#!/bin/bash
#
# Put the tools the app needs inside the app.
#
# WHY: the app looks for yt-dlp, ffmpeg and whisper-cli on the system. On the
# machine it was built on they are there, from Homebrew. On anyone else's Mac
# they are not -- and none of the Homebrew copies can simply be carried over.
# ffmpeg links nineteen Homebrew libraries, yt-dlp is a shell script pointing
# into the Cellar, and whisper-cli needs three libraries plus a directory of
# backends that ggml opens by an absolute path compiled into it.
#
# So two are fetched as the portable builds their authors publish, and whisper
# is built here with nothing left outside it.
#
# A tool that is already here and answers for itself is left alone, because a
# release runs this every time and 90 MB a release is a toll for nothing.
# `--force` fetches and builds all three again regardless.
#
# `npm run build` expects the results to be present.
# Needs: cmake and git (for whisper), curl and unzip.

set -euo pipefail

cd "$(dirname "$0")/.."
OUT="src-tauri/binaries"
WORK="build"

# Tauri names a sidecar after the target triple it belongs to.
TARGET="$(rustc -vV | sed -n 's/^host: //p')"

mkdir -p "$OUT" "$WORK"
say() { printf '\n== %s\n' "$1"; }

force=no
[ "${1:-}" = "--force" ] && force=yes

# Here, and able to run. A tool that cannot run is not here, whatever the
# directory listing says: a half-written download is the shape this goes
# wrong in, and it is the same size as a whole one until it is tried.
have() {
  local tool="$OUT/$1-$TARGET"
  [ "$force" = no ] || return 1
  [ -x "$tool" ] || return 1
  shift
  "$tool" "$@" > /dev/null 2>&1
}

# The standalone build: one file, and no Python needed on the machine.
fetch_ytdlp() {
  curl -fsSL -o "$OUT/yt-dlp-$TARGET" \
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
  chmod +x "$OUT/yt-dlp-$TARGET"
  "$OUT/yt-dlp-$TARGET" --version | sed 's/^/   /'
}

# A static arm64 build. Homebrew's own is linked against Homebrew.
fetch_ffmpeg() {
  local tmp
  tmp="$(mktemp -d)"
  curl -fsSL -o "$tmp/ffmpeg.zip" "https://www.osxexperts.net/ffmpeg711arm.zip"
  unzip -qo "$tmp/ffmpeg.zip" -d "$tmp"
  rm -f "$OUT/ffmpeg-$TARGET"
  mv "$tmp/ffmpeg" "$OUT/ffmpeg-$TARGET"
  chmod +x "$OUT/ffmpeg-$TARGET"
  rm -rf "$tmp"
  "$OUT/ffmpeg-$TARGET" -version | head -1 | sed 's/^/   /'
}

# Built rather than copied, and that is the whole point of the flags:
#
#   BUILD_SHARED_LIBS=OFF   the libraries go inside the binary
#   GGML_BACKEND_DL=OFF     so do the backends. Homebrew's build loads them by
#                           a path compiled into it -- /opt/homebrew/Cellar/...
#                           -- and GGML_BACKEND_PATH can only add one file, so
#                           a copied binary on a Mac without Homebrew comes up
#                           with no CPU and no Metal at all.
#   METAL_EMBED_LIBRARY=ON  the shaders travel in the binary too.
#   OPENMP=OFF              one fewer Homebrew library to chase.
#
# The result is about five megabytes and needs nothing but macOS itself.
build_whisper() {
  if ! command -v cmake > /dev/null; then
    echo "   cmake is needed to build it: brew install cmake"
    exit 1
  fi

  if [ -d "$WORK/whisper.cpp/.git" ]; then
    git -C "$WORK/whisper.cpp" pull --quiet --ff-only || true
  else
    git clone --depth 1 --quiet https://github.com/ggml-org/whisper.cpp "$WORK/whisper.cpp"
  fi

  cmake -S "$WORK/whisper.cpp" -B "$WORK/whisper.cpp/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF \
    -DGGML_BACKEND_DL=OFF \
    -DGGML_METAL=ON \
    -DGGML_METAL_EMBED_LIBRARY=ON \
    -DGGML_OPENMP=OFF \
    -DWHISPER_BUILD_TESTS=OFF \
    -DWHISPER_BUILD_SERVER=OFF \
    > /dev/null
  cmake --build "$WORK/whisper.cpp/build" --config Release -j > /dev/null

  rm -f "$OUT/whisper-cli-$TARGET"
  cp "$WORK/whisper.cpp/build/bin/whisper-cli" "$OUT/whisper-cli-$TARGET"
  chmod +x "$OUT/whisper-cli-$TARGET"

  # Nothing outside /usr/lib or /System, or it is not self-contained and the
  # Mac it is going to will say so instead of this script.
  local left
  left="$(otool -L "$OUT/whisper-cli-$TARGET" | awk 'NR>1 {print $1}' \
    | grep -vE "^\s*(/usr/lib|/System)" || true)"
  if [ -n "$left" ]; then
    echo "   still needs something from outside:"
    echo "$left" | sed 's/^/     /'
    exit 1
  fi
  echo "   self-contained"
}

say "yt-dlp"
have yt-dlp --version && echo "   already here" || fetch_ytdlp

say "ffmpeg"
have ffmpeg -version && echo "   already here" || fetch_ffmpeg

say "whisper-cli"
have whisper-cli --help && echo "   already here" || build_whisper

say "done"
du -sh "$OUT" | sed 's/^/   /'
ls -la "$OUT" | awk 'NR>1 {printf "   %6.1f MB  %s\n", $5/1048576, $NF}'
