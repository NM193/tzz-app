# Tzz App

A macOS app that turns lectures into readable material. Give it a YouTube link,
a playlist, or an audio or video file from your Mac, and it makes an MP3, a
transcript with timestamps and chapters, and -- for screen-share videos -- the
text that was on screen. It can also record any audio input, including the
Mac's own sound.

The transcript is markdown and PDF, one folder per video in
`~/Documents/Tzz Library`. It is meant as a better source for NotebookLM, or
for whatever you read it with, than a bare YouTube link: it carries the
timestamps, the chapters and the screen text that a link does not.

## Getting it

Download the `.dmg` from the
[latest release](https://github.com/NM193/tzz-app/releases/latest), open it,
and drag **Tzz App** into Applications.

It needs a Mac with Apple Silicon (M1 or later) and macOS 13 or later.

### The first time you open it

The app is not signed by Apple, so macOS will not open it on the first try.

- **macOS 15 and later:** open it once and let macOS refuse. Then go to
  **System Settings → Privacy & Security**, scroll down to the line about Tzz
  App, and click **Open Anyway**.
- **macOS 13 and 14:** Control-click the app in Applications, choose **Open**,
  and confirm.

This is needed only once. Updates arrive inside the app afterwards.

### Before the first transcription

- **A Whisper model.** Videos with captions need nothing. For everything else
  -- local files, recordings, videos without captions -- the app transcribes
  on your Mac with Whisper. Open **Settings** and pick a model; the app fetches
  it. Large v3 Turbo (1.6 GB) is the one to keep: on Apple Silicon it is about
  as fast as Small.
- **BlackHole**, only to record the Mac's own sound, for example a call. It
  has to be installed by hand; **Record** links to it when it is missing.

### Updates

The app checks for a new version when it opens and says what changed. Nothing
is downloaded until you click **Update**. [CHANGELOG.md](CHANGELOG.md) has the
same notes.

## Building it

Needs: the Xcode command line tools (`xcode-select --install`, for `swiftc`),
Rust, Node, and cmake (`brew install cmake`).

```bash
npm install
bash scripts/fetch-tools.sh   # yt-dlp, ffmpeg and whisper-cli, inside the app
npm run dev                   # the app, with the frontend reloading
npm run build                 # a .app and a .dmg
```

`npm run build` makes the updater archive as well, which is signed with the
release key. Without that key, add
`-- --config '{"bundle":{"createUpdaterArtifacts":false}}'`.

How the code is laid out, how a release is made and why things are the way
they are: [CLAUDE.md](CLAUDE.md).

---

Downloading from YouTube goes against its terms of service. Sensible uses are
your own uploads, Creative Commons material, and transcripts for your own study
of things you have the right to watch.
