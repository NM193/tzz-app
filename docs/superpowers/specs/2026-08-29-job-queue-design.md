# Sequential job queue

**Status:** approved 2026-08-29

## Problem

The app does one job per click. Turning a ten-lecture course into transcripts
means sitting at the machine, waiting for each download to finish, and pasting
the next link. The work is unattended by nature -- a three-hour lecture takes
minutes of Whisper time -- so the person should be able to queue the whole
course and walk away.

## What we build

Paste a list of links, press Download, and the app works through them one at a
time. Failures do not stop the run. A Stop button ends the queue after the
current item.

Out of scope: parallel jobs (the GPU is already saturated by one Whisper run),
reordering the queue, persisting it across restarts, per-item settings.

## Where the queue lives

In the frontend, not in Rust.

The commands are already one-job-per-call, progress is already emitted on a
global channel, and the requirement is strictly sequential execution. That
makes the queue a loop in `App.tsx` over the existing `runJob` and
`transcribeFile`. **The backend does not change at all** -- no new commands, no
Tauri-managed state, no new event shapes.

A Rust-side queue would need managed state, a mutex, and a second event channel
to report per-item status. It would buy robustness the tool does not need.

The cost is that the queue is lost if the webview reloads. For work the user
watches while it runs, that is acceptable.

## Model

    type QueueItem = {
      id: string;                    // crypto.randomUUID()
      kind: "url" | "file";
      label: string;                 // the link, or the file name
      status: "waiting" | "running" | "done" | "failed";
      result?: JobResult;
      error?: string;
    };

`kind` is what lets one queue hold both YouTube links and dropped files: the
runner picks `runJob` or `transcribeFile` from it.

## Input

The URL field becomes a textarea, one link per line. On Download the text is
parsed: trim each line, drop blanks, drop duplicates, keep only lines that look
like links. Parsing lives in a pure `parseUrlList(text): string[]` so the rule
is in one place.

Dropped files join the same queue. The drag-drop handler currently takes
`paths[0]` and ignores the rest; with a queue, every dropped path becomes an
item.

## Running

The loop sets each item to `running`, awaits the call, then marks it `done`
with its `JobResult` or `failed` with the error string, and moves on. Existing
progress events keep driving the meter for whichever item is current; a counter
above it reads "3 / 10".

Nothing is retried. A failed item keeps its message so the user can see what
went wrong -- a deleted video and a network drop read differently.

When the loop ends, a summary line reports how many finished and how many
failed.

## Stopping

A `useRef` flag, checked between items. The button reads "Stop after this one"
so it is clear the current lecture runs to completion.

Killing a running yt-dlp or Whisper mid-flight was considered and rejected: it
needs a process handle held across the command boundary and leaves partial
files behind, for a case the user can wait out.

## Results

Each finished row carries its own small "Show MD" and "Show PDF" buttons, so
all ten outputs stay reachable. The existing result panel keeps showing the
most recent finished job with its transcript preview and Copy button.

## Testing

This is frontend work and the project has no JS test runner. Adding one for a
single pure function is not worth the dependency, so `parseUrlList` is verified
by hand alongside the rest:

- ten links pasted at once queue in order and run one at a time
- a bad link fails, is marked, and the queue continues
- Stop ends the run after the current item, not immediately
- dropping several files queues all of them
- `npm run build:vite` typechecks
