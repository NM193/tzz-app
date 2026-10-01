/*
  Walks a Thinkific course and collects each lesson as "Name | link".

  Nothing leaves the browser. The list goes to the clipboard and you paste it
  into Tzz App yourself; the extension has no server, no account and no way to
  reach anything but the tab you opened it on.

  WHY this has to live in the browser at all: Thinkific builds its player with
  JavaScript, so a lesson's video id exists only while the page is running. It
  is not in the HTML the server sends, which is why no downloader can find it
  from outside -- not even with your sign-in.
*/

const $ = (id) => document.getElementById(id);
const EMBED = (id) => `https://fast.wistia.net/embed/iframe/${id}`;

let found = [];

async function tab() {
  const [current] = await chrome.tabs.query({ active: true, currentWindow: true });
  return current;
}

/** Run a function inside the page and hand back what it returns. */
async function inPage(tabId, fn) {
  const [frame] = await chrome.scripting.executeScript({ target: { tabId }, func: fn });
  return frame?.result ?? null;
}

/* ---- what runs inside the page. Each one stands alone: an injected
       function cannot see anything in this file. ---- */

/** Every lesson in the sidebar, once each, in the order they are listed. */
function readLessons() {
  const seen = new Set();
  return [...document.querySelectorAll('a[href*="/lessons/"]')]
    .map((a) => ({ url: a.href, name: (a.textContent || "").trim() }))
    .filter(({ url }) => {
      if (seen.has(url)) return false;
      seen.add(url);
      return true;
    });
}

/**
 * The lesson's name and the id of its video.
 *
 * Wistia marks its container with `wistia_async_<id>` before anything is
 * played, and leaves the id in the playlist address once it is. The class is
 * tried first because it is there sooner. The player is built after the page
 * is drawn, so this waits for it rather than looking once.
 */
function readLesson() {
  const idNow = () => {
    const marked = document.querySelector('[class*="wistia_async_"]');
    const fromClass = marked && /wistia_async_([a-z0-9]+)/i.exec(marked.className);
    if (fromClass) return fromClass[1];

    const source = document.querySelector('source[src*="wistia"], video[src*="wistia"]');
    const fromSource =
      source && /medias\/([a-z0-9]+)\./i.exec(source.getAttribute("src") || "");
    return fromSource ? fromSource[1] : null;
  };

  const name = () => {
    const heading = document.querySelector("h1, h2, .lesson-heading");
    const shown = heading && heading.textContent.trim();
    return shown || document.title.split("|")[0].trim();
  };

  return new Promise((resolve) => {
    const now = idNow();
    if (now) return resolve({ name: name(), id: now });

    const observer = new MutationObserver(() => {
      const id = idNow();
      if (!id) return;
      observer.disconnect();
      resolve({ name: name(), id });
    });
    observer.observe(document.documentElement, { childList: true, subtree: true });

    setTimeout(() => {
      observer.disconnect();
      resolve({ name: name(), id: idNow() });
    }, 12000);
  });
}

/* ---- the popup itself ---- */

function draw() {
  $("list").innerHTML = found
    .map(
      (lesson, index) =>
        `<li><i>${String(index + 1).padStart(2, "0")}</i><span></span></li>`,
    )
    .join("");
  // Names are written as text, never as markup: they come from a web page.
  $("list").querySelectorAll("span").forEach((cell, index) => {
    cell.textContent = found[index].name;
  });
  $("copy").disabled = found.length === 0;
  chrome.storage.local.set({ found });
}

const say = (message) => ($("status").textContent = message);

function remember(name, id) {
  const url = EMBED(id);
  if (found.some((lesson) => lesson.url === url)) return;
  found.push({ name: name || id, url });
  draw();
}

async function collectHere() {
  const current = await tab();
  say("Reading this lesson…");
  const answer = await inPage(current.id, readLesson);
  if (!answer?.id) return say("No Wistia video on this page.");
  remember(answer.name, answer.id);
  say("Done.");
}

/**
 * Open each lesson in turn and read its id.
 *
 * One tab, navigated in place. Opening twenty at once would ask the site for
 * everything at the same moment, and would have the course count every lesson
 * as started.
 */
async function walk() {
  const current = await tab();
  say("Reading the lesson list…");

  const list = await inPage(current.id, readLessons);
  if (!list?.length) return say("No lessons here. Open the course first.");

  const home = current.url;
  for (const [index, lesson] of list.entries()) {
    say(`Lesson ${index + 1} of ${list.length}…`);
    await go(current.id, lesson.url);
    const answer = await inPage(current.id, readLesson);
    if (answer?.id) remember(answer.name || lesson.name, answer.id);
  }

  await go(current.id, home);
  say(`${found.length} of ${list.length} lessons have a video.`);
}

/** Navigate the tab and wait for it to finish loading. */
function go(tabId, url) {
  return new Promise((resolve) => {
    const done = (id, info) => {
      if (id !== tabId || info.status !== "complete") return;
      chrome.tabs.onUpdated.removeListener(done);
      resolve();
    };
    chrome.tabs.onUpdated.addListener(done);
    chrome.tabs.update(tabId, { url });
  });
}

$("walk").onclick = () => walk().catch((error) => say(String(error)));
$("one").onclick = () => collectHere().catch((error) => say(String(error)));

$("clear").onclick = () => {
  found = [];
  draw();
  say("");
};

$("copy").onclick = async () => {
  await navigator.clipboard.writeText(
    found.map((lesson) => `${lesson.name} | ${lesson.url}`).join("\n"),
  );
  say("Copied. Paste it into Tzz App.");
};

chrome.storage.local.get("found").then((kept) => {
  found = kept.found ?? [];
  draw();
});

tab().then((current) => {
  $("where").textContent = /thinkific\.com/.test(current?.url ?? "")
    ? "Open the course contents, then collect."
    : "Open a Thinkific course page first.";
});
