/*
  The worker does the collecting; the popup only shows what it found.

  WHY here and not in the popup: a popup closes the instant you click outside
  it, and whatever it was awaiting dies with it. A course of forty lessons
  takes long enough that this happens by accident. So the popup asks, the
  worker works, and the answer goes to storage -- where the popup finds it
  whether it was open at the time or not.
*/

const store = (patch) => chrome.storage.local.set(patch);

/**
 * Run one of the collectors in the tab and keep what it says.
 *
 * `collect.js` is injected first so its functions exist in the page, then the
 * named one is called there. Both run in the same isolated world, which is
 * what lets the second call see what the first defined.
 */
async function collect(tabId, which) {
  await store({ running: true, status: "Asking the course player…", error: null });
  try {
    await chrome.scripting.executeScript({ target: { tabId }, files: ["collect.js"] });
    const [frame] = await chrome.scripting.executeScript({
      target: { tabId },
      func: (name) => window[name](),
      args: [which],
    });

    const answer = (await frame.result) ?? {};
    if (answer.error) return store({ running: false, status: "", error: answer.error });

    const found = answer.lessons ?? [];
    const withVideo = found.filter((lesson) => lesson.url).length;
    await store({
      running: false,
      error: null,
      found,
      status: `${withVideo} of ${found.length} lessons have a video.`,
    });
  } catch (caught) {
    // Usually the tab is not a Thinkific page, so the extension cannot reach it.
    await store({ running: false, status: "", error: caught.message });
  }
}

chrome.runtime.onMessage.addListener((message, _sender, respond) => {
  if (message.type === "progress") {
    store({ status: message.status });
    return false;
  }

  (async () => {
    if (message.type === "course") await collect(message.tabId, "collectCourse");
    if (message.type === "lesson") await collect(message.tabId, "collectLesson");
    if (message.type === "clear") await store({ found: [], status: "", error: null });
    respond(true);
  })();
  return true;
});
