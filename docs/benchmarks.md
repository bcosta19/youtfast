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
