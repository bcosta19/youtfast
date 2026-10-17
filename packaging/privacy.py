"""Redacted, read-only pre-publication inventory. Never reads raw report blobs.

This is a focused credential-pattern check, not a certification that all private
or personal information has been found. It never rewrites Git history.
"""

import argparse
import collections
import hashlib
import json
import re
import subprocess
from pathlib import Path

from release import ROOT, forbidden_path

PATTERNS = {
    "github_credential": re.compile(rb"(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})"),
    "google_api_key": re.compile(rb"AIza[0-9A-Za-z_-]{35}"),
    "private_key": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
    "google_session_assignment": re.compile(rb"(?:__Secure-[13]P(?:SAPISID|SID)|SAPISID)\s*[=:]\s*[\"']?[A-Za-z0-9_./-]{30,}"),
    "credential_in_url": re.compile(rb"https?://[^\s/:@\"']{1,80}:[^\s/@\"']{8,}@"),
}


def findings(data):
    return {name: len(pattern.findall(data)) for name, pattern in PATTERNS.items()
            if pattern.search(data)}


def git(*args, root=ROOT):
    return subprocess.check_output(["git", *args], cwd=root)


def history(root=ROOT):
    objects = {}
    blocked = set()
    for line in git("rev-list", "--objects", "--all", root=root).decode().splitlines():
        oid, _, path = line.partition(" ")
        objects[oid] = path
        if path and forbidden_path(path):
            blocked.add(oid)
    metadata = git("cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype) %(objectsize)", root=root)
    blobs = {oid: int(size) for oid, kind, size in
             (line.split() for line in metadata.decode().splitlines()) if kind == "blob"}
    matches = []
    skipped_large = 0
    scanned = 0
    lfs_pointers = 0
    for oid in sorted(objects.keys() & blobs.keys() - blocked):
        if blobs[oid] > 8 * 1024 * 1024:
            skipped_large += 1
            continue
        data = git("cat-file", "blob", oid, root=root)
        scanned += 1
        lfs_pointers += data.startswith(b"version https://git-lfs.github.com/spec/v1\n")
        counts = findings(data)
        if counts:
            # No paths, values, snippets or report content enter the output.
            matches.append({"object_id": oid, "categories": counts})
    return {
        "commits": int(git("rev-list", "--count", "--all", root=root)),
        "refs": git("for-each-ref", "--format=%(refname)", root=root).decode().splitlines(),
        "reachable_objects": len(objects), "scanned_blobs": scanned,
        "excluded_sensitive_object_ids": sorted(blocked),
        "excluded_sensitive_blobs": len(blocked & blobs.keys()),
        "oversized_blobs_not_scanned": skipped_large,
        "lfs_pointer_blobs": lfs_pointers,
        "credential_pattern_findings": matches,
        "scope": "reachable local refs; known sensitive blobs excluded without reading; pattern check only",
    }


def working_tree(paths, root=ROOT):
    counts = collections.Counter()
    manifest = []
    for name in sorted(set(paths)):
        if forbidden_path(name):
            raise ValueError("Sensitive/generated path cannot enter the public candidate")
        path = root / name
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("Only regular files inside the checkout can enter the public candidate")
        if path.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("Oversized public candidate file requires separate review")
        data = path.read_bytes()
        counts.update(findings(data))
        manifest.append({"path": name, "sha256": hashlib.sha256(data).hexdigest()})
    return {"files": manifest, "credential_pattern_findings": dict(counts),
            "scope": "explicit file inventory; no history or unlisted untracked files"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["history", "tree"])
    parser.add_argument("--manifest", type=Path, help="JSON array of explicitly selected paths for tree mode")
    args = parser.parse_args()
    if args.mode == "tree":
        if args.manifest is None:
            parser.error("tree mode needs an explicit --manifest")
        report = working_tree(json.loads(args.manifest.read_text()))
    else:
        report = history()
    print(json.dumps(report, indent=2))
