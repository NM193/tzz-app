/*
  The walk itself, in the background.

  WHY not in the popup: Chrome closes a popup the moment the tab navigates,
  and everything running in it dies with it. That is why the first version
  stopped on lesson one. So this is a state machine, not a loop: the state
  lives in storage, and each finished page load wakes it for the next step.
  A background worker may be shut down between events -- this survives that.
*/

const EMBED = (id) => `https://fast.wistia.net/embed/iframe/${id}`;

const read = () => chrome.storage.local.get(null);
const write = (patch) => chrome.storage.local.set(patch);

/* ---- what runs inside the page ---- */

/** Every lesson link on the page, once each, in the order they are listed. */
function readLessons() {
  const seen = new Set();
  return [...document.querySelectorAll('a[href*="/lessons/"]')]
    .map((a) => ({ url: a.href.split("#")[0], name: (a.textContent || "").trim() }))
    .filter(({ url }) => {
      if (seen.has(url)) return false;
      seen.add(url);
      return true;
    });
}

/**
 * The lesson's name and the id of its video.
 *
 * Wistia marks its container with `wistia_async_<id>` before anything plays,
 * and leaves the id in the playlist address once it does. The player is built
 * after the page is drawn, so this waits for it rather than looking once.
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
    }, 15000);
  });
}

/**
 * Run a function in every frame of the tab and take the first real answer.
 *
 * `allFrames` is the part that matters: a course player often puts the lesson
 * in an iframe, and the top document then holds nothing but chrome. Injecting
 * only into the top frame finds a page with no video on it every time.
 */
async function inPage(tabId, fn) {
  try {
    const frames = await chrome.scripting.executeScript({
      target: { tabId, allFrames: true },
      func: fn,
    });
    const answers = frames.map((frame) => frame?.result).filter(Boolean);
    // A list answer is the longest one; a lesson answer is the one with an id.
    if (Array.isArray(answers[0])) {
      return answers.reduce((best, list) => (list.length > best.length ? list : best), []);
    }
    return answers.find((answer) => answer.id) ?? answers[0] ?? null;
  } catch {
    // The tab went somewhere the extension cannot reach; the walk says so.
    return null;
  }
}

/* ---- the walk ---- */

/**
 * Keep what a lesson turned out to be -- including that it had no video.
 *
 * A lesson with nothing to download is worth showing: a course has text and
 * quiz pages too, and a silently short list looks like a broken walk.
 */
async function remember(name, id, from) {
  const { found = [] } = await read();
  const url = id ? EMBED(id) : null;
  if (url && found.some((lesson) => lesson.url === url)) return;
  await write({ found: [...found, { name: name || "Untitled", url, from }] });
}

async function start(tabId) {
  const list = await inPage(tabId, readLessons);
  if (!list?.length) {
    return write({ status: "No lessons on this page. Open the course contents first." });
  }

  const tab = await chrome.tabs.get(tabId);
  await write({
    running: true,
    tabId,
    queue: list,
    at: 0,
    home: tab.url,
    status: `Found ${list.length} lessons. Reading the first…`,
  });
  await chrome.tabs.update(tabId, { url: list[0].url });
}

/*
  A page can report itself loaded more than once -- Thinkific redirects on its
  way in. Without this the walk would read one lesson twice and skip the next.
*/
let busy = false;

/** One page has finished loading: read it, then move on. */
async function step(tabId) {
  if (busy) return;
  const state = await read();
  if (!state.running || state.tabId !== tabId) return;
  busy = true;
  try {
    await walkOne(tabId, state);
  } finally {
    busy = false;
  }
}

async function walkOne(tabId, state) {

  const lesson = state.queue[state.at];
  if (!lesson) return finish(state);

  const answer = await inPage(tabId, readLesson);
  // The sidebar's own text is the lesson's name; a heading on the page is as
  // likely to be the course's.
  await remember(lesson.name || answer?.name, answer?.id, lesson.url);


  const at = state.at + 1;
  if (at >= state.queue.length) return finish({ ...state, at });

  await write({ at, status: `Lesson ${at + 1} of ${state.queue.length}…` });
  await chrome.tabs.update(tabId, { url: state.queue[at].url });
}

async function finish(state) {
  const { found = [] } = await read();
  const withVideo = found.filter((lesson) => lesson.url).length;
  await write({
    running: false,
    status: `${withVideo} of ${state.queue.length} lessons have a video.`,
  });
  // Put the tab back where it started, so the walk leaves nothing behind.
  if (state.home) await chrome.tabs.update(state.tabId, { url: state.home });
}

chrome.tabs.onUpdated.addListener(async (tabId, info) => {
  if (info.status !== "complete") return;
  await step(tabId);
});

chrome.runtime.onMessage.addListener((message, _sender, respond) => {
  (async () => {
    if (message.type === "walk") await start(message.tabId);

    if (message.type === "one") {
      await write({ status: "Reading this lesson…" });
      const tab = await chrome.tabs.get(message.tabId);
      const answer = await inPage(message.tabId, readLesson);
      await remember(answer?.name, answer?.id, tab.url);
      await write({ status: answer?.id ? "Done." : "No Wistia video on this page." });
    }

    if (message.type === "stop") {
      await write({ running: false, status: "Stopped." });
    }

    respond(true);
  })();
  return true;
});
