# Benchmarks

How much does a native client save compared with a "YouTube desktop app" that
wraps the website in Electron? These numbers come from one machine and one
scenario; treat them as an indication, rerun them on yours and send results.

## Setup (2026-11-12 — 0.7.0)

- **Machine:** AMD Ryzen 7 7735HS (16 threads), Radeon 680M, 6 GiB RAM visible
  to the system, Arch Linux, Hyprland 0.56.2.
- **YouTFast:** the `0.7.0` Linux x86_64 build produced by the release workflow
  (`28,691,240` bytes; `.tar.gz` `13,086,988` bytes), system `mpv 0.41.0` and
  `yt-dlp 2026.08.19`.
- **Electron baseline:** Electron `44.10.2` (`electron44-bin`, `301.24 MiB`
  installed size) running [a minimal app](../tools/benchmark/electron-app/main.js)
  whose only window loads `youtube.com`, which is what website-wrapper apps do.
- Both signed out, each run with a new empty profile, full screen at 1920×1080
  on a headless output, so nothing else drew on top of them.
- **Search scenario:** search results for `"blender open movie"`. After the
  window appeared, wait 10 s, then sample CPU for 20 s and read memory.
- **Playback scenario:** `https://www.youtube.com/watch?v=WhWc3b3KhnY` (*Spring*,
  Blender Studio, 1080p). After the window appeared, wait 20 s so playback is
  underway, then sample CPU for 20 s and read memory.
- **Metrics:** proportional set size (PSS) summed over the whole process tree,
  which counts shared libraries fairly between processes; CPU time of the tree
  per wall-clock second, where 100% is one core; time until the first window
  maps.

## Results (0.7.0)

### Search results page

Median of three alternating runs:

| Search results page | YouTFast 0.7.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Memory (PSS, all processes) | **136 MiB** | 662 MiB |
| CPU while idle on the page | **0.0%** | 5.6% |
| Processes | **1** | 8 |
| First window on screen | **0.5 s** | 0.7 s |
| On disk | **29 MB** binary (13 MB download) | 316 MB (301 MiB) Electron runtime |

All runs:

| Run | YouTFast PSS | Electron PSS | YouTFast CPU | Electron CPU | YouTFast window | Electron window |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 132.1 MiB | 662.0 MiB | 0.0% | 5.6% | 1.027 s | 1.547 s |
| 2 | 136.3 MiB | 687.5 MiB | 0.0% | 28.4% | 0.505 s | 0.701 s |
| 3 | 141.7 MiB | 644.9 MiB | 0.0% | 4.9% | 0.339 s | 0.698 s |

### Video playback (1080p, *Spring*)

Median of three alternating runs:

| Playback (1080p, *Spring*) | YouTFast 0.7.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Memory (PSS, all processes) | **206 MiB** | 625 MiB |
| CPU during playback | **13.9%** | 41.2% |
| Processes | **1** | 8 |
| First window on screen | **0.5 s** | 0.8 s |

All runs:

| Run | YouTFast PSS | Electron PSS | YouTFast CPU | Electron CPU | YouTFast window | Electron window |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 206.6 MiB | 621.7 MiB | 13.9% | 40.3% | 1.005 s | 0.808 s |
| 2 | 206.1 MiB | 625.7 MiB | 14.4% | 41.2% | 0.451 s | 0.756 s |
| 3 | 202.6 MiB | 625.0 MiB | 13.8% | 41.9% | 0.453 s | 0.694 s |

### UI frame cost (`0.7.0`)

Measured with `cargo test --release -p youtfast-ui --lib frame_cost -- --ignored --nocapture --test-threads=1`
(300 frames per view; UI CPU work per frame, excluding compositor presentation):

| View | Mean | p50 | p95 | Allocations / frame |
|---|---:|---:|---:|---:|
| `results` | 0.074 ms | 0.073 ms | 0.089 ms | 388 |
| `home` | 0.274 ms | 0.269 ms | 0.301 ms | 824 |
| `watch` | 0.278 ms | 0.271 ms | 0.369 ms | 558 |
| `notifications` | 0.303 ms | 0.296 ms | 0.338 ms | 697 |
| `history_search` | 0.447 ms | 0.442 ms | 0.480 ms | 1414 |
| `history` | 0.453 ms | 0.450 ms | 0.485 ms | 1440 |

## Previous baseline (2026-10-17 — 0.1.0)

Setup: same laptop, Hyprland 0.56, YouTFast 0.1.0 release binary, Electron 44.8 (`296 MB`).
Median of three alternating runs on the search results page:

| Search results page | YouTFast 0.1.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Memory (PSS, all processes) | **161 MiB** | 658 MiB |
| CPU while idle on the page | **0%** | 18% |
| Processes | **1** | 8 |
| First window on screen | 1.1 s | **0.4 s** |
| On disk | **31 MB** binary (13 MB download) | 296 MB Electron runtime |

All `0.1.0` runs:

| Run | YouTFast PSS | Electron PSS | YouTFast CPU | Electron CPU | YouTFast window | Electron window |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 160.9 MiB | 658.4 MiB | 0.0% | 19.0% | 0.84 s | 0.43 s |
| 2 | 161.0 MiB | 668.3 MiB | 0.0% | 18.4% | 1.14 s | 0.46 s |
| 3 | 159.5 MiB | 607.7 MiB | 0.0% | 7.7% | 1.60 s | 0.42 s |

## Reading the numbers

- Dropping RustyPipe in `0.7.0` (`#91`) for the in-tree InnerTube client
  reduced search-page RSS/PSS (`161 → 136 MiB`) and binary size (`31 → 29 MB`),
  bringing memory usage to roughly **4.9× less** than Electron on search results.
- On 1080p playback (*Spring*), YouTFast uses libmpv inside the same single
  process (`206 MiB`, `13.9%` CPU) instead of eight Chromium/renderer/GPU
  processes (`625 MiB`, `41.2%` CPU): about **3.0× less** memory and CPU.
- Electron's idle CPU on search results comes from the page itself (animated
  previews and top-slot ads), so it varies with what YouTube serves (`4.9%–28.4%`
  in the `0.7.0` session, `7.7%–19.0%` in `0.1.0`). YouTFast stays at `0.0%`
  once idle.
- YouTFast's disk figure excludes the system libmpv/FFmpeg and yt-dlp it uses;
  the Flatpak bundles them. Most Electron apps ship their own copy of the
  runtime instead of sharing the system one.

## Reproduce

The harness is in [tools/benchmark](../tools/benchmark). It relies on Hyprland
(`hyprctl`) and `grim`, and runs the clients on a headless output so the real
screen is untouched:

```sh
hyprctl output create headless YTSHOT
hyprctl eval 'hl.monitor({ output = "YTSHOT", mode = "1920x1080@60", position = "0x5000", scale = 1 })'
hyprctl eval 'hl.window_rule({ match = { class = "^(youtfast|yt-electron-bench|electron)$" }, workspace = "1 silent", fullscreen = true })'
# Move an empty workspace 1 to YTSHOT if Hyprland did not put it there.

profile=$(mktemp -d)
XDG_CONFIG_HOME=$profile/config XDG_DATA_HOME=$profile/data \
XDG_CACHE_HOME=$profile/cache XDG_STATE_HOME=$profile/state \
  python3 tools/benchmark/bench.py youtfast 10 20 -- ./youtfast "blender open movie"

BENCH_URL='https://www.youtube.com/results?search_query=blender+open+movie' \
  python3 tools/benchmark/bench.py electron 10 20 -- \
  electron --ozone-platform=wayland --user-data-dir="$(mktemp -d)" tools/benchmark/electron-app

hyprctl output remove YTSHOT
```

The Lua `hyprctl eval` lines match Hyprland 0.56 with a Lua configuration; older
versions use `hyprctl keyword monitor` and `windowrule` instead.

## Startup

How long YouTFast takes from launch to a filled page, measured in the
unchanged binary: [`tools/benchmark/startup/probe.c`](../tools/benchmark/startup/probe.c)
is an `LD_PRELOAD` library that timestamps frames (`eglSwapBuffers`), texture
uploads (`glTexImage2D`: icons, the font atlas, thumbnails), files opened,
connections and DNS lookups. Times count from the `exec`:

- **First frame:** the window on screen, with its icons and text.
- **Last thumbnail:** the last thumbnail visible on the page, drawn.
- **Content → last thumbnail:** from the list being drawn (its thumbnails are
  asked for) to the last of them on screen.

Each launch runs signed out, full screen at 1920×1080 on the headless output,
with a profile of its own. *Search* opens `youtfast "blender open movie"` (four
thumbnails visible). *Home* starts from a library of 12 public channels and 8
Blender open movies in the history, so its For you fills (20 thumbnails
visible). *Thumbnails on disk* keeps the profile of an earlier run; *empty
cache* starts each run from nothing; *files out of memory* first drops the
binary, its libraries, Mesa's drivers and the installed fonts from the page
cache ([`evict.py`](../tools/benchmark/startup/evict.py)), as after a while
without opening the app on a machine short of memory.

Same machine, `0.7.0` official release binary on 2026-11-12 (4 runs each,
median and min–max in milliseconds; *files out of memory* evicted `1,181` files):

| Scenario (`0.7.0`) | Libraries loaded | First frame | Content drawn | Last thumbnail | Content → last thumb | TLS connections |
|---|---:|---:|---:|---:|---:|---:|
| Search, empty cache | 215 (107–451) | 590 (350–1250) | 1292 (999–1911) | 1471 (1130–2286) | 181 (127–375) | 6 (6–6) |
| Search, thumbnails on disk | 147 (103–157) | 376 (328–389) | 1067 (1004–1087) | **1112** (1095–1122) | 35 (34–111) | **2** (2–3) |
| Home, empty cache | 119 (102–314) | 364 (331–536) | 1325 (1212–1417) | 2250 (2101–2456) | 974 (684–1148) | **24** (23–25) |
| Home, thumbnails on disk | 132 (108–191) | 355 (326–445) | 1275 (1226–1320) | **1760** (1499–1916) | 458 (272–651) | 23 (23–24) |
| Home, files out of memory | 549 (547–590) | 1044 (1041–1120) | 1995 (1922–1999) | **2663** (2509–3020) | 665 (587–1027) | 23 (23–24) |

Compared with `0.1.0` and the 2026-10-17 optimisation below, `0.7.0` reduces
TLS connections on cold Home from 50 → 36 → **24** (and warm Search to **2–3**),
and lowers median last-thumbnail completion on warm Search from 1740 → 1268 → **1112 ms**.
First-frame latency (`355–376 ms` warm on the release binary vs `153–172 ms` in
the local October build) reflects both three weeks of feature growth and host
stack updates across separate test dates.

Earlier comparison on the same machine, 2026-10-17. Before is 0.1.0, after is the change that
looks for the installed fonts on a thread and reads kept thumbnails without a
download turn; both built locally with the release profile and run
alternately. Median (minimum–maximum) in milliseconds, 4 or 5 runs each:

| Scenario | First frame before | after | Last thumbnail before | after | Content → last thumbnail before | after |
|---|---:|---:|---:|---:|---:|---:|
| Search, empty cache | 213 (202–264) | **172** (158–219) | 1766 (1618–2246) | **1622** (1443–1717) | 187 (128–571) | **135** (122–181) |
| Search, thumbnails on disk | 202 (201–302) | **153** (151–192) | 1740 (1297–1947) | **1268** (1215–1636) | 34 (34–132) | 34 (34–34) |
| Home, empty cache | 213 (209–214) | **157** (156–162) | 2515 (2035–3248) | **2005** (1944–2310) | 587 (542–599) | **477** (249–528) |
| Home, thumbnails on disk | 207 (200–244) | **154** (152–157) | 2359 (1614–3178) | **1812** (1810–1976) | 391 (256–818) | **250** (167–510) |
| Home, files out of memory | 2472 (2446–2482) | **973** (969–984) | 4522 (4043–6369) | **2671** (2417–3033) | 447 (184–610) | **333** (221–448) |

Reading the numbers:

- The window used to wait for a probe of every installed font (816 files
  here) for the scripts Inter lacks: some 50 ms with the files in memory, 1.5 s
  from disk. The probe now runs on a thread while the window and its OpenGL
  context are made, and the faces are put in once found; Latin, Greek and
  Cyrillic text never waits for them.
- With the window up sooner, the first requests leave sooner, so the page
  fills sooner too: 1.85 s sooner when the files were out of memory.
- Thumbnails already on disk used to wait for download turns held by the
  page's new thumbnails (four at a time, each opening its own TLS
  connection). They now answer at once, and downloads keep their connections
  open: Home with an empty cache opened 36 TLS connections instead of 50.
- When the list arrives depends on YouTube: two round trips signed out, the
  visitor data and the request itself, some 1.2 s here. Not changed.
- Linking libmpv loads about 200 libraries before `main`: 55 ms in memory,
  0.4 s from disk. Not changed either.

To reproduce, build the probe and run each binary in turn, on Hyprland with
`grim` (the same headless output as above, without the window rule: each
launch carries its own):

```sh
cc -O2 -fPIC -shared -o tools/benchmark/startup/probe.so tools/benchmark/startup/probe.c -ldl
hyprctl output create headless YTBENCH
hyprctl eval 'hl.monitor({ output = "YTBENCH", mode = "1920x1080@60", position = "0x5000", scale = 1 })'
for run in 0 1 2 3 4; do
  for build in before after; do
    python3 tools/benchmark/startup/run.py out $build ./youtfast-$build home warm 1 start=$run
  done
done
python3 tools/benchmark/startup/summarize.py out
hyprctl output remove YTBENCH
```

Add `evict` after the run count for the files-out-of-memory case.
