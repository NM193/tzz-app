/*
  The popup only shows what the background has collected and asks it to start.
  It holds no state of its own, because Chrome closes it whenever the tab
  navigates -- which is exactly what collecting a course does.
*/

const $ = (id) => document.getElementById(id);

async function tab() {
  const [current] = await chrome.tabs.query({ active: true, currentWindow: true });
  return current;
}

function draw(state) {
  const found = state.found ?? [];

  $("list").innerHTML = found
    .map(
      (lesson, index) =>
        `<li ${lesson.url ? "" : 'class="none"'}><i>${String(index + 1).padStart(2, "0")}</i><span></span></li>`,
    )
    .join("");
  // Names are written as text, never as markup: they come from a web page.
  $("list").querySelectorAll("span").forEach((cell, index) => {
    cell.textContent = found[index].name;
    // Which page it read, so a wrong walk can be seen rather than guessed.
    cell.title = found[index].from ?? "";
  });

  $("status").textContent = state.status ?? "";
  $("copy").disabled = !found.some((lesson) => lesson.url);
  $("walk").textContent = state.running ? "Stop" : "Collect the course";
  $("one").disabled = !!state.running;
}

chrome.storage.local.get(null).then(draw);
chrome.storage.onChanged.addListener(() => chrome.storage.local.get(null).then(draw));

$("walk").onclick = async () => {
  const state = await chrome.storage.local.get(null);
  const current = await tab();
  chrome.runtime.sendMessage({ type: state.running ? "stop" : "walk", tabId: current.id });
};

$("one").onclick = async () => {
  const current = await tab();
  chrome.runtime.sendMessage({ type: "one", tabId: current.id });
};

$("clear").onclick = () => chrome.storage.local.set({ found: [], status: "" });

$("copy").onclick = async () => {
  const { found = [] } = await chrome.storage.local.get("found");
  await navigator.clipboard.writeText(
    found
      .filter((lesson) => lesson.url)
      .map((lesson) => `${lesson.name} | ${lesson.url}`)
      .join("\n"),
  );
  $("status").textContent = "Copied. Paste it into Tzz App.";
};

tab().then((current) => {
  $("where").textContent = /thinkific\.com/.test(current?.url ?? "")
    ? "Open the course contents so every lesson is listed, then collect."
    : "Open a Thinkific course page first.";
});
