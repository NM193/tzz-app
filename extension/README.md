# Tzz Collector

A Chrome extension that turns a Thinkific course into a list Tzz App can take:

```
Standard i očekivanja u majčinstvu | https://fast.wistia.net/embed/iframe/t8tfr1p7aj
```

## Why it has to exist

Thinkific builds its video player with JavaScript, so a lesson's video id only
exists while the page is running. It is not in the HTML the server sends --
which is why `yt-dlp` answers "Unsupported URL" even when it is signed in as
you. Only something inside the browser can see it.

## Install

1. Chrome → `chrome://extensions`
2. Turn on **Developer mode** (top right)
3. **Load unpacked** → choose this `extension` folder

## Use

1. Open the course in the Chrome profile you are signed in with.
2. Click the extension.
3. **Collect the course** — it opens each lesson in turn and reads its video
   id. One tab, in place; it puts you back where you started when it finishes.
   **Just this lesson** does only the one you are on.
4. **Copy all**, then paste into Tzz App's link field.

## What it does and does not do

It reads the page you opened. It makes no requests of its own, has no server
and no account, and the only thing it writes is the clipboard and its own list,
kept in Chrome so it survives closing the popup.

It needs you to be signed in, because it only ever sees what your browser
already shows you. It does not get past anything you could not open yourself.
