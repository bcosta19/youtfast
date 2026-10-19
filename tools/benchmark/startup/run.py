#!/usr/bin/env python3
"""Times YouTFast's startup with the LD_PRELOAD probe. See docs/benchmarks.md.

usage: run.py <out-dir> <label> <binary> <search|home> <cold|warm> <runs> [evict] [start=N]

Each launch runs full screen on the headless Hyprland output YTBENCH, through
`hl.exec_cmd` rules that follow only that process, with a profile and runtime
directory of its own: the real library, settings, cache and a running copy
are never touched. `search` opens results for "blender open movie"; `home`
seeds a library of public channels and Blender open movies, so Home's For you
fills signed out. `cold` starts every run from an empty profile; `warm` keeps
the profile of a first, unrecorded run. `evict` drops the binary, its
libraries, Mesa's drivers and the installed fonts from the page cache first
(evict.py), as after a while without opening the app.

Writes run-NN/{probe.log,metrics.json,settled.png} and results.json, in
milliseconds from the exec. Requires probe.so built next to this script.
"""
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROBE = HERE / "probe.so"
OUTPUT = "YTBENCH"
CHANNELS = [
    ("UCSMOQeBJ2RAnuFungnQOxLg", "Blender"), ("UCLA_DiR1FfKNvjuUpBHmylQ", "NASA"),
    ("UCHnyfMqiRRG1u-2MsSQLbXA", "Veritasium"), ("UCsXVk37bltHxD1rDPwtNM8Q", "Kurzgesagt"),
    ("UCXuqSBlHAE6Xw-yeJA0Tunw", "Linus Tech Tips"), ("UCYO_jab_esuFRV4b17AJtAw", "3Blue1Brown"),
    ("UC9-y-6csu5WGm29I7JiwpnA", "Computerphile"), ("UCsBjURrPoezykLs9EqgamOA", "Fireship"),
    ("UCY1kMZp36IQSyNx_9h4mpCg", "Mark Rober"), ("UCR-DXc1voovS8nhAvccRZhg", "Jeff Geerling"),
    ("UCoxcjq-8xIDTYp3uz647V5A", "Numberphile"), ("UCBa659QWEk1AI4Tg--mrJ2A", "Tom Scott"),
]
HISTORY = [
    ("YE7VzlLtp-4", "Big Buck Bunny"), ("eRsGyueVLvQ", "Sintel"), ("R6MlUcmOul8", "Tears of Steel"),
    ("WhWc3b3KhnY", "Spring"), ("UXqq0ZvbOnk", "CHARGE"), ("_cMxraX_5RE", "Sprite Fright"),
    ("Y-rmzh0PI3c", "Cosmos Laundromat"), ("mN0zPOpADL4", "Agent 327"),
]
# A run ends once no texture arrived for this long after the first thumbnail.
QUIET_S = 4.0
MAX_S = 45.0


def hypr(*args):
    return subprocess.run(["hyprctl", *args], check=True, capture_output=True, text=True).stdout


def workspace():
    for monitor in json.loads(hypr("monitors", "all", "-j")):
        if monitor["name"] == OUTPUT:
            return monitor["activeWorkspace"]["id"]
    raise SystemExit(f"no {OUTPUT} output; see docs/benchmarks.md")


def seed(profile, scenario):
    for sub in ("config", "data", "cache", "state"):
        (profile / sub).mkdir(parents=True, exist_ok=True)
    if scenario == "home":
        library = profile / "data" / "youtfast" / "library.json"
        library.parent.mkdir(parents=True, exist_ok=True)
        now = int(time.time())
        history = [
            {"video": {"id": video, "title": title, "channel": "Blender", "channel_id": CHANNELS[0][0]},
             "at": now - index * 3600, "position": 0.0}
            for index, (video, title) in enumerate(HISTORY)
        ]
        subscriptions = [{"id": channel, "name": name} for channel, name in CHANNELS]
        library.write_text(json.dumps({"subscriptions": subscriptions, "history": history}))


def events(path, t0):
    found = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return found
    for line in lines:
        parts = line.split(" ", 3)
        if len(parts) >= 3:
            found.append(((float(parts[0]) - t0) * 1000, int(parts[1]), parts[2], parts[3] if len(parts) > 3 else ""))
    return found


def launch(binary, args, profile, run_dir, ws):
    runtime = run_dir / "runtime"
    runtime.mkdir(mode=0o700, parents=True)
    wayland = Path(os.environ.get("WAYLAND_DISPLAY", "wayland-0"))
    env = {
        "XDG_CONFIG_HOME": profile / "config", "XDG_DATA_HOME": profile / "data",
        "XDG_CACHE_HOME": profile / "cache", "XDG_STATE_HOME": profile / "state",
        # The instance lock lives in the runtime directory, so the measured
        # copy never hands over to one the user has open.
        "XDG_RUNTIME_DIR": runtime,
        "WAYLAND_DISPLAY": Path(os.environ["XDG_RUNTIME_DIR"]) / wayland,
        "DBUS_SESSION_BUS_ADDRESS": os.environ["DBUS_SESSION_BUS_ADDRESS"],
        "LD_PRELOAD": PROBE, "YTPROBE_LOG": run_dir / "probe.log",
    }
    command = "env " + " ".join(shlex.quote(f"{key}={value}") for key, value in env.items())
    command += " python3 -I " + " ".join(shlex.quote(str(a)) for a in [HERE / "launch.py", run_dir / "stamp", binary, *args])
    hypr("eval", f'hl.exec_cmd([==[{command}]==], {{ workspace = "{ws} silent", fullscreen = true }})')
    stamp = run_dir / "stamp"
    for _ in range(500):
        if stamp.exists() and stamp.read_text().strip():
            pid, t0 = stamp.read_text().split()
            return int(pid), float(t0)
        time.sleep(0.01)
    raise SystemExit("the launch never started")


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def wait_until_settled(log, t0, pid):
    while True:
        time.sleep(0.25)
        elapsed = time.monotonic() - t0
        found = events(log, t0)
        thumbs = [e for e in found if e[2] == "tex" and e[3].startswith("320x180")]
        last_texture = max((e[0] for e in found if e[2] in ("tex", "texsub")), default=0) / 1000
        if not alive(pid):
            return "exited"
        if thumbs and elapsed - last_texture > QUIET_S:
            return "settled"
        if elapsed > MAX_S:
            return "timeout"


def stop(pid):
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    for _ in range(100):
        if not alive(pid):
            return
        time.sleep(0.05)
    os.kill(pid, signal.SIGKILL)


def analyse(found, pid):
    app = [e for e in found if e[0] >= 0]
    first = lambda pred: next((e[0] for e in app if pred(e)), None)
    last = lambda pred: next((e[0] for e in reversed(app) if pred(e)), None)
    m = {}
    # The probe's constructor runs once the dynamic loader is done.
    m["loader"] = first(lambda e: e[2] == "ctor" and f"pid={pid}" in e[3])
    m["main"] = first(lambda e: e[2] == "open" and e[3].endswith("instance.lock"))
    m["gl_ready"] = first(lambda e: e[2] == "dlopen" and "libwayland-egl" in e[3])
    font = lambda e: e[2] == "open" and re.search(r"/usr/share/fonts/.*\.(ttf|otf|ttc|pfb|pcf|gz|woff2?)$", e[3], re.I)
    m["fonts_n"] = sum(1 for e in app if font(e))
    m["fonts_start"], m["fonts_end"] = first(font), last(font)
    mpv = [e for e in app if e[2] == "mpv_initialize"]
    m["mpv_init_ms"] = float(mpv[0][3].split("dt=")[1]) * 1000 if mpv else None
    swaps = [e for e in app if e[2] == "swap"]
    m["first_frame"] = swaps[0][0] if swaps else None
    m["frames"] = len(swaps)
    textures = [(e[0], *map(int, e[3].split()[0].split("x"))) for e in app if e[2] == "tex"]
    thumbs = [t[0] for t in textures if (t[1], t[2]) == (320, 180)]
    m["thumbs_n"], m["thumbs_first"], m["thumbs_last"] = len(thumbs), min(thumbs, default=None), max(thumbs, default=None)
    thumb_io = lambda e: ("/thumbnails/" in e[3] and e[2] in ("open", "rename")) or (
        e[2] == "dns" and e[3].endswith(("i.ytimg.com", "yt3.ggpht.com", "yt3.googleusercontent.com")))
    # The list is drawn a pass before its thumbnails are asked for.
    m["thumbs_requested"] = first(thumb_io)
    m["thumb_cache_reads"] = sum(1 for e in app if e[2] == "open" and "fd=-1" not in e[3]
                                 and "flags=0x80000 " in e[3] and re.search(r"/thumbnails/[0-9a-f]{1,16}$", e[3]))
    m["thumb_downloads"] = sum(1 for e in app if e[2] == "rename" and "/thumbnails/" in e[3])
    m["connections"] = sum(1 for e in app if e[2] == "connect" and e[3].endswith(" 443"))
    m["dns_max_ms"] = max((float(e[3].split("dt=")[1].split()[0]) * 1000 for e in app if e[2] == "dns"), default=0)
    m["threads"] = sum(1 for e in app if e[2] == "thread")
    return m


def main():
    out, label, binary, scenario, cache, runs = sys.argv[1:7]
    evict = "evict" in sys.argv[7:]
    start = next((int(a.split("=")[1]) for a in sys.argv[7:] if a.startswith("start=")), 0)
    out = Path(out) / f"{label}-{scenario}-{cache}{'-evicted' if evict else ''}"
    out.mkdir(parents=True, exist_ok=True)
    binary = str(Path(binary).resolve())
    args = ["blender open movie"] if scenario == "search" else []
    ws = workspace()
    profile = out / "profile"
    if cache == "warm" and not profile.exists():
        seed(profile, scenario)
        prime = out / "prime"
        shutil.rmtree(prime, ignore_errors=True)
        prime.mkdir()
        pid, t0 = launch(binary, args, profile, prime, ws)
        print("priming:", wait_until_settled(prime / "probe.log", t0, pid), flush=True)
        stop(pid)
        time.sleep(1.5)
    for index in range(start, start + int(runs)):
        run_dir = out / f"run-{index:02d}"
        shutil.rmtree(run_dir, ignore_errors=True)
        run_dir.mkdir()
        if cache == "cold":
            shutil.rmtree(profile, ignore_errors=True)
            seed(profile, scenario)
        if evict:
            subprocess.run([sys.executable, "-I", str(HERE / "evict.py"), binary], check=True)
            time.sleep(0.5)
        pid, t0 = launch(binary, args, profile, run_dir, ws)
        outcome = wait_until_settled(run_dir / "probe.log", t0, pid)
        subprocess.run(["grim", "-o", OUTPUT, str(run_dir / "settled.png")], check=False)
        stop(pid)
        metrics = analyse(events(run_dir / "probe.log", t0), pid)
        metrics["outcome"] = outcome
        (run_dir / "metrics.json").write_text(json.dumps(metrics, indent=1))
        shown = ("loader", "first_frame", "thumbs_requested", "thumbs_n", "thumbs_last", "connections", "outcome")
        print(f"{label} run {index}:", {k: round(v) if isinstance(v, float) else v for k, v in metrics.items() if k in shown}, flush=True)
        time.sleep(1.5)
    runs = [json.loads(path.read_text()) for path in sorted(out.glob("run-*/metrics.json"))]
    (out / "results.json").write_text(json.dumps(runs, indent=1))


main()
