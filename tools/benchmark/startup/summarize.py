"""Medians (minimum–maximum) of each condition run.py wrote: summarize.py <out-dir>..."""
import json
import statistics
import sys
from pathlib import Path

ROWS = [
    ("loader", "libraries loaded"),
    ("first_frame", "first frame"),
    ("thumbs_requested", "content drawn"),
    ("thumbs_last", "last thumbnail"),
]


def cell(values):
    values = [v for v in values if v is not None]
    if not values:
        return "—"
    return f"{statistics.median(values):.0f} ({min(values):.0f}–{max(values):.0f})"


for root in sys.argv[1:]:
    for results in sorted(Path(root).glob("*/results.json")):
        runs = json.loads(results.read_text())
        print(f"\n{results.parent.name}: {len(runs)} runs, ms from exec")
        for key, label in ROWS:
            print(f"  {label:22s} {cell([run.get(key) for run in runs])}")
        gaps = [run["thumbs_last"] - run["thumbs_requested"] for run in runs
                if run.get("thumbs_last") is not None and run.get("thumbs_requested") is not None]
        print(f"  {'content → last thumb':22s} {cell(gaps)}")
        print(f"  {'thumbnails':22s} {cell([run['thumbs_n'] for run in runs])}, "
              f"from disk {cell([run['thumb_cache_reads'] for run in runs])}, "
              f"downloaded {cell([run['thumb_downloads'] for run in runs])}")
        print(f"  {'TLS connections':22s} {cell([run['connections'] for run in runs])}")
