#!/usr/bin/env python3
"""Exercise Workyard's human output with a real PTY and interruptible Git probes."""

import errno
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
from terminal_confirmation import durable_state, git


def command(binary, root, *args, env=None, check=True):
	return subprocess.run(
		[binary, *args], cwd=root, env=env, check=check, capture_output=True, timeout=15
	)


def repository(path):
	path.mkdir(parents=True)
	git(path, "init", "-b", "main")
	git(path, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
		"commit", "--allow-empty", "-m", "Fixture")


def create(binary, root, source, task):
	command(binary, root, "create", task, "--repo", str(source), "--from", "main", "--no-fetch")


def terminal(binary, root, action, env):
	master, slave = pty.openpty()
	try:
		with subprocess.Popen([binary, action], cwd=root, env=env,
			stdout=slave, stderr=subprocess.PIPE) as process:
			os.close(slave)
			slave = None
			try:
				data = bytearray()
				deadline = time.monotonic() + 10
				while True:
					timeout = max(0, deadline - time.monotonic())
					ready, _, _ = select.select([master], [], [], timeout)
					assert ready, f"{action} timed out"
					try:
						chunk = os.read(master, 8192)
					except OSError as error:
						if error.errno != errno.EIO:
							raise
						break
					if not chunk:
						break
					data.extend(chunk)
				assert process.wait(timeout=5) == 0, process.stderr.read()
				return bytes(data).replace(b"\r\n", b"\n")
			finally:
				if process.poll() is None:
					process.kill()
	finally:
		os.close(master)
		if slave is not None:
			os.close(slave)


def output_colors(binary):
	with tempfile.TemporaryDirectory(prefix="workyard-output-") as directory:
		root = Path(directory)
		source = root / "api"
		repository(source)
		command(binary, root, "init")
		for task in ("healthy", "damaged", "closed"):
			create(binary, root, source, task)
		command(binary, root, "remove", "closed", "--yes")
		(root / "tasks/damaged/api").rename(root / "displaced")

		env = dict(os.environ, TERM="xterm-256color")
		env.pop("NO_COLOR", None)
		for action in ("list", "status"):
			colored = terminal(binary, root, action, env)
			assert b"[\x1b[32mhealthy\x1b[0m]" in colored, colored
			assert b"[\x1b[31mneeds attention\x1b[0m]" in colored, colored
			if action == "list":
				assert b"[\x1b[2mretired\x1b[0m]" in colored, colored

			plain = re.sub(rb"\x1b\[[0-9;]*m", b"", colored)
			assert terminal(binary, root, action, dict(env, NO_COLOR="1")) == plain
			assert terminal(binary, root, action, dict(env, TERM="dumb")) == plain
			assert command(binary, root, action, env=env).stdout == plain


def interrupt_preview(binary, root, args, env, expected_success, check_stdout=None):
	marker = Path(env["WORKYARD_TEST_PREVIEW_READY"])
	marker.unlink(missing_ok=True)
	with subprocess.Popen([binary, *args], cwd=root, env=env,
		stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
		try:
			deadline = time.monotonic() + 10
			while not marker.exists() and process.poll() is None and time.monotonic() < deadline:
				time.sleep(0.01)
			assert marker.exists(), f"{args} did not reach its Git preview"
			if check_stdout is not None:
				os.set_blocking(process.stdout.fileno(), False)
				prior = process.stdout.read()
				os.set_blocking(process.stdout.fileno(), True)
				assert check_stdout in prior, prior
			process.send_signal(signal.SIGINT)
			stdout, stderr = process.communicate(timeout=5)
			assert (process.returncode == 0) == expected_success, (process.returncode, stdout, stderr)
			assert b"Found 1 project" not in stdout, stdout
			return stdout
		finally:
			if process.poll() is None:
				process.kill()


def cancellation(binary):
	with tempfile.TemporaryDirectory(prefix="workyard-preview-") as directory:
		base = Path(directory)
		root = base / "workspace"
		projects = root / "projects"
		projects.mkdir(parents=True)
		source = projects / "api"
		repository(source)

		shims = base / "bin"
		shims.mkdir()
		shim = shims / "git"
		shim.write_text("""#!/bin/sh
if [ "$WORKYARD_TEST_PREVIEW" = init ] && [ "$1" = --git-dir ]; then
	: > "$WORKYARD_TEST_PREVIEW_READY"
	exec sleep 20
fi
case "$*" in
	*"worktree list --porcelain -z"*)
		if [ "$WORKYARD_TEST_PREVIEW" = create ]; then
			: > "$WORKYARD_TEST_PREVIEW_READY"
			exec sleep 20
		fi
		;;
esac
exec "$WORKYARD_TEST_REAL_GIT" "$@"
""")
		shim.chmod(0o755)
		env = dict(os.environ, PATH=f"{shims}{os.pathsep}{os.environ['PATH']}",
			WORKYARD_TEST_REAL_GIT=shutil.which("git"),
			WORKYARD_TEST_PREVIEW_READY=str(base / "ready"))

		interrupt_preview(binary, root, ["init"], dict(env, WORKYARD_TEST_PREVIEW="init"),
			expected_success=True, check_stdout=b"Workspace initialized")
		assert (root / ".workyard/workspace.json").is_file()

		create(binary, root, source, "feat/duplicate")
		prior = durable_state(root)
		args = ["create", "feat/duplicate", "--repo", str(source), "--from", "main", "--no-fetch"]
		stdout = interrupt_preview(binary, root, args, dict(env, WORKYARD_TEST_PREVIEW="create"),
			expected_success=False)
		assert b"Task name is already in use" in stdout, stdout
		assert prior == durable_state(root)


if __name__ == "__main__":
	binary = str(Path(sys.argv[1]).resolve())
	output_colors(binary)
	print("List and status preserve their facts with terminal color, NO_COLOR, TERM=dumb and pipes")
	cancellation(binary)
	print("Ctrl-C stops optional Git previews and preserves the completed outcome")
