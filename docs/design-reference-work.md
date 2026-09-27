# Design reference: the "Work" archive page

Extracted 2026-09-26 from another project of Nemanja's (a Next.js marketing
site). **Nothing in that project was changed, run or installed** -- this is a
read-only transcription of its design decisions.

Content is replaced with placeholders throughout: `{Item}`, `{Group}`,
`{Kind}`. What is described here is the *mechanism* -- measurements, states,
timings and the reasoning -- so it can be rebuilt on any stack. It happens to
be React + CSS there; nothing here depends on that.

Source, for anyone who needs the original:
`Next Projects/next arihtecture` ->
`docs/superpowers/specs/2026-09-26-work-page-design.md` (spec),
`src/features/projects/components/work/` (implementation),
`.superpowers/work-prototype/work-final.html` (the prototype it was designed
in; only the variants `data-cs="b"`, `data-cc="d"`, `data-sv="c"` are live).

---

## 1. What the page is

A searchable archive. Roughly two dozen items, all present at once, filtered
in the browser. The visitor finds one in seconds by name, group or topic, and
chooses how densely to look at them: a large grid, a dense grid, or a list.

Two rules hold the whole design together:

- **Nothing paginates and nothing loads.** Every item is in the page from the
  start; filtering is pure computation over an in-memory index. There is no
  spinner anywhere in this design, because there is nothing to wait for.
- **One motion idea, used everywhere: the wipe from below.** Cards open
  bottom-to-top, list rules draw left-to-right, the cursor image opens
  bottom-to-top. Nothing fades in from nowhere, nothing bounces, nothing
  parallaxes. The page has one gesture and repeats it.

---

## 2. Tokens

### 2.1 Colour

All colours live in exactly one file. Nothing else defines a colour.

| Token | Value | Use |
|---|---|---|
| `--bg` | `#181616` | page background, warm near-black |
| `--bg-deep` | `#0e0d0d` | transitions, dark chips |
| `--card` | `#100e0e` | behind an image while it loads |
| `--bone` | `#fffddc` | primary text, light pills, selected states |
| `--muted` | `#7a726c` | dimmed words in headings -- **large text only** (3.82:1) |
| `--muted-2` | `#bdb5ac` | body text |
| `--caption` | `#8a827b` | small text (4.77:1) |
| `--line` | `#2a2626` | rules and hairlines |
| `--line-2` | `#3a3533` | edges of ghost controls |
| `--accent` | `#f7ab29` | the only accent -- focus ring, hovered arrow, selection |

One accent, one background, three greys for text. The discipline is the
point: `--muted` is *not* allowed on small text, and the file says so next to
the value.

Two colours are not tokens because they appear once:
- search field interior `#1d1a1a` (one step lighter than `--bg`);
- the count inside a selected category `#6b645e` (a grey that survives on
  `--bone`).

Highlight for a search hit: `rgb(247 171 41 / .25)` -- the accent at 25%, as a
background behind the matched letters, text colour untouched.

### 2.2 Type

| Role | Family | Notes |
|---|---|---|
| Display / headings / list titles | Instrument Serif, 400, normal + italic | the only serif; it carries the page's character |
| UI and body | Geist | 400 and 500 only |
| Numbers, counts, tags, keycaps | Geist Mono | 500, small sizes, letter-spacing `.1em`–`.14em` |

Two families plus a mono for figures. Headings never use the sans; UI never
uses the serif. The split is absolute, which is why the page reads as
editorial rather than as an app.

Scale actually used:

| Element | Size / line-height |
|---|---|
| Page title | `3rem / 0.95`, letter-spacing `-0.03em` |
| List title, large rows | `clamp(1.6rem, 2.4vw, 2.4rem) / 1.05`, `-0.015em` |
| List title, medium rows | `1.5rem` |
| List title, dense rows | `1.1rem` |
| Empty state | `2.2rem` serif |
| Lede | `0.875rem / 1.55` |
| Search input | `0.9375rem` |
| Category row | `0.875rem / 1.25`, weight 500 |
| View buttons | `0.8125rem`, weight 500 |
| Card title | `1rem` weight 500 (3 cols `0.9375rem`, 4 cols `0.875rem`) |
| Card group | `0.875rem` (3 cols `0.8125rem`, 4 cols `0.75rem`) |
| Counts, row numbers | `0.75rem` mono |
| Tags, keycap | `0.625rem` / `0.6875rem` mono, uppercase |

### 2.3 Size system: everything in rem

The prototype was measured at a fixed 16px root. The real page scales its root
font with the viewport:

```css
@media (min-width: 1024px) {
  html { font-size: clamp(13px, min(max(16px, 100vw / 90), 100svh / 59.25), 30px); }
}
```

- width term: `1rem = viewport / 90` (16px at 1440px), never below 16px;
- height term: the tallest section is `59.25rem`, so a short window shrinks
  everything until it fits;
- clamped to 13–30px.

**The conversion rule:** every px measurement from the design becomes rem by
dividing by 16. That includes animation offsets (`72px` -> `4.5rem`).

Exceptions:
- hairlines and borders of 1–2px stay in px;
- percentages and `vw` stay as they are;
- values computed in JS read the live root size:
  `parseFloat(getComputedStyle(document.documentElement).fontSize) || 16`.

### 2.4 Space, radius, motion

Spacing steps in use: `0.125 · 0.25 · 0.375 · 0.5 · 0.625 · 0.75 · 0.875 · 1 ·
1.25 · 1.5 · 1.75 · 2.25 · 2.75 · 3.5 · 5rem`.

| Radius | Where |
|---|---|
| `2px` | mini-preview tiles, highlight background |
| `0.25rem` (4px) | cards, thumbnails, tags, keycap |
| `0.4375rem` (7px) | mini preview, view pill |
| `0.5rem` (8px) | search field, category pill, hover outline, cursor image |
| `0.625rem` (10px) | the Grid/List box |
| `50%` / `999px` | stepper buttons, clear button, phone chips |

| Easing | Curve | Used for |
|---|---|---|
| `EASE` | `cubic-bezier(.2,.8,.2,1)` | almost everything: pills, FLIP, rises |
| `WIPE` | `cubic-bezier(.7,0,.2,1)` | clip-path reveals and the drawn rule |
| `EASE_OUT` | `cubic-bezier(.4,0,1,1)` | exits only |
| intro | `cubic-bezier(.22,.9,.24,1)` | the page's own entrance, 750ms, `translateY(24px)` |

| Duration | What |
|---|---|
| 160ms | lede fades out |
| 180ms | old view fades out when switching Grid/List |
| 200 / 220ms | row exit / card exit |
| 250ms | border colour on focus |
| 300ms | text colour, cursor image hides |
| 350ms | keycap shrink |
| 420ms | old count leaves; lede height |
| 450ms | view pill slides; clear button spins in |
| 480ms | new lede rises |
| 500ms | row padding and title size (row density) |
| 520ms | new count enters; count width; cursor image opens |
| 550ms | category pill slides |
| 600ms | mini-preview tiles re-flow; card text rises |
| 650ms | FLIP on filtering; new cursor image over the old |
| 750ms | FLIP on a column change |
| 850ms | list row parts rise |
| 900ms | card frame wipe; list rule draws |
| 1300ms | card image settles from scale 1.25 |

Stagger: 60ms per card, 45ms per row, 35ms per part within a row, 80ms per
text line within a card.

---

## 3. Layout

```
┌──────────────────────────────────────────────────────────┐
│  pinned bar (sticky, top: 0)                             │  ← hairline appears once scrolled
├───────────────┬──────────────────────────────────────────┤
│ 16rem         │  results                                 │
│ (sticky)      │                                          │
│               │   ┌─────────┐ ┌─────────┐                │
│ Title (N)     │   │  item   │ │  item   │   grid 2/3/4   │
│ lede          │   └─────────┘ └─────────┘                │
│ [ search   / ]│        …or a list of rows…               │
│ All        23 │                                          │
│ {Group A}   9 │                                          │
│ {Group B}   5 │                                          │
│ …             │                                          │
│ [Grid | List] │                                          │
│ −  ▦▦  +      │   [ primary call to action ]             │
└───────────────┴──────────────────────────────────────────┘
```

### 3.1 Desktop (>= 1024px / 64rem)

```css
.work-layout { display: grid; gap: 2.25rem; }
@media (min-width: 64rem) {
  .work-layout {
    grid-template-columns: 16rem minmax(0, 1fr);
    gap: 3.5rem;
    align-items: start;
  }
  .work-aside { position: sticky; top: 7.5rem; }  /* JS refines `top` */
}
```

- **Left column is 16rem and sticks.** Contents in order: title with the live
  count, lede, search, categories, Grid/List, then the density control for
  whichever view is active.
- **The sticky offset is computed**, because a short window cannot fit the
  column below the bar:

  ```js
  const top = Math.min(menuHeight() + 1.5 * rootPx(),
                       window.innerHeight - aside.offsetHeight - rootPx());
  aside.style.top = `${top}px`;   // may be negative on purpose
  ```

  A negative `top` means the column scrolls with the page until its *bottom*
  is on screen, and only then pins. Recomputed on window resize and on a
  `ResizeObserver` over the column itself (the column's height changes when
  the lede wraps).
- There is no "23 ITEMS" meta line. **The count lives in the title**, in
  brackets, and animates -- one place, not two.
- Empty state sits in the results column: "Nothing matches that.", serif
  `2.2rem`, `--muted-2`, centred, `5rem` of vertical padding. The lede
  simultaneously says *what* was not found.

### 3.2 Below 1024px

- Single column, same order, **not sticky**. The bar stays pinned.
- Categories become **one horizontally scrolling row of chips**, bleeding to
  both screen edges:

  ```css
  .work-cats-real {
    display: flex; gap: 0.5rem; overflow-x: auto; scrollbar-width: none;
    margin: 0 calc(-1 * var(--work-gutter));
    padding: 0 var(--work-gutter);
  }
  ```

  Chip: height `2.25rem`, padding `0 0.875rem`, `1px` `--line-2`, radius
  `999px`, `0.8125rem`. Selected: background and border `--bone`, text `--bg`.
- The `/` keycap is hidden. The sliding pill and the hover outline are hidden.
  The density steppers are hidden.
- Grid: 1 column below 768px, 2 columns 768–1023px, regardless of the stored
  choice.
- List rows get a thumbnail on the left (`4.75rem`, 16:10) instead of the
  number, title and group stack, arrow on the right, no tags. No cursor image.
- The grid column must be `minmax(0, 1fr)` or the chip row widens the page.

### 3.3 The pinned bar

```css
.site-curtain:has([data-pin-nav]) > header {
  position: sticky;
  top: var(--lock-y, 0px);
  background: var(--bg);
  transition: box-shadow 0.3s;
}
html[data-scrolled] .site-curtain:has([data-pin-nav]) > header {
  box-shadow: 0 1px 0 var(--line);
}
```

- The page opts in by putting `data-pin-nav` on its own root element; the
  layout reacts with `:has()`. No prop drilling, no layout-level flag.
- `sticky`, not `fixed`: the bar sits inside a "curtain" element that lifts
  away at the end of the page to reveal the footer. A fixed bar would float
  over that footer.
- The hairline is driven by a single attribute toggled on scroll:
  `root.toggleAttribute('data-scrolled', window.scrollY > 4)`.

---

## 4. Components

### 4.1 Title with a rolling count

`{Page title} (N)` -- serif `3rem`, the bracketed number in `--muted` italic.

When N changes, the old number leaves and the new one arrives, and the
bracket does not jump because the container's width animates too.

- old: `translateY(∓90%)` + fade out, **420ms**, `EASE`, removed on finish;
- new: from `translateY(±90%)` + transparent, **520ms**, `EASE`, `backwards`;
- direction: up when the number grows, down when it shrinks;
- width: from the old width to the new width, **520ms**.

The container clips and reserves its own padding so digits are not sheared:

```css
.work-odo { position: relative; display: inline-block; vertical-align: top;
            overflow: hidden; padding: 0 .08em; margin: 0 -.08em; }
.work-odo > [data-old] { position: absolute; left: .08em; top: 0; }
```

The framework renders only the current number; the outgoing one is created in
the DOM by hand in a layout effect and deleted when its animation ends.

### 4.2 Lede that never moves anything

One sentence under the title, reacting to the filter:

| Situation | Text |
|---|---|
| no query | `{Description} for {tail of the selected group}.` |
| with a query | `{Description} for {tail}, matching “{q}”.` |
| nothing found | `Nothing here matches “{q}” yet.` |

Each group carries its own `tail` string, so the sentence stays grammatical
("for {Kind} teams", "for studios and agencies").

**The height is reserved.** Before anything animates, a hidden clone of the
paragraph -- same width, same font -- is measured against *every* group's
no-query sentence, and the tallest becomes `min-height`. Switching groups
therefore never shifts the search field below it.

```js
const probe = el.cloneNode();
probe.style.cssText =
  `position:absolute;visibility:hidden;min-height:0;width:${el.getBoundingClientRect().width}px`;
el.after(probe);
let max = 0;
for (const t of measure) { probe.textContent = t; max = Math.max(max, probe.offsetHeight); }
probe.remove();
el.style.minHeight = `${max}px`;
```

Re-measured on a `ResizeObserver` over the parent and after `document.fonts.ready`.

Change sequence: old fades out and lifts `0.375rem` (**160ms**, `EASE_OUT`);
then the text swaps; then the new one rises `0.625rem` into place
(**480ms**, `EASE`). Only a long query can need an extra line -- then the
height itself animates over **420ms**, so there is still no jump.

### 4.3 Search field that types its own examples

Box: height `3rem`, padding `0 0.75rem 0 0.875rem`, `1px` `--line-2`, radius
`0.5rem`, interior `#1d1a1a`, cursor `text`. On focus the border becomes
`--accent` over 250ms. Inside, left to right: a `1rem` magnifier in
`--caption`, the input, the ghost example, the `/` keycap, the clear button.

**Idle typewriter.** While the field is empty *and* unfocused, a ghost span
types out example queries with a blinking caret:

- phrases: `Try “{example A}”`, `Try “{example B}”`, … (five of them);
- 65ms per letter in, hold 1500ms, 28ms per letter out, 300ms pause, next;
- stops the moment the field is focused or has text.

The real `placeholder` is empty and the ghost is `aria-hidden`; the input
carries an `aria-label`. With reduced motion there is no typewriter and the
placeholder is a plain string instead.

```js
const tick = () => {
  const text = EXAMPLES[word];
  el.textContent = text.slice(0, length);
  if (step > 0 && length === text.length) { step = -1; timer = setTimeout(tick, 1500); return; }
  if (step < 0 && length === 0) { step = 1; word = (word + 1) % EXAMPLES.length;
                                  timer = setTimeout(tick, 300); return; }
  length += step;
  timer = setTimeout(tick, step > 0 ? 65 : 28);
};
```

The caret is CSS only:

```css
.work-search-example::after {
  content: ''; display: inline-block; width: 1px; height: 1.05em;
  margin-left: 2px; vertical-align: -2px; background: var(--muted-2);
  animation: work-caret 1s steps(1) infinite;
}
@keyframes work-caret { 50% { opacity: 0; } }
.work-search-example:empty::after { display: none; }
```

**Keyboard.** `/` anywhere focuses the field -- but not while the focus is in
another input, not with a modifier, and not while the menu is open. `Esc`
clears and blurs, but only when the focus is already inside (the document
listener for `Esc` belongs to the menu). `Enter` just blurs.

**The keycap-to-clear swap.** With text present, the `/` keycap shrinks to
`scale(.5)` and fades (350ms), and in its place the `×` button unrolls from
`scale(.5) rotate(-90deg)` to normal over **450ms**. It is `tabIndex={-1}`
while invisible, `pointer-events: none`, and returns focus to the input after
clearing. Both are driven by one attribute on the wrapper:

```css
.work-search[data-has] .work-search-key   { opacity: 0; transform: scale(0.5); }
.work-search[data-has] .work-search-clear { opacity: 1; transform: none; pointer-events: auto; }
```

### 4.4 Categories: a pill that is a window

Desktop. `All` plus the groups, each with the number of items that match the
**current query** -- so the numbers move as you type. A group at 0 drops to
`opacity: .35` but stays clickable.

Row: `0.875rem` weight 500, padding `0.625rem 0.75rem`, radius `0.5rem`, gap
`0.125rem`; the count is `0.6875rem` mono in `--caption`.

The selected row is marked by a **light pill in `--bone` that slides**
(transform and height, 550ms `EASE`). The trick: rather than recolouring text
under the pill, the pill *contains a second, dark copy of the entire list*
and moves it in the opposite direction. The text under the pill is dark
exactly where the pill covers it -- during the slide as well.

```css
.work-cats-pill { position: absolute; inset: 0 0 auto; z-index: 2;
                  overflow: hidden; border-radius: .5rem; background: var(--bone);
                  transition: transform .55s var(--work-ease), height .55s var(--work-ease); }
.work-cats-copy { position: absolute; inset: 0 0 auto;
                  transition: transform .55s var(--work-ease); }
.work-cats-copy button { color: var(--bg); }
```

```js
const y = btn.getBoundingClientRect().top - wrap.getBoundingClientRect().top;
pill.style.transform = `translateY(${y}px)`;
pill.style.height    = `${btn.offsetHeight}px`;
copy.style.transform = `translateY(${-y}px)`;   // equal and opposite
```

The copy is `inert` and `aria-hidden`, so neither Tab nor a screen reader ever
reaches the duplicate.

**Hover** is a separate 1px outline (`box-shadow: inset 0 0 0 1px var(--line-2)`)
that slides to whichever row the pointer is over and fades out when the
pointer leaves the list. Same positioning maths, `opacity` as the on/off.

Both the pill and the outline must be placed **without a transition** when the
cause is a resize rather than a click: set `transition: none`, write the
values, force a reflow (`void el.offsetHeight`), restore. The first
`ResizeObserver` callback fires immediately after observing and must be
skipped, or it cancels the very first slide.

### 4.5 Grid | List

A box with `1px` `--line-2`, radius `0.625rem`, padding `0.25rem`, two equal
buttons of height `2.25rem` with an icon and a label. A `--bone` pill slides
under the selected one over **450ms**:

```css
.work-views-pill { position: absolute; top: .25rem; left: .25rem;
                   width: calc(50% - .375rem); height: 2.25rem; border-radius: .4375rem;
                   background: var(--bone); transition: translate .45s var(--work-ease); }
.work-views[data-view='list'] .work-views-pill { translate: calc(100% + .25rem) 0; }
```

The selected button's text turns `--bg`; `aria-pressed` follows the state.

Switching views: the outgoing view fades over **180ms** (`EASE_OUT`,
`fill: forwards`), and only in its `onfinish` does the new view mount and play
its own entrance. If the visitor had scrolled past the top of the results,
the page scrolls back to the top of the results, minus the height of the
pinned bar.

### 4.6 Density control ("mini preview")

One control at a time, both occupying the same `3.875rem`-tall slot. The
inactive one fades to 0 and slides `0.875rem` sideways (`opacity` .2–.35s,
`translate` .5s `EASE`), plus `visibility` delayed by the slide so it cannot
be clicked while leaving. No label -- the preview explains itself.

Shape: a round `−` button, a miniature of the layout, a round `+` button.

- buttons: `1.75rem` circle, `1px` `--line-2`; disabled at the ends
  (`opacity: .3`);
- preview: `4.875rem × 3rem`, `1px` `--line-2`, radius `0.4375rem`, and it
  **fades out at the bottom** via `mask-image: linear-gradient(#000 55%, transparent)`
  -- which implies the layout continues past the frame;
- clicking the preview advances to the next value and wraps around;
- **`+` always means "more items on screen"**, whichever control it is. For
  columns that is more columns; for rows that is *smaller* rows. The
  `aria-label`s say "More columns" / "Smaller rows" accordingly.

**Columns (grid): 2 · 3 · 4.** Control sits on the left, aligned to the Grid
button. The preview holds 12 tiles at 16:10 in N columns; when N changes the
tiles FLIP to their new places over 600ms.

**Rows (list): 1 · 2 · 3.** Control sits on the right, aligned to the List
button. The preview holds 8 rows whose height (`0.875` / `0.5625` /
`0.375rem`) animates over 500ms, with the "title" bar scaling with it:

```css
.work-mini-list > span::before {
  width: 52%; height: max(2px, calc(var(--rh) * .34));
  transition: height .5s var(--work-ease);
}
```

**Width caps the choice.** 1024–1279px allows 2–3 columns, >= 1280px allows
2–4. What renders is `effective = min(chosen, max for this width)`, and `−`,
`+` and the preview all operate on the *effective* value and store the
result. So a stored 4 shows as 3 on a narrow screen, `+` is disabled, and `−`
goes to 2 -- while the stored 4 survives in the URL until deliberately
changed.

### 4.7 Grid card: the mask reveal

Image at a fixed aspect ratio, radius `0.25rem`, `object-position: top`. An
item with no image is a solid block in its own colour. Below: a row with the
title (`1rem`/500) and the group (`0.875rem`, `--caption`), then tags
(`0.625rem` mono, uppercase, `1px` `--line`, radius `0.25rem`).

Entrance, once the element scrolls into view (and not before the page's own
intro has finished):

```js
frame.animate([{ clipPath: `inset(100% 0 0 0 round ${R})` },
               { clipPath: `inset(0 0 0 0 round ${R})` }],
              { duration: 900, delay, easing: WIPE, fill: 'backwards' });
frame.querySelector('img').animate([{ transform: 'scale(1.25)' }, { transform: 'scale(1)' }],
              { duration: 1300, delay, easing: EASE, fill: 'backwards' });
texts.forEach((t, k) => t.animate(RISE,
              { duration: 600, delay: delay + 350 + k * 80, easing: EASE, fill: 'backwards' }));
```

- the frame opens bottom-to-top; the mask uses the **same radius as the card**,
  or the corners flash square;
- the image starts 25% oversized and settles -- the reveal feels like a camera,
  not a curtain;
- text rises `0.625rem` with a fade, staggered 80ms per line, starting 350ms
  in, so it arrives while the image is still settling;
- cards entering together stagger 60ms.

Exit (filtered away): `clip-path` closes downward over **220ms**, `EASE_OUT`.
Re-order: FLIP over **650ms**; genuinely new cards use the mask entrance.
Hover: image `scale(1.03)` over 900ms. Column-count changes FLIP with scale
over **750ms**, `transform-origin: 0 0`.

Grid spacing per column count:

| Columns | Gap (row / column) | Title | Group |
|---|---|---|---|
| 2 | `3.5rem / 1.5rem` | `1rem` | `0.875rem`, right-aligned |
| 3 | `2.75rem / 1.25rem` | `0.9375rem` | `0.8125rem` |
| 4 | `2.25rem / 1rem` | `0.875rem` | `0.75rem`, under the title, tags hidden |

At 4 columns the title/group row switches from `space-between` to a stacked
column -- there is no longer width for two things on one line.

### 4.8 List row

Desktop columns: number (mono `0.75rem`) · title (serif) · group
(`0.875rem`, `--muted-2`) · tags (fixed `13.5rem`, right-aligned, only from
1280px) · arrow.

```css
.work-row-link {
  display: grid;
  grid-template-columns: 2.75rem minmax(0, 1.3fr) minmax(0, 1fr) 1.5rem;
  gap: 1.25rem; padding: 1.375rem 0;
  transition: padding .5s var(--work-ease);
}
@media (min-width: 80rem) {
  .work-row-link { grid-template-columns: 2.75rem minmax(0,1.3fr) minmax(0,1fr) 13.5rem 1.5rem; }
}
```

- **The number is the item's permanent place in the catalogue** (01, 02, …),
  not its position in the current results. It is an identifier, not a counter,
  so filtering does not renumber the world.
- Fixed column widths mean the group text sits on the same vertical line in
  every row -- the thing that makes a list read as a table without drawing one.
- The rule under each row is a `::after` layer with `transform-origin: left`,
  so it can be drawn.

Entrance:

- the rule draws from the left: `scaleX(0 -> 1)`, **900ms** `WIPE`, animated
  through `pseudoElement: '::after'`;
- the row's parts rise out of the row itself: `translateY(4.5rem) -> 0`,
  **850ms** `EASE`, +35ms per part -- the row has `overflow: hidden`, so they
  genuinely emerge from behind its edge;
- rows stagger 45ms.

Exit: fade and `-0.75rem` sideways, 200ms.

Hover (pointer devices only) is a three-part gesture:

```css
@media (hover: hover) {
  .work-list:hover .work-row-title             { color: rgb(255 253 220 / .35); }
  .work-list .work-row:hover .work-row-title   { color: var(--bone); }
  .work-list .work-row:hover .work-row-link    { padding-left: .75rem; }
  .work-list .work-row:hover .work-row-arrow   { color: var(--accent); translate: .25rem 0; }
}
```

Every *other* title dims -- the hovered one does not brighten, the rest
retreat. That reads as focus rather than as a highlight.

Row densities (padding and title size animate over 500ms):

| Density | Padding | Title | Notes |
|---|---|---|---|
| 1 · large | `1.375rem` | `clamp(1.6rem, 2.4vw, 2.4rem)` | |
| 2 · medium | `0.875rem` | `1.5rem` | |
| 3 · dense | `0.375rem` | `1.1rem` | group `0.8125rem`, tags `1px 0.3125rem` |

The dense list is meant as an index: the entire catalogue fits a 900px-tall
window. That only holds if the arrow has `line-height: 1` -- otherwise the
arrow's own line box sets the row height and the target is missed.

### 4.9 Cursor image (mouse only)

A `23.75rem` 16:10 panel, radius `0.5rem`, heavy shadow
(`0 1.875rem 3.75rem -1.25rem rgb(0 0 0 / .7)`), `position: fixed`,
`pointer-events: none`, `visibility: hidden` until used.

- follows the cursor with a lag: `p += (t - p) * 0.16` per frame, through one
  shared ticker rather than its own `requestAnimationFrame` loop;
- offset `2rem` to the right of the cursor, vertically centred on it, and
  always kept `1rem` inside the viewport;
- **never slides under the pinned bar**: its top limit is the bar's
  `getBoundingClientRect().bottom + 1rem`;
- appears with the same wipe from below (**520ms** `WIPE`), hides upward
  (**300ms**);
- moving to another row stacks a *new* image over the old one -- it opens from
  below while scaling 1.2 -> 1 over **650ms** -- and at most 3 layers are kept;
- the follow loop stops itself once the pointer has left and the panel has
  caught up (`|dx| + |dy| < 0.5`), instead of running forever.

```js
const aim = (e) => {
  const r = rootPx();
  const top = Math.max(0, menu?.getBoundingClientRect().bottom ?? 0) + r;
  const bottom = Math.max(top, innerHeight - box.offsetHeight - r);
  tx = Math.min(e.clientX + 2 * r, innerWidth - box.offsetWidth - r);
  ty = Math.min(bottom, Math.max(top, e.clientY - box.offsetHeight / 2));
};
```

### 4.10 Column entrance

After the page's own intro, the left column rises in the same way as the rest
of the site, with hand-set delays: title words 90 + i·55ms, lede 200ms, then
search 380ms, categories 470ms, view switch 560ms, density control 650ms.

---

## 5. Search: ranking rules

Each item is indexed into four word groups -- lowercased, diacritics stripped,
split on anything that is not a letter or digit:

| Group | Source |
|---|---|
| name | the item's title |
| group | its industry/kind label |
| categories | the names of its categories |
| body | its summary and highlights |

The query is split into words. **Every query word must hit something**, or the
item is out. Each word scores for the best group it hits:

| Hit | Points |
|---|---|
| a name word starts with the query word | 60 |
| a group word starts with it | 30 |
| a category word starts with it | 25 |
| body: 3+ letters -> word prefix; exactly 2 -> whole word only; 1 -> never | 10 |

Plus **100** if the full name (words joined by spaces) starts with the full
query. Ordering: score descending, ties broken by the catalogue order. No
query means everything, in catalogue order. The category filter is applied
*before* scoring, and the counts shown beside each category are "how many
items of that category survive the current query".

```js
const points =
  anyStartsWith(entry.title, token) ? 60 :
  anyStartsWith(entry.industry, token) ? 30 :
  anyStartsWith(entry.category, token) ? 25 :
  token.length >= 3 && anyStartsWith(entry.body, token) ? 10 :
  token.length === 2 && entry.body.includes(token) ? 10 : 0;
if (points === 0) return 0;
```

The one-letter rule in the body is the interesting part: without it, typing
`L` matches an item whose description happens to contain "L.A." and the
result list becomes noise. Two letters must be a whole word, so `AI` and `UX`
work but random fragments do not.

**Highlighting is deliberately narrower than matching.** Only the name and the
group get `<mark>`, because a single letter matching inside a paragraph would
light up half the page. The highlighter splits on word boundaries and tries
the longest query token first.

---

## 6. State in the URL

Everything the visitor chose is in the query string, so a filtered view can be
sent to someone: `?q=…&c=…&view=list&cols=3&rows=2`.

- defaults are never written (`c=all`, `view=grid`, `cols=2`, `rows=1`, empty
  query);
- written with `replaceState`, so the back button is not filled with noise;
- the search box writes 250ms after typing stops;
- invalid values are ignored, not corrected.

**The URL is read exactly once, at load.** After that the flow is one-way,
state -> URL. In a framework that re-renders on URL change, deriving state
from the URL (or keying a component by it) means the 250ms debounce
overwrites whatever was typed during it.

---

## 7. Pitfalls already paid for

1. **Scroll lock and a sticky bar fight each other.** Locking the page by
   freezing `body` at `position: fixed; top: -scrollY` moves the sticky bar's
   containing block, so the bar rides off screen. The lock publishes
   `--lock-y: <scrollY>px` and the bar uses `top: var(--lock-y, 0px)`.
2. **Smooth scrolling must observe `body`, not `html`.** The root has a fixed
   height, so a resize observer on it never fires and the scroll length stays
   wrong after content grows.
3. **The cursor image must stay below the bar** -- clamp its top to the bar's
   bottom edge, or it slides under an opaque element and disappears.
4. **Convert prototype px to rem.** The live root font scales; leaving px
   behind produces a page that only looks right at 1440px.
5. **The sliding pill is positioned from rects**, relative to the list
   wrapper, and the inner copy gets the exact negative of the pill's offset.
6. **Skip the first `ResizeObserver` callback.** It fires immediately on
   observe and, since resize repositioning is instant-and-untransitioned, it
   kills the first real slide.
7. **Target the view buttons specifically** (`button[data-view]`), not every
   button inside the control box -- the density steppers live there too and
   were being caught by the broader selector.
8. **The chip row scrolls itself** (`row.scrollTo`), never `scrollIntoView`,
   which scrolls the whole page as well.
9. **The lede probe must match width and font**, or the reserved height is
   wrong and the layout still jumps.
10. **Exit, then measure, then swap, then FLIP.** The framework removes an
    element the instant it leaves the data, so the rendered list has to lag
    the computed one by the exit duration: play exits -> capture rects ->
    write the new list -> in a layout effect, FLIP the survivors and run the
    entrance for the newcomers.
11. **Reduced motion has to be checked in JS too.** Global CSS only shortens
    CSS transitions and animations; script-driven animations ignore it
    entirely. Every animated component takes an `animate` flag derived from
    `matchMedia('(prefers-reduced-motion: reduce)')`.
12. **Hidden-before-entrance must depend on script being alive.** The rule
    that hides not-yet-revealed items is scoped to an attribute the intro
    script sets (`html[data-intro] …`), so a visitor without scripts sees
    everything rather than an empty page.
13. **Returning from a detail page.** If the item's image animates back into
    its card, that one card must be marked as already-entered before first
    paint, or the image lands on a hidden card, vanishes, and the card then
    reveals itself a second time. The source project stores the slug in
    `sessionStorage` with a 10-minute expiry.

---

## 8. Reduced motion

No masks, no FLIP, no rises, no cursor image, no typewriter. Pills and
controls jump to position, the count and the lede swap instantly, the
placeholder becomes a plain string, and everything is visible immediately.

## 9. Accessibility notes worth copying

- The result count is announced through a visually hidden `aria-live="polite"`
  paragraph -- "N items" -- because the visible count is decorative motion.
- Toggles are real `<button>`s with `aria-pressed`, not styled checkboxes.
- The decorative duplicate list inside the pill is `inert` + `aria-hidden`.
- Focus is always visible: `1px solid var(--accent)` with an offset; inside
  clipped rows the offset is negative so the ring is not cut off.
- `--muted` is documented as large-text-only, next to its own definition.

---

## 10. What is worth bringing into Tzz App

The parts that transfer, in order of value:

1. **The token discipline.** One file, one accent, three text greys, a rule
   written next to the token about where it may not be used. Tzz App already
   has tokens but no such rule, and no single accent -- it currently uses a
   blue/violet gradient in several intensities.
2. **Two families, hard split.** A serif for headings and titles, a sans for
   UI, a mono for figures. Tzz App is one system sans throughout, which is
   exactly why it reads as a generic utility.
3. **One motion idea, repeated.** The wipe from below would suit a queue of
   jobs: a finished item's card could open the same way, once, instead of
   every element having its own transition.
4. **A count that animates in the heading instead of a status line.**
   "Library (23)" removes a whole row of chrome.
5. **The reserved-height rule.** Nothing below a changing element may move.
   Tzz App has exactly this problem in the action bar, where the status text
   grows a second line while a job runs.
6. **The sliding pill for a set of exclusive choices.** Directly applicable to
   the sidebar navigation and to the Markdown/PDF-style segmented controls.
7. **Search over the library**, with the scoring rules as written -- the
   one-letter and two-letter rules especially.
8. **Density control with a self-explaining preview**, for a Library screen
   that will eventually hold hundreds of folders.
9. **The permanent number.** If Tzz App numbers jobs or library items, the
   number should identify the item, not its position in a filtered view.

What does **not** transfer: the curtain and pinned-bar mechanics (there is no
page scroll of that kind in a desktop app window), the URL as state (no URL),
the cursor image (a lecture folder has no cover worth previewing that way),
and the intro/loader sequence.

One caution: that design is built for a warm near-black page with no window
chrome behind it. Tzz App's window is transparent with macOS vibrancy showing
the desktop through it. Those two ideas fight -- a frosted panel cannot also
be `#181616`. Applying this direction means choosing one: keep the glass and
adapt the palette to tints, or drop the glass and paint the warm near-black.
