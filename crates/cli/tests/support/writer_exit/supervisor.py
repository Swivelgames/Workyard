#!/usr/bin/env python3
"""Linux prototype. Only a successful worktree add can publish a receipt."""
import ctypes
import os
from pathlib import Path
import signal
import subprocess
import sys


def drain_descendants():
	while True:
		try:
			os.waitpid(-1, 0)
		except ChildProcessError:
			return


def created_tree_identity(git, args):
	destination = args[args.index("--") + 1]
	admin = subprocess.check_output([
		git, "-C", destination, "rev-parse", "--path-format=absolute", "--git-dir"
	]).removesuffix(b"\n")
	return os.stat(admin)


def supervise(git, args):
	# Linux reparents orphaned descendants to the nearest living subreaper,
	# including descendants that started a new process group or session.
	libc = ctypes.CDLL(None, use_errno=True)
	if libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
		raise OSError(ctypes.get_errno(), "enable child subreaper")

	fixture = Path(os.environ["WY_FIXTURE"])
	with (fixture / "adds").open("a") as counter:
		counter.write("worktree add\n")
	attempt = fixture / "attempt"
	attempt.mkdir()
	# Workyard atomically replaces the effect file on each transition. Pin the
	# original inode before launching Git so a receipt cannot match a later add.
	os.link(os.environ["WY_PENDING"], attempt / "pending")
	if os.environ["WY_CASE"] == "prelaunch":
		(fixture / "supervisor-pid").write_text(str(os.getpid()))
		(fixture / "before-launch").touch()
		signal.pause()  # The test kills the supervisor before it can launch Git.

	result = subprocess.run([git, *args])
	try:
		if result.returncode == 0:
			identity = created_tree_identity(git, args)
		(fixture / "git-exited").touch()
	finally:
		drain_descendants()

	if result.returncode:
		(fixture / "add-failed").touch()
		return 1

	with (attempt / "identity").open("x") as output:
		output.write(f"{identity.st_dev:x}:{identity.st_ino:x}")
		output.flush()
		os.fsync(output.fileno())

	# Publish last. Death before this link leaves the normal pending guard intact.
	os.link(attempt / "pending", attempt / "success")
	directory = os.open(attempt, os.O_RDONLY | os.O_DIRECTORY)
	try:
		os.fsync(directory)
	finally:
		os.close(directory)
	return 0


if __name__ == "__main__":
	real_git = os.environ["WY_REAL_GIT"]
	args = sys.argv[1:]
	if args[:2] != ["worktree", "add"]:
		os.execv(real_git, [real_git, *args])
	sys.exit(supervise(real_git, args))
