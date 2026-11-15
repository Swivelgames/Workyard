#!/usr/bin/env python3
"""Linux/macOS PTY smoke checks for Workyard's actual Inquire confirmations.

Run with the absolute path to the built workyard binary. Uses disposable repos;
requires Python 3 and Git. Windows needs equivalent native console verification.
"""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


def git(directory, *arguments):
	return subprocess.run(["git", "-C", str(directory), *arguments], check=True,
		stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)


def fixture(directory, binary, dirty=True, multiple=False):
	repository = directory / "api"
	repository.mkdir()
	git(repository, "init", "-b", "main")
	git(repository, "config", "user.email", "fixture@example.invalid")
	git(repository, "config", "user.name", "Fixture")
	(repository / "file").write_text("initial\n")
	git(repository, "add", "file")
	git(repository, "commit", "-m", "initial")

	root = directory / "workspace"
	root.mkdir()
	repositories = ["--repo", str(repository)]
	if multiple:
		web = directory / "web"
		git(directory, "clone", str(repository), str(web))
		repositories += ["--repo", str(web)]
	subprocess.run([binary, "create", "feat/task", *repositories,
		"--from", "main", "--no-fetch"], cwd=root, check=True,
		stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
	if dirty:
		(root / "feat/task/api/file").write_text("dirty\n")
	return root


def assert_disclosure(output, dirty, multiple):
	assert b"\t" not in output, "terminal output contains a literal tab"
	assert output.count(b"no upstream") == (2 if multiple else 1), output
	question = b"Are you sure you want to remove feat/task?"
	assert question in output, output
	if dirty:
		assert output.count(b"Would discard: file") == 1, output
		assert output.index(b"Would discard: file") < output.index(question), output
		warning = b"Removing this task will discard the disclosed files."
		assert output.index(warning) < output.index(question), output
		assert b"? Are you sure" not in output, "default prompt decoration remains"
	else:
		assert output.index(b"no upstream") < output.index(question), output
		assert b"? Are you sure" not in output, "default prompt decoration remains"
	assert b"commits remain on the retained branch" not in output, output


def durable_state(root):
	return {
		str(path.relative_to(root)): path.read_bytes()
		for directory in ("tasks", "evidence")
		for path in (root / ".workyard" / directory).iterdir()
	}


def assert_healthy_retry(root, binary, before, dirty):
	assert durable_state(root) == before, "decline rewrote task history or evidence"
	status = subprocess.run([binary, "status", "feat/task"], cwd=root,
		stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
	assert status.returncode == 0, status.stdout
	assert b"[healthy]" in status.stdout, status.stdout
	assert b"Recorded operation: remove" not in status.stdout, status.stdout

	option = "--force" if dirty else "--yes"
	retry = subprocess.run([binary, "remove", "feat/task", option], cwd=root,
		stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
	assert retry.returncode == 0, retry.stdout


def terminal_case(binary, key, expected, consent=False, dirty=True, multiple=False):
	with tempfile.TemporaryDirectory(prefix="workyard-terminal-") as temporary:
		root = fixture(Path(temporary), binary, dirty=dirty, multiple=multiple)
		before_state = durable_state(root)
		master, slave = pty.openpty()
		fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
		before = termios.tcgetattr(slave)
		arguments = [binary, "remove", "feat/task"]
		if dirty:
			arguments.append("--yes")
		process = subprocess.Popen(arguments, cwd=root,
			stdin=slave, stdout=subprocess.PIPE, stderr=slave, start_new_session=True)
		output = bytearray()
		sent = False
		deadline = time.monotonic() + 15
		try:
			while process.poll() is None:
				if time.monotonic() > deadline:
					raise AssertionError(f"prompt did not finish: {output!r}")
				if not select.select([master], [], [], 0.1)[0]:
					continue
				chunk = os.read(master, 65536)
				output.extend(chunk)
				if b"\x1b[6n" in chunk:
					os.write(master, b"\x1b[1;1R")
				if not sent and b"(y/N)" in output:
					os.write(master, key)
					sent = True

			while select.select([master], [], [], 0)[0]:
				output.extend(os.read(master, 65536))

			assert sent, output
			assert_disclosure(output, dirty, multiple)
			result = process.stdout.read()
			assert expected in result, result
			assert b"(y/N)" not in result, "prompt contaminated stdout"
			assert b"\x1b" not in result, "redirected result must remain plain"
			assert b"output_failed" not in result, result
			assert b"output_failed" not in output, output
			assert termios.tcgetattr(slave) == before, "terminal modes were not restored"

			if consent:
				assert not (root / "feat/task/api").exists()
				assert process.returncode == 0
			else:
				assert (root / "feat/task/api/file").read_text() == ("dirty\n" if dirty else "initial\n")
				assert process.returncode != 0
				assert b"No worktrees changed." in result, result
				assert b"needs attention" not in result, result
				assert b"removal unconfirmed" not in result, result
				assert_healthy_retry(root, binary, before_state, dirty)
		finally:
			if process.poll() is None:
				process.kill()
				process.wait()
			os.close(master)
			os.close(slave)


def closed_stdin_case(binary):
	with tempfile.TemporaryDirectory(prefix="workyard-eof-") as temporary:
		root = fixture(Path(temporary), binary)
		before_state = durable_state(root)
		result = subprocess.run([binary, "remove", "feat/task", "--yes"], cwd=root,
			stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
			timeout=10)
		assert result.returncode != 0
		assert b"decision_required" in result.stdout, result.stdout
		assert (root / "feat/task/api/file").read_text() == "dirty\n"
		assert_healthy_retry(root, binary, before_state, True)


if __name__ == "__main__":
	binary = str(Path(sys.argv[1]).resolve())
	for label, key in [("No", b"n\r"), ("Ctrl+C", b"\x03"), ("Escape", b"\x1b"), ("Ctrl+D", b"\x04")]:
		terminal_case(binary, key, b"Removal cancelled")
		print(f"{label}: cancelled, files preserved, terminal restored")
	terminal_case(binary, b"y\r", b"Removal succeeded", consent=True, multiple=True)
	print("Explicit yes: removal succeeds, terminal restored, stdout stays plain")
	terminal_case(binary, b"y\r", b"Removal succeeded", consent=True, dirty=False)
	print("Clean prompt: concise disclosure precedes task confirmation, no decoration/tabs")
	closed_stdin_case(binary)
	print("Closed stdin: decision required, files preserved, no hang")
