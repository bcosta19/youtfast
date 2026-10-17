"""Measure a desktop YouTube client on Hyprland. See docs/benchmarks.md.

Usage: bench.py <label> <settle-seconds> <sample-seconds> [--shot PNG] -- <command...>

Expects a window rule that places the client full screen on a headless
output named YTSHOT, so measurements and captures never use the real screen.
Reports time until the first window maps, PSS of the whole process tree and
the tree's average CPU use (100 = one core) while sampling. Run each client
with a fresh profile and signed out.
"""
import json
import os
import subprocess
import sys
import time

TICK = os.sysconf("SC_CLK_TCK")


def tree(root):
    children = {}
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                with open(f"/proc/{entry}/stat") as handle:
                    ppid = int(handle.read().rsplit(")", 1)[1].split()[1])
                children.setdefault(ppid, []).append(int(entry))
            except (OSError, ValueError, IndexError):
                pass
    found, stack = [], [root]
    while stack:
        pid = stack.pop()
        found.append(pid)
        stack.extend(children.get(pid, []))
    return found


def pss_kib(pids):
    total = 0
    for pid in pids:
        try:
            with open(f"/proc/{pid}/smaps_rollup") as handle:
                for line in handle:
                    if line.startswith("Pss:"):
                        total += int(line.split()[1])
        except OSError:
            pass
    return total


def cpu_ticks(pids):
    total = 0
    for pid in pids:
        try:
            with open(f"/proc/{pid}/stat") as handle:
                fields = handle.read().rsplit(")", 1)[1].split()
            total += int(fields[11]) + int(fields[12])
        except (OSError, ValueError, IndexError):
            pass
    return total


def window_for(pids):
    for client in json.loads(subprocess.check_output(["hyprctl", "clients", "-j"])):
        if client["pid"] in pids and client["mapped"]:
            return client
    return None


def main():
    label, settle, sample = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
    rest = sys.argv[4:]
    shot = None
    if rest[0] == "--shot":
        shot, rest = rest[1], rest[2:]
    command = rest[rest.index("--") + 1:]
    started = time.monotonic()
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               start_new_session=True)
    window_s = None
    try:
        while time.monotonic() - started < 30:
            client = window_for(set(tree(process.pid)))
            if client:
                window_s = time.monotonic() - started
                break
            time.sleep(0.02)
        if client is None:
            raise SystemExit(f"{label}: no window")
        time.sleep(settle)
        pids = tree(process.pid)
        before, at = cpu_ticks(pids), time.monotonic()
        time.sleep(sample)
        pids = tree(process.pid)
        cpu = (cpu_ticks(pids) - before) / TICK / (time.monotonic() - at) * 100
        pss = pss_kib(pids)
        client = window_for(set(pids))
        if shot:
            subprocess.run(["grim", "-o", "YTSHOT", shot], check=True)
        print(json.dumps({"label": label, "window_s": round(window_s, 3),
                          "pss_mib": round(pss / 1024, 1), "cpu_pct": round(cpu, 1),
                          "processes": len(pids), "size": client and client["size"],
                          "fullscreen": client and client["fullscreen"],
                          "monitor": client and client["monitor"]}))
    finally:
        # Only the main process: signalling every Chromium child at once makes
        # them trap and dump core.
        process.terminate()
        try:
            process.wait(15)
            time.sleep(2)
        except subprocess.TimeoutExpired:
            pass
        try:
            os.killpg(process.pid, 9)
        except ProcessLookupError:
            pass


main()
