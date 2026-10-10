# YouTFast design

YouTFast looks like its siblings, Spotifast and ZapFast: a native app in the
desktop's interface face, with rounded cards, pill buttons and chips, round
avatars and soft fills instead of borders. The layout and what each page
offers follow youtube.com (home's grid of cards under chips, search's large
rows, the watch page's title, channel line, actions and description box,
the subscribed channels in the sidebar). The palette is YouTube's dark or
light, a shared preset, or the desktop's (Omarchy), as `src/theme.rs`
resolves it.

Mode: Operate. Scanning a feed, opening a video, and keyboard control come
before decoration. Every keyboard binding stays; the `?` overlay lists them.

## Type

- Inter at four weights through fastframe-fonts (the platform's interface
  face where it has one), installed faces behind it for other scripts, and
  the desktop's text rendering (`theme::install_fonts`).
- `theme::{regular, medium, semibold, bold}(size)` give the weights.
- Sizes: 12–13 secondary and metadata, 14 body, lists and buttons, 18 bold
  section headings, 20 bold a video's title on the watch page, 24 bold a
  page's own title (a channel's name). Emphasis comes from weight first.
- Numbers keep the body face; durations sit in the thumbnail's badge.

## Colour

`theme::Palette` roles, as the siblings use them:

| What | Colour |
| --- | --- |
| Window, page | `window` |
| Sidebar, top bar | `panel` (the same as `window` in YouTube's palettes) |
| Cards, inputs, chips, soft buttons | `surface`; hover `surface_hover`; pressed or the keyboard's selected row `surface_active` |
| Menus, toasts, modals | `overlay` with a 1 px `outline` and the popup shadow |
| Body text | `text`; channel names, counts, ages and other metadata `secondary`; hints and placeholders `dim` |
| Primary button (Subscribe), the chip that is on | `text` fill, `window` label (white on dark, as YouTube) |
| Progress (seek bar played, watched bar), focus, badges' counts | `accent` |
| LIVE, errors | `red` |
| Links in descriptions and comments | `blue`, underlined on hover |

The video rectangle is always black.

## Shape

- Corners: `theme::RADIUS` (8) on cards, rows, thumbnails, menus; widgets
  6; badges `RADIUS_SMALL` (4); buttons, chips and the search field are
  pills (radius half their height); avatars are round.
- No borders on containers: sections sit on the page under a heading,
  cards are fills. 1 px `outline` only around menus, toasts and modals.
- Spacing: 8 px between items, 12 px inside cards, 24 px page padding,
  16–24 px between sections.

## The shell

- **Top bar** (56 px, `panel`): the back arrow when there is a page to go
  back to, the search field centred (`Search, or paste a link`, the search
  icon at its left, a clear button at its right; Enter searches or opens a
  link; `/` focuses it, Esc leaves it), and at the right a keyboard icon
  for the `?` overlay.
- **Sidebar** (`panel`, 240 px): the `YouTFast` wordmark and the collapse
  toggle, then nav items (icon + label, 40 px, rounded `surface` fill when
  current, `surface_hover` on hover): Home, Subscriptions, History, Watch
  later, Liked and Playlists (signed in), Notifications (with its count as
  an `accent` pill), then the account and Settings. Under them, a
  `Subscriptions` section lists the channels followed (round avatar, name).
  Number keys still jump to the views (1–9, in that order).
  Collapsed (`b`, or the toggle) it is a 72 px rail of the icons and the
  channels' avatars, each named on hover.
- **Notices** are toasts at the bottom right (`overlay`, outline, radius 8,
  an icon, the text, a close button); errors stay until closed.
- **Keys overlay**: `?` toggles a modal card listing every binding in two
  columns (the key in a `surface` pill, the action in `text`). Esc closes.

## Lists

- Home is YouTube's grid of cards: as many columns as fit at 280 px or
  more (up to five), each card the rounded 16:9 thumbnail across its width,
  the title (semibold, two lines), the channel and `views · age` under it.
  Day headings split the grid when the feed has dates. Chips above it:
  `All`, `Today`, `Unwatched`, `Live` (as the feed allows).
- Every other list of videos is rows: a rounded 168×94 thumbnail (search
  results larger, up to 360 wide and never more than half the row), then
  the title, the channel and `views · age`. The duration sits in the
  thumbnail's corner on a dark badge (LIVE on `red`); a watched video shows
  YouTube's red bar along the thumbnail's bottom edge.
- Keyboard: `j`/`k` or ↓/↑ move the selected row or card (a
  `surface_active` fill; it scrolls into view), Enter opens it, `g`/`G` go
  to the top or bottom. Hover is `surface_hover`; a click opens.
- Lists of channels are rows with a round avatar, the name, and the
  controls at the end.
- Loading: egui's spinner and `Loading <what>…` in `secondary`. Empty: a
  centred title in `text` and what would fill it in `secondary`. Failed:
  the error in `red` and a `Retry` button.

## Controls

- Buttons are pills (`widgets::button`, `widgets::icon_text_button`):
  Normal is a `surface` pill, On the same with a bold label, Primary the
  `text` fill. Icon-only buttons are round on hover (`icon_button`).
- The seek bar is thin and rounded: played in `accent`, buffered in
  `surface_active`, the rest in `surface`; a round handle while hovered or
  dragged. Times `0:00 / 12:04` in `secondary`.
- Text fields are filled pills or rounded boxes; focus brightens the fill.
- Menus (quality, share, save) are `overlay` popups with rounded items.

## The watch page

- The video takes the width (theater mode: the whole window's width), the
  controls under it.
- Under it, as on YouTube: the title in 20 bold, then the channel's line —
  its round avatar, the name in semibold over `11M subscribers` in
  `secondary`, `Subscribe` as the primary pill — and at the right the
  actions as pills: like (with the count) and dislike joined in one pill,
  Share and Save. When they do not fit, they go to the line under it.
- The **description** is a card: `1.3M views · Jun 8, 2023` in semibold,
  then the text with links in `blue`, collapsed to three lines with
  `more`/`Show less`.
- **Chapters** when the video has them: rows with the time and the name,
  the current one selected; a click seeks.
- **Comments**: the count as the section's heading; each comment its round
  avatar, `@author` in semibold and the age in `secondary`, the text,
  then the likes, the creator's heart and the replies' toggle; replies
  indented under it with smaller avatars.
- **Up next** and **Related** to the right in a wide window (under in a
  narrow one), as rows.

## States and motion

- Hover, focus, selected, disabled (`dim`), loading (spinner), empty,
  error, on every list and control.
- Motion: the theme change's reveal, egui's short hover fades
  (`animation_time` 0.12), the spinner, toasts appearing. Nothing else.
