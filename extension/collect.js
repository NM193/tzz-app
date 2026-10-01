/*
  The two functions that run inside the course page.

  Chrome sends an injected function to the page as source text, so each one has
  to stand alone: no helper from this file, no variable from around it. That is
  why the small pieces are nested rather than shared.

  WHY the player's API and not a walk: the first version of this opened every
  lesson in turn and read the player out of the DOM. It was wrong in every way
  that matters. Thinkific redirects on its way into a lesson, so a page reports
  itself loaded twice and the walk reads one lesson and skips the next. The
  player is built after the page is drawn, so there is nothing to find until it
  is. The sidebar is only in the DOM while it is open, so a closed one looks
  like a three-lesson course -- which is exactly what it looked like. And a
  popup dies the moment its tab navigates, which is the one thing a walk does
  constantly.

  The API has none of those problems: one request lists the whole course under
  its real names, one request per lesson carries the Wistia id, the tab never
  moves, and the browser's own cookies make it authenticated for free. Nothing
  is read off the screen at all.

  `collectLesson` still reads the DOM, because for the page you are already
  standing on that is the shortest road and it needs no API to exist.
*/

/** Every lesson of the course, in the order it is taught, with its video. */
async function collectCourse() {
  const slug = /\/courses\/take\/([^/?#]+)/.exec(location.pathname);
  if (!slug) return { error: "This is not a course page. Open a lesson first." };
  const api = `${location.origin}/api/course_player/v2`;

  const json = async (url) => {
    const response = await fetch(url, {
      credentials: "include",
      headers: { accept: "application/json", "x-requested-with": "XMLHttpRequest" },
    });
    if (!response.ok) throw new Error(`${response.status} from ${new URL(url).pathname}`);
    return response.json();
  };

  /*
    Thinkific names the video field differently depending on how the video was
    added, and the shape around it differs again. So this does not walk the
    object: it reads the whole thing as text and takes the first id that is
    labelled like one. A Wistia id is ten characters of lowercase and digits.
  */
  const idIn = (data) => {
    const text = JSON.stringify(data);
    const patterns = [
      /"wistia_(?:video_)?id"\s*:\s*"([a-z0-9]{8,12})"/,
      /wistia_async_([a-z0-9]{8,12})/,
      /(?:fast\.)?wistia\.(?:net|com)\/(?:embed\/)?(?:iframe|medias)\/([a-z0-9]{8,12})/,
      /"identifier"\s*:\s*"([a-z0-9]{8,12})"/,
    ];
    for (const pattern of patterns) {
      const hit = pattern.exec(text);
      if (hit) return hit[1];
    }
    return null;
  };

  let course;
  try {
    course = await json(`${api}/courses/${slug[1]}`);
  } catch (caught) {
    return { error: `The course player would not answer: ${caught.message}` };
  }

  /*
    The listing is flat: chapters hold ids and the contents are a separate
    array. Following the chapters is what puts the lessons in the order the
    course is taught in; the contents array is in no order worth having.
  */
  const byId = new Map((course.contents ?? []).map((item) => [item.id, item]));
  const ordered = (course.chapters ?? [])
    .flatMap((chapter) => chapter.content_ids ?? [])
    .map((id) => byId.get(id))
    .filter(Boolean);
  const contents = ordered.length ? ordered : (course.contents ?? []);
  if (!contents.length) return { error: "The course player listed no lessons." };

  const lessons = [];
  for (const [index, content] of contents.entries()) {
    // Nobody may be listening, and that is not a reason to stop collecting.
    chrome.runtime
      .sendMessage({ type: "progress", status: `Lesson ${index + 1} of ${contents.length}…` })
      .catch(() => {});

    let id = idIn(content);
    if (!id) {
      try {
        id = idIn(await json(`${api}/course_contents/${content.id}`));
      } catch {
        // A lesson that will not open is reported as having no video, below.
      }
    }

    lessons.push({
      name: (content.name || "Untitled").trim(),
      url: id ? `https://fast.wistia.net/embed/iframe/${id}` : null,
    });
  }

  return { lessons };
}

/**
 * The video on the page in front of you, read off the page itself.
 *
 * Wistia marks its container `wistia_async_<id>` before anything plays and
 * leaves the id in the media address once it does. The player is built after
 * the page is drawn, so this waits for it instead of looking once.
 */
function collectLesson() {
  const idNow = () => {
    const marked = document.querySelector('[class*="wistia_async_"]');
    const fromClass = marked && /wistia_async_([a-z0-9]{8,12})/i.exec(marked.className);
    if (fromClass) return fromClass[1];
    const source = document.querySelector('source[src*="wistia"], video[src*="wistia"]');
    const fromSource =
      source && /medias\/([a-z0-9]{8,12})/i.exec(source.getAttribute("src") || "");
    return fromSource ? fromSource[1] : null;
  };

  const name = () => {
    const heading = document.querySelector("h1, h2, .lesson-heading");
    return (heading && heading.textContent.trim()) || document.title.split("|")[0].trim();
  };

  const answer = (id) =>
    id
      ? { lessons: [{ name: name(), url: `https://fast.wistia.net/embed/iframe/${id}` }] }
      : { error: "No Wistia video on this page." };

  return new Promise((resolve) => {
    const now = idNow();
    if (now) return resolve(answer(now));

    const observer = new MutationObserver(() => {
      const id = idNow();
      if (!id) return;
      observer.disconnect();
      resolve(answer(id));
    });
    observer.observe(document.documentElement, { childList: true, subtree: true });

    setTimeout(() => {
      observer.disconnect();
      resolve(answer(idNow()));
    }, 15000);
  });
}
