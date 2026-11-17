"""Linux/macOS live Bash/Zsh acceptance; run after cargo build -p workyard-cli."""
import os
import pty
import select
import shlex
import subprocess
import tempfile
import time
from pathlib import Path


def capture(shell, setup, typed):
	master, slave = pty.openpty()
	argv = [shell, "-f"] if shell == "zsh" else [shell, "--noprofile", "--norc"]
	process = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave,
		env={**os.environ, "TERM": "xterm"})
	os.close(slave)

	def read(seconds):
		output = b""
		deadline = time.monotonic() + seconds
		while time.monotonic() < deadline:
			if select.select([master], [], [], 0.05)[0]:
				try:
					output += os.read(master, 65536)
				except OSError:
					break
		return output

	try:
		read(0.2)
		os.write(master, setup.encode() + b"\n")
		read(1)
		os.write(master, typed.encode())
		return read(1).decode(errors="replace").replace("\a", "")
	finally:
		process.terminate()
		try:
			process.wait(timeout=1)
		except subprocess.TimeoutExpired:
			process.kill()
			process.wait()
		os.close(master)


def snapshot(state):
	return {path.relative_to(state): path.read_bytes() if path.is_file() else None
		for path in state.rglob("*")}


def main():
	binary = Path("target/debug/workyard").resolve()
	with tempfile.TemporaryDirectory(prefix="dynamic-completion-", dir="target") as directory:
		root = Path(directory).resolve()
		repo = root / "source api"
		repo.mkdir()
		subprocess.run(["git", "-C", str(repo), "init", "-b", "main"], check=True, capture_output=True)
		subprocess.run(["git", "-C", str(repo), "-c", "user.name=Fixture",
			"-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-m", "Initial"],
			check=True, capture_output=True)
		workspace = root / "workspace"
		workspace.mkdir()
		for task in ["fix/one", "fix/two"]:
			subprocess.run([str(binary), "create", task, "--repo", str(repo), "--from", "main", "--no-fetch"],
				cwd=workspace, check=True, capture_output=True)
		(workspace / "feat/decoy").mkdir(parents=True)
		(workspace / "some space/child").mkdir(parents=True)
		before = snapshot(workspace / ".workyard")

		for shell in ["bash", "zsh"]:
			initialize = "autoload -Uz compinit; compinit -D; " if shell == "zsh" else ""
			setup = (f"export PATH={shlex.quote(str(binary.parent))}:$PATH; {initialize}"
				f"source <({shlex.quote(str(binary))} completion {shell}); "
				f"cd {shlex.quote(str(workspace))}; PS1='READY> '")
			for typed in ["workyard amend f\t", "workyard amend feat/\t", "workyard create feat/\t"]:
				output = capture(shell, setup, typed)
				assert "command not found" not in output, output
				if typed == "workyard amend f\t":
					assert "fix/" in output, output
				else:
					assert "decoy" not in output, output
				print(shell, repr(typed), "passed")

			if shell == "zsh":
				menu = capture(shell, setup, "workyard amend \t\t")
				assert "Tasks" in menu and "Options" in menu, menu
				assert menu.index("Tasks") < menu.index("fix/one") < menu.index("Options") < menu.index("--repo"), menu
				assert "--repo-name" not in menu, menu
				assert "Print help" in menu, menu
				print("zsh named task-first groups and option descriptions passed")

				hidden = setup + "; zstyle ':completion:*:Tasks' hidden all"
				menu = capture(shell, hidden, "workyard amend \t\t")
				assert "Tasks" not in menu and "fix/one" not in menu, menu
				assert "Options" in menu and "--repo" in menu, menu
				print("zsh hidden-all suppresses task headings and display passed")

				styled = setup + "; zstyle ':completion:*:descriptions' format '== %d =='"
				menu = capture(shell, styled, "workyard amend --repo 'some space' \t\t")
				assert "== Tasks ==" in menu and "== Options ==" in menu, menu
				assert "--repo-name" in menu, menu
				print("zsh styled groups and completed-anchor child options passed")

				metadata = setup + (
					"; workyard() { printf '%s\\n' 'Tasks:%B' 'literal:one%' 'help: 100%' "
					"'Options' '--other' 'literal help'; }"
				)
				menu = capture(shell, metadata, "workyard amend \t\t")
				assert "Tasks:%B" in menu and "Tasks:%%B" not in menu, menu
				assert "literal:one% -- help: 100%" in menu, menu
				assert menu.index("Tasks:%B") < menu.index("literal:one%") < menu.index("Options") < menu.index("--other"), menu
				print("zsh literal punctuation and percent metadata passed")

			# Inspect the actual argv after completion, rather than only terminal redraws.
			inspect = setup + (
				"; workyard() { if [[ $1 == -- ]]; then command workyard \"$@\"; "
				"else printf '\\nARGS:'; printf '<%s>' \"$@\"; fi; }"
			)
			output = capture(shell, inspect, "workyard status fix/o\t\n")
			assert "ARGS:<status><fix/one>" in output, output
			print(shell, "unique task remains exact")
			for quote in ["", "'", '\"']:
				typed = f"workyard info --state-dir {quote}so\tc\t"
				# Zsh leaves the user's original quote open for directory continuation.
				# Bash closes it automatically; its next completion requotes the word.
				if quote and shell == "zsh":
					typed += quote
				output = capture(shell, inspect, typed + "\n")
				assert "ARGS:<info><--state-dir><some space/child/>" in output, output
				print(shell, "two-Tab path", repr(quote), "passed")
			if shell == "bash":
				fallback = inspect + "; enable -n compopt"
				output = capture(shell, fallback, "workyard status fix/o\t\n")
				assert "ARGS:<status><fix/one>" in output, output
				output = capture(shell, fallback, "workyard info --state-dir so\tc\t\n")
				assert "ARGS:<info><--state-dir><some space/child/>" in output, output
				assert "compopt" not in output, output
				print(shell, "no-compopt fallback passed")

			# Explicitly exercise backslash-escaped placement independently of quotes.
			escaped_state = str(workspace / "some space" / ".." / ".workyard").replace(" ", "\\ ")
			output = capture(shell, inspect, f"workyard status --state-dir {escaped_state} fix/o\t\n")
			assert "<fix/one>" in output.split("ARGS:")[-1], output
			print(shell, "escaped placement passed")

			setup += f"; cd {shlex.quote(str(root))}"
			typed = f"workyard amend --state-dir={shlex.quote(str(workspace / '.workyard'))} f\t"
			output = capture(shell, setup, typed)
			assert "fix/" in output, output
			print(shell, "explicit equals placement passed")

		assert snapshot(workspace / ".workyard") == before
		print("State documents and directory entries unchanged.")


if __name__ == "__main__":
	main()
