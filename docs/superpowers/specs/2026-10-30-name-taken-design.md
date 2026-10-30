# A name the library already has

**Status:** built 2026-10-30.
**Prototype:** `.superpowers/prototype/tzz-name-taken.html`

## What was found

Every job writes into a folder of its own in the library, and nothing asks
whether that folder is already there:

- **A YouTube link run again** goes into the same `Title [id]` folder, and
  its transcript is written over the old one without asking
  (`audio_path.with_extension("md")`). The notes survive, because
  `<transcript>.notes.md` keeps its name. There is no way to keep both.
- **A local file** gets a folder named after its stem. Two different
  `Lecture 1.mp4`, from two folders, share one library folder: `mp3.rs` runs
  ffmpeg with `-y`, so the second MP3 overwrites the first, and so does the
  transcript. The first file's `screens.json` is even reused for the second
  when the second is the older file, since `screen.rs` judges the cache fresh
  by modification time alone.
- **A recording** is saved through the macOS save dialog, which already asks
  before replacing. Only its transcription, which is a local file job, is
  covered here.

**Fixed first, in `245fdba`.** A run that failed after its download left the
raw audio (`….webm`) in its folder, and the next run failed with HTTP 416:
with `--no-part`, yt-dlp took the finished file for an interrupted one and
tried to resume past its end. `download_audio` now passes `--no-continue`.
Measured on 2026-10-30 with the bundled yt-dlp. A leftover file is fetched
again, and a finished MP3 beside a file of the same name is still recognised,
leaving that file alone. This design counts such a folder as not taken, which
is only safe because a retry now works.

## Decided

When Start (or Transcribe in the language dialog) would send a job into a
folder that already holds a result, one dialog lists every such job before
anything runs. After it, the queue runs to the end without stopping. A
question in the middle of a queue would stall an hour of lectures at the
seventh one while nobody is at the Mac. The update notice waits for the same
reason.

### What you see

```
Already in the library

Updates: Lumos for Webflow, Astro, Chrome Extension, and More
FINISHED 24 OCT · MP3 · TRANSCRIPT · NOTES     [•New name•][ Replace ][ Skip ]
[ Updates: Lumos for Webflow, Astro, Chrome Extension, and More (2)     ]

Lecture 1.mp4
FROM ~/DOWNLOADS/WEEK 1 · MP3 · TRANSCRIPT     [ New name ][•Replace•][ Skip ]

Lecture 1.mp4
FROM ~/DOWNLOADS/WEEK 2 · SAME NAME AS ANOTHER IN THIS START
                                               [•New name•][ Replace ][ Skip ]
[ Lecture 1 (3)                                                          ]

                                                       Cancel     [Start]
```

- **Taken** means the folder this job would write into exists and holds an
  MP3 or a transcript (`.md` other than `.notes.md`, or `.pdf`). Not taken,
  and the job goes in as it does today:
  - a folder holding only leftovers of a failed run, a kept video, or notes;
  - the folder a file is being made again from. A file dropped from its own
    library folder (`Lecture 1/Lecture 1.mp3`, or a kept
    `Title [id]/Title [id].mp4`) would be deleted by Replace before it was
    read. `mp3.rs` already treats that case as "already where it belongs".
- **Two jobs in one Start** that would land in the same folder clash with
  each other, even when the library has no such folder. The first of them
  is not asked about unless the library has the folder too. Every later one
  is: its state line says `SAME NAME AS ANOTHER IN THIS START`, and its
  Replace is disabled, since it would overwrite what this run had just made.
- **Names are compared without regard to case**, as APFS compares them, so
  `Lecture 1.mp4` and `lecture 1.mp4` clash.
- **New name** is the default, so Enter loses nothing. Its field is filled
  with the first free `Name (2)`, `Name (3)`, …: free in the library, and not
  used by any other job in this Start, above or below. It can be edited. A
  name is refused if it is empty or begins with a dot after sanitising (`.`,
  `..`, or a folder the Library would hide), or if it is taken by the library
  or by another job in this Start. A refused name marks its field and its
  state line in the accent, and Start waits.
- **Replace** clears the folder when the queue reaches that job, not when it
  is chosen. A queue stopped before then leaves the old results untouched.
  From the moment that job starts, the old results are gone: a Stop or a
  failure after that leaves the folder without them. Clearing deletes every
  file in the folder except `*.notes.md`. The notes are the one thing in the
  folder the app cannot make again, and they reattach because the new
  transcript takes the old one's name. The state line names what goes,
  built from what the folder holds: `MP3, TRANSCRIPT, VIDEO AND SCREEN TEXT
  GO · NOTES STAY`, with only the parts that are there. A kept video is
  about a gigabyte to fetch again, and reading the screen again costs about
  twelve minutes an hour. Replace is not offered when another job in this
  Start reads its file from that folder, such as a kept video dropped
  beside its own link.
- **Skip** leaves the job out of the queue. Its state line reads `NOT RUN`.
- **Cancel** or Escape starts nothing. The pills stay on Convert. A recording
  stays saved where it is.
- **The state line** is read, never clicked: mono, uppercase, dim. A link's
  row says when its folder last changed and what it holds (`FINISHED 24 OCT ·
  MP3 · TRANSCRIPT · NOTES`). A local file's row says which folder the file
  comes from instead of the date (`FROM ~/DOWNLOADS/WEEK 1 · MP3 ·
  TRANSCRIPT`), since two rows can carry the same file name.

The dialog is the language prompt's sheet (`.prompt`): the question in the
serif, names and buttons in the sans, the same wipe from below. A row that
has chosen New name grows its field beneath it.

**What the new name names.** For a local file it names the folder, the MP3
and the transcript, and becomes the transcript's title. For a link it
becomes the job's title, which is how a pasted name works today. The folder
is then `New name [id]`, and the `[id]` stays because `thumb.rs` finds the
cover through it. The MP3 inside keeps yt-dlp's name.

**Which jobs are checked.** Local files and recordings always. A link only
when its video id is known, which it is once its title has been read or it
came out of a playlist. A link whose title is still being read, whose read
failed, or whose name was given in the paste runs as it does today.

**One check at a time.** Start is disabled while a check is running, so it
cannot send a batch twice. A batch that arrives while the dialog is open,
such as a recording stopped from the menu bar, waits. It is checked when
the dialog closes.

## How it works

**Rust, `services/library.rs`** (it reads the library already):

1. `folder_name(name, id)`: the name a job's folder gets. It is moved out
   of `commands/media.rs` so the job and the check cannot disagree. The name
   is sanitised, cut to fit, and trimmed, and then ` [id]` is appended for a
   link. It fits when the whole folder name, `[id]` included, is at most 120
   characters. That is today's longest name, so the byte length stays where
   it is. Cutting before the `[id]` is added keeps the `[id]`, which today's
   order loses on a long non-YouTube title. Every name short enough to keep
   its `[id]` today gets the same folder as before, so existing folders keep
   their names. YouTube titles are at most 100 characters. `library_folder`
   and `job_folder` call it and keep creating the folder.
2. `suggest(name, id, n)`: `<name> (n)`, with the name cut and trimmed
   first so the whole of it fits the same 120 characters. A suggestion therefore comes
   back unchanged through `folder_name`, whether it travels as a row's
   `name`, a file's `LocalFileRequest.name`, or a link's title. If the cut
   came afterwards, a long name would lose its `(2)` on the way, and the
   app would refuse its own suggestion.
3. `holds_result(folder)`: the folder holds an MP3 or a transcript.
4. `clashes(root, items)`: one result per item, in order. Each result says:
   - the folder name;
   - what has the folder: the library, the key of an earlier item, or
     nothing;
   - whether Replace is allowed;
   - the suggested free name;
   - what the folder holds: MP3, transcript, video, screen text, notes, and
     when it last changed.

   A folder is not taken when the item's own source file is inside it.
   The source's parent and the folder are compared by device and inode, the
   way `mp3.rs`'s `same_file` compares files, because APFS names that differ
   only in case are one folder. Replace is not allowed for a later item of
   the same folder, nor when another item in the batch reads its file from
   that folder: one job's Replace must not delete the next one's source.
   Within the batch, names compare case-folded. It is pure apart from
   reading the folders.
5. `clear_for_replace(root, folder)`: deletes every file in `folder` except
   `*.notes.md`. It refuses any folder that is not a direct child of `root`.

**Command:** `library_clashes(outDir, items)` in `commands/library.rs`,
validation only. An item is `{ key, kind, path?, title?, videoId?, name? }`:

- a file sends its `path`, and Rust takes the stem the way
  `transcribe_file_inner` does;
- a link sends its `title` and `videoId`;
- either sends `name` when it is a New name row: the name in its field,
  whether suggested or typed.

**The dialog's protocol.** The first call sends the whole batch. The rows
are the items that came back taken, and the dialog keeps them for as long
as it is open. On every edit or change of choice it calls again with the
batch as it would run: Skip rows left out, Replace rows under their own
name, New name rows under the name in their field, every other item as it
is. Later calls only mark fields and enable or disable Start. They never add
or remove rows.

A clash found on a re-check is pinned on a row. Only a row's name can have
changed, so the row is always the cause:

- between a row and an item without one, whichever of the two the result
  marks, the row is refused;
- between two rows, the one just edited is refused, or else the later one.

Each result names the key it clashes with, which is what makes this
possible. A Replace row's own result will say the library has its folder.
That is what Replace means, and it is never a refusal.

**Requests:** `JobRequest` gains `replace`. `LocalFileRequest` gains `name`
and `replace`. A job with `replace` calls `clear_for_replace` right after
its folder is known, before anything is fetched. A local job with `name`
uses it for the folder, the files and the title.

**Frontend:**

- `queue.ts`: `PendingLink` gains `videoId`, set by `readTitle` from the
  probe and by `unpack` from the playlist item, which already carries `id`.
  `QueueItem` gains `replace` and `name`. A link's new name goes into
  `label` with `named: true`, the way a pasted name travels today. A file's
  goes into `name`.
- `useJobs.ts`: `runOne` sends `replace` for both kinds, and `name` for a
  file.
- `api.ts`: `libraryClashes(…)` and its types, mirroring the Rust structs.
  `transcribeFile` and `runJob` carry the new fields.
- `NamePrompt.tsx` (new): the dialog, in the shape of `LanguagePrompt.tsx`.
- `App.tsx` owns one way into the queue: check, ask if anything clashes,
  then `jobs.enqueue`. `ConvertView`'s Start and the language dialog's
  Transcribe both go through it, in place of calling `jobs.enqueue`
  themselves. Convert's pills are cleared once the jobs are admitted, not on
  Start, and only the batch's own, by id: a file dropped while the dialog
  is open stays as a pill.
- `styles.css`: the row's field, from the existing tokens.

## Not in scope

- Jobs already waiting in a running queue. Convert hides Start while a
  queue runs, so only a recording can arrive then, and it is checked
  against the library and against its own batch.
- A remembered choice, or one choice for all rows.
- Finding an earlier copy filed under another name (`Title (2) [id]`).
  Taken means this folder, by this name.
- The combined document of a queue. It keeps stepping aside as it does.

## Done when

- `cargo test` covers:
  - `folder_name` and `suggest`: today's names are unchanged; on a name of
    200 characters, `(2)` and `(3)` stay distinct and the `[id]` stays; a
    suggestion fed back through `folder_name` comes out unchanged;
  - `holds_result`;
  - `clashes`: the library; two jobs in one Start; case; suggestions that
    skip names used anywhere in the batch; Replace only for the first, and
    not when another item reads from the folder; a source file inside its
    own folder is not taken; each clash names the key it clashes with;
  - `clear_for_replace`: notes survive, and a folder outside the root is
    refused.

  `npm run build:vite` is clean.
- On the installed app:
  - a finished YouTube link run again asks. New name makes a `(2)` folder
    beside the first. Replace keeps the notes, and they show against the new
    transcript. Skip runs nothing for it.
  - two different `Lecture 1.mp4` started together give two folders.
  - a file dropped from its own library folder is not asked about.
  - a link that failed, run again, is not asked about and finishes.
- `CHANGELOG.md`, under Unreleased, New: "When something you start is
  already in the library, the app asks whether to give it a new name,
  replace it, or skip it."
