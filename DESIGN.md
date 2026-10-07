# YouTFast design

YouTFast looks like a terminal program, the way lazygit, btop or yazi do,
drawn in the desktop's own terminal font and Omarchy palette. It is not a
copy of youtube.com: there are panes instead of cards, a command line
instead of a search box, a status line instead of toasts, and a feed that
reads like a table. Video, thumbnails and avatars are real pictures; all the
chrome around them is text, rules and colour.

Mode: Operate. Scanning a feed, opening a video, and keyboard control come
before decoration.

## Type

- One face for everything: the desktop's monospace font (on Linux the file
  `fc-match -f '%{file}' monospace` names, run once at start), registered as
  both egui's Proportional and Monospace families. When that fails, egui's
  bundled Hack. Your terminal's font and the app's are the same font.
- Three sizes only: 13 (secondary and metadata), 14 (body, lists, buttons),
  16 bold (a pane's main title, a video's title on the watch page). There is
  no larger text anywhere.
- Emphasis comes from weight and colour, never from size.
- Numbers (durations, counts, times) are already tabular in a mono face; keep
  columns aligned by giving every column a fixed width in character cells.

## Colour

The palette is `theme::Palette`: the 16 base colours plus five ANSI roles a
terminal theme always has, `red`, `green`, `yellow`, `blue`, `magenta`,
`cyan` (`red` and `yellow` default to `danger` and `warning`). The Omarchy
template fills them from the theme's own `red`, `green`, ... so the app
matches the terminal next to it. Restrained strategy: neutrals carry the
surface, `accent` marks focus and selection, the ANSI roles colour data the
way `ls --color` does:

| What | Colour |
| --- | --- |
| Pane border | `outline`; the pane with keyboard focus: `accent` |
| Selected row | `accent` background, `window` text (inverted, like a TUI cursor) |
| Hovered row | `surface_hover` background |
| Channel names | `blue` |
| Durations | `yellow` |
| View counts, ages, other metadata | `secondary` |
| LIVE, errors | `red` |
| Watched / resume progress, success | `green` |
| Key hints (`[space]`, `[/]`) | `magenta` for the key, `secondary` for its label |
| Links in descriptions and comments | `cyan`, underlined on hover |

Dark or light comes from the palette (Omarchy says which). The video
rectangle is always black.

## Shape

- Square everything: corner radius 0 on panes, buttons, inputs, thumbnails,
  menus, the seek bar. Thumbnails and avatars too (avatars are square).
- 1 px rules. No shadows, no gradients, no blur, no filled cards.
- A pane is a 1 px rectangle whose title sits in its top rule, the way
  lazygit draws it: `─ Up next ──────────`, the title in `text` bold for the
  focused pane and `secondary` otherwise, with an optional right-aligned
  count (`12`) in the same rule. Panes are the only containers; nothing is
  nested inside a second box.
- Spacing on the character grid: 1 cell (≈ the width of one character) of
  padding inside a pane, 1 line between groups, 2 lines above a group
  heading.

## The shell

```
┌ youtfast ──────────┐ ❯ search, or paste a link_                     [?] keys
│ [1] Home           │┌─ Home ── 48 ──────────────────────────────────────────┐
│ [2] Subscriptions  ││ ▌thumb▐ Title of the video                    12:04    │
│ [3] History        ││ ▌     ▐ Channel name · 1.2M views · 2 days ago        │
│ [4] Watch later    ││ ...                                                    │
│ [5] Liked          │└────────────────────────────────────────────────────────┘
│ [6] Playlists      │
│ [8] Account        │
│ [9] Settings       │
└────────────────────┘
 HOME  ⠋ loading feed                    personal · 1080p · Everforest   [?]
```

- **Command line** (top): the prompt `❯` in `accent`, then the search
  field with no box (just the text and a block caret), hint text in `dim`.
  Enter searches or opens a link. `/` from anywhere focuses it; Esc leaves
  it.
- **Sidebar**: a pane titled `youtfast`. Each item is `[n] Label`, the key
  in `magenta`; the current view is the selected row (inverted). Number keys
  jump to views when no text field has focus.
- **Status line** (bottom, one line, `panel` fill, no border): the current
  view's name as an inverted block in `accent` (` HOME `), then the
  backend's activity as a braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) with what is
  loading, or the last notice (with `[x]` to dismiss), then right-aligned:
  account name, quality, theme name, `[?]`. Notices live here, not in a
  panel.
- **Keys overlay**: `?` toggles a pane centred over the window listing every
  key binding in two columns (key in `magenta`, action in `text`). Esc
  closes it. This is the only overlay; there are no modals elsewhere.

## Lists

- Every list of videos (home, results, history, watch later, liked,
  playlist, channel tabs) is a list of rows, not a grid: a 16:9 thumbnail
  (120×68 at body size; the duration in its corner in `yellow` on black),
  then the title in `text` (one line, cut with an ellipsis), and under it
  `channel · views · age` with the colours above. A watched video shows a
  2 px `green` bar under its thumbnail, as long as the part watched.
- Keyboard: `j`/`k` or ↓/↑ move the selected row (it scrolls into view),
  Enter opens it, `g`/`G` go to the top or bottom. The pointer works as
  before: hover highlights, click opens.
- Lists of channels are rows with a square avatar, the name in `blue`, and
  the controls at the end.
- Loading: the spinner and `loading <what>` inside the pane, in
  `secondary`. Empty: a centred line in `dim`, `-- nothing here --`, and
  under it what would fill it (`subscribe to channels to fill your feed`).
  Failed: the error in `red` and `[r] retry`.

## Controls

- Buttons are bracketed text: `[ subscribe ]`, `[ watch later ]`. Inactive:
  `secondary` text. Hover: `surface_hover` background, `text` text.
  Active/on: `accent` text and bold (`[ subscribed ]`). Primary actions use
  the inverted style (`accent` background, `window` text).
- Player controls keep the Lucide icons (drawn in `text`), square, no frame.
- The seek bar is a 1-line strip: played in `accent`, buffered in
  `surface_active`, the rest in `surface`; a 1-cell-wide block marks the
  position while hovered or dragged. Times are `00:00 / 12:04`.
- Text fields: no box, a 1 px `outline` underline that turns `accent` with
  focus, block caret.
- Selectable rows (settings, menus) use the selected-row style.

## The watch page

- The video pane (titled with the channel name) takes the width, the
  controls strip under it.
- Under it, a **details** pane titled with the video title in bold, laid out
  as aligned key/value lines with dim leader dots, like a `man` page header:

  ```
  channel ······ Channel Name            [ subscribe ]
  views ········ 1,234,567
  likes ········ 34K
  published ···· 2 days ago (2026-10-04)
  ```

- A **description** pane: the text with links in `cyan` and timestamps
  (`12:34`) that seek when clicked; collapsed to 6 lines with `[ more ]`.
- A **chapters** pane when the video has chapters: `[00:00] Intro` rows,
  the current one selected; clicking seeks.
- A **comments** pane: threaded like a mail reader, each comment
  `@author · 3d · ▲ 120 · 4 replies` in `secondary` (author in `blue`), the
  text under it, replies indented 2 cells under a `│` rule; `[ load more ]`
  at the end.
- **Up next** pane to the right in a wide window (under in a narrow one):
  the related videos as rows.

## States and motion

- Hover, focus, selected, disabled (`dim` text), loading (spinner), empty,
  error, as above, on every list and control.
- Motion: the theme change's reveal is the one authored moment, plus the
  caret blinking in the command line. Nothing else animates.
