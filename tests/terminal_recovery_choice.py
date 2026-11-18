#!/usr/bin/env python3
"""Exercise copy-recovery file selection with real Git and a POSIX terminal."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time

from terminal_confirmation import fixture, git


def interrupt_copy(binary, root, target):
	"""Fail the first Git repair after Workyard has copied and verified the files."""
	shims = root.parent / "bin"
	shims.mkdir()
	shim = shims / "git"
	real_git = shlex.quote(shutil.which("git"))
	shim.write_text(f'#!/bin/sh\ncase " $* " in *" worktree repair "*) exit 1;; esac\nexec {real_git} "$@"\n')
	shim.chmod(0o755)
	env = {**os.environ, "PATH": f"{shims}{os.pathsep}{os.environ['PATH']}"}
	result = subprocess.run([binary, "relocate", "--keep-source", "--old", root, "--new", target],
		cwd=root, env=env, capture_output=True, timeout=15)
	assert result.returncode != 0 and (target / "feat/task/api/file").is_file(), result


def answer_prompt(command, cwd, prompt_options, keys, before_answer=None):
	master, slave = pty.openpty()
	fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 200, 0, 0))
	buffer = bytearray()
	try:
		with subprocess.Popen(command, cwd=cwd, stdin=slave, stderr=slave,
				stdout=subprocess.PIPE, start_new_session=True) as process:
			try:
				for index, key in enumerate(keys):
					deadline = time.monotonic() + 12
					# Inquire may wrap a long path inside an option.
					while not all(option in buffer.replace(b"\r\n", b"") for option in prompt_options):
						assert process.poll() is None and time.monotonic() < deadline, buffer
						if select.select([master], [], [], 0.1)[0]:
							part = os.read(master, 65536)
							buffer.extend(part)
							if b"\x1b[6n" in part:
								os.write(master, b"\x1b[1;1R")
					if index == 0 and before_answer:
						before_answer()
					buffer.clear()
					os.write(master, key)
				stdout, _ = process.communicate(timeout=12)
				return process.returncode, stdout
			finally:
				if process.poll() is None:
					process.kill()
	finally:
		os.close(master)
		os.close(slave)


def assert_pending_decision(binary, base, state, expected):
	cli = subprocess.run([binary, "recover", "--state-dir", state, "--json"],
		cwd=base, capture_output=True, timeout=15)
	assert cli.returncode != 0, cli
	outcome = json.loads(cli.stdout)["result"]
	assert outcome["problem"]["code"] == "decision_required", outcome
	decision = outcome["recovery_choice"].copy()
	backup = decision.pop("backup", None)
	assert decision == expected, outcome
	assert bool(backup) == expected["destination_occupied"], decision

	request = {
		"jsonrpc": "2.0", "id": 1, "method": "invoke",
		"params": {"action": ["recover"], "input": {"state_dir": str(state)}},
	}
	rpc = subprocess.run([binary, "rpc"], cwd=base, input=json.dumps(request) + "\n",
		text=True, capture_output=True, timeout=15, check=True)
	assert json.loads(rpc.stdout)["error"]["data"] == outcome, rpc.stdout
	return Path(backup) if backup else None


def scenario(binary, occupied, choice, edit_during_prompt=False, cancel_key=None):
	with tempfile.TemporaryDirectory(prefix="wy-choice-") as directory:
		base = Path(directory).resolve()
		root = fixture(base, binary)
		target = base / "new"
		interrupt_copy(binary, root, target)
		endpoint = target / "feat/task/api"
		local = root / "feat/task/api"
		scratch = base / "scratch"
		if occupied:
			with (base / "api/.git/info/exclude").open("a") as excludes:
				excludes.write("\nlocal-only\n")
			shutil.copytree(endpoint, scratch, symlinks=True)
			(endpoint / "local-only").write_text("keep me")
		else:
			endpoint.rename(scratch)
		git(base / "api", "worktree", "repair", str(scratch))
		(scratch / "scratch-only").write_text("chosen work")
		command = [binary, "recover", "--state-dir", root / ".workyard"]

		expected = {
			"participant": "api",
			"destination": str(endpoint),
			"destination_occupied": occupied,
			"local_candidate": str(endpoint if occupied else local),
			"registered_candidate": str(scratch),
		}
		backup = assert_pending_decision(binary, base, root / ".workyard", expected)

		first = f"Keep files at {endpoint}" if occupied else f"Use files at {local}"
		second = f"Use files at {scratch} and keep a separate backup at {backup}" if occupied else f"Use files at {scratch}"
		labels = [f"Recover api to {endpoint}".encode(), first.encode(), second.encode(), b"Cancel"]
		keys = [cancel_key or b"\x1b[B" * choice + b"\r"]
		before_answer = None
		if edit_during_prompt:
			def edit_registered_file():
				(scratch / "scratch-only").write_text("changed at prompt")

			before_answer = edit_registered_file
			keys.append(b"\r")

		code, result = answer_prompt(command, base, labels, keys, before_answer)
		if choice == 2 or cancel_key:
			assert code != 0 and (scratch / "scratch-only").exists(), result
			assert str(scratch).encode() in git(base / "api", "worktree", "list").stdout
			assert (endpoint / "local-only").is_file() == occupied
			return
		assert code == 0, result
		use_scratch = choice == 1 and not edit_during_prompt
		if backup and use_scratch:
			assert (backup / "local-only").read_text() == "keep me"
			assert backup.name.encode() in result, result
		elif backup:
			assert not backup.exists()
		assert (endpoint / "scratch-only").exists() == use_scratch
		assert (endpoint / "local-only").exists() == (occupied and not use_scratch)
		assert str(endpoint).encode() in git(base / "api", "worktree", "list").stdout


if __name__ == "__main__":
	binary = str(Path(sys.argv[1]).resolve())
	for occupied in (False, True):
		for choice in range(3):
			scenario(binary, occupied, choice)
	scenario(binary, True, 1, edit_during_prompt=True)
	for key in [b"\x03", b"\x1b"]:
		scenario(binary, True, 0, cancel_key=key)
	print("Recovery selection passed for vacant and occupied destinations.")
