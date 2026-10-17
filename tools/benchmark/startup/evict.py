"""Drops what a YouTFast launch reads from the page cache, without root:
POSIX_FADV_DONTNEED on the binary, its libraries (ldd), Mesa's EGL/GL
drivers and every installed font. Pages another process maps stay cached,
as they would on the user's machine. usage: evict.py <binary>"""
import glob
import os
import subprocess
import sys

binary = sys.argv[1]
files = {binary}
for line in subprocess.run(["ldd", binary], capture_output=True, text=True).stdout.splitlines():
    parts = line.split("=>")
    if len(parts) == 2 and parts[1].strip().startswith("/"):
        files.add(parts[1].split("(")[0].strip())
for pattern in ("/usr/lib/libEGL*", "/usr/lib/libGL*", "/usr/lib/libgallium*", "/usr/lib/libLLVM*",
                "/usr/lib/dri/*", "/usr/lib/libvulkan*", "/usr/lib/libwayland*", "/usr/lib/libxkbcommon*"):
    files.update(glob.glob(pattern))
for root, _, names in os.walk("/usr/share/fonts"):
    files.update(os.path.join(root, name) for name in names)
dropped = 0
for path in files:
    try:
        fd = os.open(path, os.O_RDONLY)
    except OSError:
        continue
    try:
        os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        dropped += 1
    finally:
        os.close(fd)
print(f"evicted {dropped} files", file=sys.stderr)
