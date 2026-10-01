/*
  The popup shows what the worker collected and asks it to collect more. It
  keeps nothing of its own: it can be closed at any moment, including in the
  middle of a course, and reopening it shows where things got to.
*/

const $ = (id) => document.getElementById(id);

const tab = async () => {
  const [current] = await chrome.tabs.query({ active: true, currentWindow: true });
  return current;
};

function draw(state) {
  const found = state.found ?? [];

  $("list").innerHTML = found
    .map(
      (_, index) =>
        `<li><i>${String(index + 1).padStart(2, "0")}</i><span></span></li>`,
    )
    .join("");
  // Names are written as text, never as markup: they come off a web page.
  $("list").querySelectorAll("li").forEach((row, index) => {
    row.querySelector("span").textContent = found[index].name;
    if (!found[index].url) row.className = "none";
  });

  $("status").textContent = state.status ?? "";
  $("error").textContent = state.error ?? "";
  $("copy").disabled = !found.some((lesson) => lesson.url);
  $("course").disabled = !!state.running;
  $("lesson").disabled = !!state.running;
}

chrome.storage.local.get(null).then(draw);
chrome.storage.onChanged.addListener(() => chrome.storage.local.get(null).then(draw));

const ask = (type) => async () =>
  chrome.runtime.sendMessage({ type, tabId: (await tab()).id });

$("course").onclick = ask("course");
$("lesson").onclick = ask("lesson");
$("clear").onclick = ask("clear");

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
  $("where").textContent = /thinkific\.com\/courses\/take\//.test(current?.url ?? "")
    ? "Collect reads the whole course without leaving this page."
    : "Open a lesson of the course first.";
});
