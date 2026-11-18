#!/usr/bin/env python3
"""A controlled Git hook with an optional child in an independent session."""
import os
from pathlib import Path
import subprocess
import sys
import time


def wait_for(path):
	deadline = time.monotonic() + 20
	while not path.exists():
		if time.monotonic() > deadline:
			sys.exit("fixture release timed out")
		time.sleep(0.02)


fixture = Path(os.environ["WY_FIXTURE"])
if sys.argv[1:] == ["child"]:
	wait_for(fixture / "release-child")
	sys.exit(0)

if os.environ["WY_CASE"] == "child":
	subprocess.Popen([sys.executable, __file__, "child"], start_new_session=True,
		stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
(fixture / "hook-entered").touch()
wait_for(fixture / "release-git")
sys.exit(1 if os.environ["WY_CASE"] == "failure" else 0)
