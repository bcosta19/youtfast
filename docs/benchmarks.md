# Benchmarks

How much does a native client save compared with a "YouTube desktop app" that
wraps the website in Electron? These numbers come from one machine and one
scenario; treat them as an indication, rerun them on yours and send results.

## Setup (2026-10-17)

- **Machine:** AMD Ryzen 7 7735HS (16 threads), Radeon 680M, 6 GiB RAM visible
  to the system, Arch Linux, Hyprland 0.56.
- **YouTFast:** the 0.1.0 Linux x86_64 build produced by the release workflow,
  system mpv 0.41 and yt-dlp.
- **Electron baseline:** Electron 44.8 from Arch running
  [a minimal app](../tools/benchmark/electron-app/main.js) whose only window
  loads youtube.com, which is what website-wrapper apps do.
- Both signed out, each run with a new empty profile, full screen at 1920×1080
  on a headless output, so nothing else drew on top of them.
- **Scenario:** search results for "blender open movie". After the window
  appeared, wait 10 s, then sample CPU for 20 s and read memory.
- **Metrics:** proportional set size (PSS) summed over the whole process tree,
  which counts shared libraries fairly between processes; CPU time of the tree
  per wall-clock second, where 100% is one core; time until the first window
  maps.

## Results

Median of three alternating runs:

| Search results page | YouTFast 0.1.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Memory (PSS, all processes) | **161 MiB** | 658 MiB |
| CPU while idle on the page | **0%** | 18% |
| Processes | **1** | 8 |
| First window on screen | 1.1 s | **0.4 s** |
| On disk | **31 MB** binary (13 MB download) | 296 MB Electron runtime |

All runs:

| Run | YouTFast PSS | Electron PSS | YouTFast CPU | Electron CPU | YouTFast window | Electron window |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 160.9 MiB | 658.4 MiB | 0.0% | 19.0% | 0.84 s | 0.43 s |
| 2 | 161.0 MiB | 668.3 MiB | 0.0% | 18.4% | 1.14 s | 0.46 s |
| 3 | 159.5 MiB | 607.7 MiB | 0.0% | 7.7% | 1.60 s | 0.42 s |

## Reading the numbers

- Electron's idle CPU comes from the page itself: animated previews and the ad
  that autoplays at the top of the results. That is what users of wrapper apps
  get, but it varies with what YouTube serves, hence the spread between runs.
- Electron maps an empty window sooner. The time until results are usable was
  not measured for either client, so the "first window" row says nothing about
  which one shows content first.
- YouTFast's disk figure excludes the system libmpv/FFmpeg and yt-dlp it uses;
  the Flatpak bundles them. Most Electron apps ship their own copy of the
  runtime instead of sharing the system one.
- **Playback is not compared yet.** During this session YouTube answered this
  machine's signed-out requests with HTTP 429 (too many requests), so yt-dlp
  could not extract streams, while the website kept working. A playback
  comparison measured under that block would not be fair. Contributions with
  playback measurements are welcome.

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

Same machine as above, 2026-10-17. Before is 0.1.0, after is the change that
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
