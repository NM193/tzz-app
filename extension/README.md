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

1. Open any lesson of the course, in the Chrome profile you are signed in with.
2. Click the extension → **Collect the course**.
3. **Copy all**, then paste into Tzz App's link field.

**Just this lesson** does only the one you are looking at, and reads it off the
page rather than asking for the course -- useful if a course is arranged in a
way the first button does not understand.

Nothing navigates and nothing opens in a new tab: you stay on the lesson you
were on. The popup can be closed while it works; what it found is kept, and
reopening it shows where things got to.

## How it reads the course

Through the course player's own API, the same one the page uses: one request
lists every chapter and lesson under its real name, and one request per lesson
carries the Wistia id. Your browser's cookies make those requests
authenticated, so it sees exactly what you see and nothing more.

The first version instead opened every lesson in turn and read the player out
of the DOM. That is a worse idea in four separate ways, written up at the top
of `collect.js`; the short of it is that it found three lessons in a course of
forty and gave them all the same name.

## What it does and does not do

It has no server and no account, and the only things it writes are the
clipboard and its own list, kept in Chrome so it survives closing the popup.

It needs you to be signed in, because it only ever asks for what your browser
already has. It does not get past anything you could not open yourself.
