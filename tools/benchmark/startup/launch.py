"""Started by Hyprland (`hl.exec_cmd`): stamps the exec time on the clock the
probe uses (CLOCK_MONOTONIC), then becomes the measured program. The PID stays
the same, so Hyprland's rules for this launch follow it."""
import os
import sys
import time

stamp, program, args = sys.argv[1], sys.argv[2], sys.argv[3:]
with open(stamp, "w") as handle:
    handle.write(f"{os.getpid()} {time.monotonic():.6f}\n")
os.execv(program, [program, *args])
