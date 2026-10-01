# Updating the app, and putting it on someone else's Mac

**Status:** agreed 2026-09-29, not started. Decision pending: where the update
files are hosted.

## What is wanted

1. The app updates itself. On launch it asks a JSON on the internet what the
   newest version is; if it is behind, a panel says so, shows what changed, and
   offers to fetch it. One click, then it restarts as the new version.
2. It runs on a second Mac -- the owner's wife's -- not only the machine it was
   built on.

This reverses the decision of 2026-09-17 ("no updater and no signing: this app
lives on one machine"), which held until there was a second machine.

## What updating needs

**A signing key.** Tauri refuses an update that is not signed with the owner's
private key; that is what stops anyone who can reach the URL from serving a
different app. Generated once with `npx tauri signer generate`. **The private
key is the owner's and is never handled here** -- only the public half goes in
`tauri.conf.json`.

**Somewhere to put two files**, per release: the bundle and `latest.json`.

- **GitHub Releases** is the least work, but the repo has to be *public* for
  the download to work without a token. The repo has no remote today.
- **Vercel** keeps the repo private: the bundle and the JSON sit there as
  static files. A little more to do by hand each release.

**A release step**: raise the version, `npm run build`, upload the bundle, its
signature and `latest.json`. One script once it is set up.

## What the second Mac needs, which is the harder half

Updating is the easy part. Two things actually stand in the way:

- **Gatekeeper.** The app is not signed with an Apple certificate, so macOS
  calls it damaged on first open. Right-click -> Open gets past it once. Doing
  it properly needs an Apple Developer account (99 USD a year) and
  notarisation.
- **The tools are not in the app.** `yt-dlp`, `ffmpeg` and `whisper-cli` are
  found on the system at runtime (`services/binaries.rs`), and a fresh Mac has
  none of them, nor a Whisper model. Either she runs
  `brew install yt-dlp ffmpeg whisper-cpp` and is handed a model, or they are
  bundled into the app -- about 100 MB larger, and then it simply works.
  `swiftc` is not needed on her machine: the OCR helper is compiled into the
  bundle at build time.

## Order agreed

1. The updater, hosted wherever the owner decides.
2. Bundling the tools, which is what makes the app giftable at all.
3. Apple signing, only if the right-click on first open becomes annoying.

## Open decision

**GitHub public, or Vercel with a private repo.** Nothing starts until that is
answered.
