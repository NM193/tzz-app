# From transcripts to study material

**Status:** superseded 2026-09-27. The app will not build summaries, courses
or quizzes: NotebookLM does them for free and well enough. What Tzz builds
instead is the best possible *source* -- a transcript with timestamps,
chapters, a table of contents and the text that was on screen -- which is more
than NotebookLM can gather from a YouTube link on its own, since it reads only
the captions and never sees the picture.

The reasoning below is kept because it still explains why the transcript is
shaped the way it is. Revisit it only if NotebookLM's limits start to bite, or
notes are wanted in Serbian, offline, in your own words.

## What this becomes

Point the app at ten YouTube lectures on a subject. It collects what was said
and what was shown, then produces something you can actually learn from: the
substance, not the ramble. On top of the raw transcript it offers a summary, a
structured course, and a quiz to test yourself.

The problem being solved is not access to the material -- it is that an hour of
lecture usually contains fifteen minutes of substance, and finding that fifteen
minutes costs the hour.

**For personal study, not redistribution.** The output is a study aid built from
material the user has access to; it is not someone else's course repackaged as
your own.

## What already works

The pipeline underneath is built and measured, on this machine:

| Piece | Status | Measured |
|---|---|---|
| Download and MP3 | working | -- |
| Transcript from captions | working | instant |
| Transcript from Whisper | working | 3.5 h audio in ~7 min (large-v3-turbo) |
| Queue over many videos | working | strictly sequential |
| One combined document per queue | working | 3 lectures, 37k words |
| Screen text from video (OCR) | proven, not built in | 18 min video in ~22 s; 0.5 s per frame |

The visual half was proven on the user's own example -- a Webflow framework
course. Frames are extracted only where the screen changes, then read with
Apple's Vision OCR: local, free, no install. On that video it recovered the
class names being taught (`u-theme-dark`, `page_wrap`, `Global Styles`), the
panel being configured (`Visual Video`, `Autoplay`, `Loop`), and the section
title slides.

Two lessons from that experiment:

- **Screens repeat.** The application chrome is identical in every frame, so a
  raw dump is mostly noise. Showing only what changed since the last screen
  turns it into signal. This is the same problem as the scrolling captions, and
  the same fix.
- **OCR reads text, not meaning.** A slide of code or a settings panel comes
  through perfectly. A diagram, a photograph, an unlabelled chart gives
  nothing. Covering those needs a vision model, which costs money per image.

## What is missing

Everything above is mechanical: transcription, extraction, formatting. Summary,
course structure and quizzes are not -- they need a language model, which this
app has never had.

That is the one real decision in front of us, and it shapes everything else:

**A local model** (llama.cpp, same family as Whisper) keeps the app free,
private and offline. A 3-hour lecture is roughly 50k tokens of transcript,
which is a lot to ask of a small local model, and quality on "extract the
substance" varies.

**An API** (Claude, GPT) gives markedly better summaries and quizzes for
roughly $0.15-0.50 per lecture, so a ten-lecture course costs a few dollars.
It needs a key, it sends the transcript off the machine, and it makes the app
dependent on a service.

Nothing else about the design can be settled before this is.

## Open questions

- What is a "course" here -- chapters with key points, or something closer to
  a lesson plan with objectives?
- What kind of quiz: multiple choice, open questions, flashcards?
- Should the quiz be answerable in the app, or is a document enough?
- Does the summary cite timestamps back into the transcript, so a claim can be
  traced to where it was said?
- One document per lecture, or one course document from all ten?

## Suggested order

1. **Screen text into the transcript.** Finish the visual half first: it is
   proven, local, free, and it makes everything downstream better. A summary
   built without knowing what was on screen misses half a Webflow lecture.
2. **Decide the model question.** Local or API. Try one lecture both ways and
   compare the summaries before committing.
3. **Summary.** The smallest useful step: the substance of one lecture, with
   timestamps.
4. **Course and quiz.** Only once summaries are good, because they are built on
   the same understanding.

Doing 4 before 1 would produce a course that never mentions what the lecturer
was pointing at.
