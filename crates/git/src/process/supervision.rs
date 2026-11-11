use super::CommandOutput;
use super::capture::capture_pipe;
#[cfg(unix)]
use crate::unix::Managed;
#[cfg(windows)]
use crate::windows::Managed;
use std::{
	ffi::OsString,
	path::Path,
	process::{ExitStatus, Stdio},
	thread,
	time::Duration,
};
use workyard_core::{Control, Problem};

struct Completion {
	status: ExitStatus,
	interrupted: bool,
	failure: Option<Problem>,
}

pub(super) fn supervise(
	executable: &Path,
	directory: &Path,
	arguments: &[OsString],
	control: &Control,
) -> Result<CommandOutput, Problem> {
	control.check()?;
	let command = prepare_command(executable, directory, arguments);

	thread::scope(|scope| {
		// Allocate drain workers before launching Git. Thread allocation failure
		// must not leave an unmanaged child behind or block a full pipe.
		let (send_out, receive_out) = std::sync::mpsc::channel();
		let (send_err, receive_err) = std::sync::mpsc::channel();
		let out = thread::Builder::new()
			.name("workyard-git-stdout".into())
			.spawn_scoped(scope, move || capture_pipe(receive_out))
			.map_err(|error| Problem::io("Start Git stdout drainer", error))?;
		let err = thread::Builder::new()
			.name("workyard-git-stderr".into())
			.spawn_scoped(scope, move || capture_pipe(receive_err))
			.map_err(|error| Problem::io("Start Git stderr drainer", error))?;

		let mut managed = Managed::spawn(command)?;
		send_out
			.send(managed.child.stdout.take().expect("piped stdout"))
			.expect("stdout worker awaits its pipe");
		send_err
			.send(managed.child.stderr.take().expect("piped stderr"))
			.expect("stderr worker awaits its pipe");
		let completion = wait_for_completion(&mut managed, control);

		let stdout = out
			.join()
			.map_err(|_| {
				Problem::new(
					workyard_core::ProblemCode::SupervisionFailed,
					"stdout drainer panicked",
				)
			})?
			.map_err(|e| Problem::io("Drain Git stdout", e))?;
		let stderr = err
			.join()
			.map_err(|_| {
				Problem::new(
					workyard_core::ProblemCode::SupervisionFailed,
					"stderr drainer panicked",
				)
			})?
			.map_err(|e| Problem::io("Drain Git stderr", e))?;
		if let Some(error) = completion.failure {
			return Err(error);
		}

		if completion.interrupted && !completion.status.success() {
			return Err(Problem::new(
				workyard_core::ProblemCode::Cancelled,
				format!(
					"Git interrupted in {directory:?}; managed writers stopped. {}",
					String::from_utf8_lossy(&stderr.bytes)
				),
			));
		}

		Ok(CommandOutput {
			status: completion.status,
			stdout: stdout.bytes,
			stderr: stderr.bytes,
			truncated: stdout.truncated || stderr.truncated,
			interrupted: completion.interrupted,
		})
	})
}

/// Keep protection until both the root and managed descendants have settled.
/// Observation failures retain protection; cancellation never triggers a timeout.
fn wait_for_completion(managed: &mut Managed, control: &Control) -> Completion {
	let mut interrupted = false;
	let mut failure = None;
	let mut status = None;

	loop {
		if control.cancelled() && !interrupted {
			interrupted = true;
			if let Err(error) = managed.interrupt() {
				failure = Some(error);
			}
		}

		if status.is_none() {
			match managed.child.try_wait() {
				Ok(observed) => status = observed,
				Err(error) => failure = Some(Problem::io("Observe Git exit", error)),
			}
		}

		if let Some(status) = status {
			match managed.alive() {
				Ok(false) => {
					return Completion {
						status,
						interrupted,
						failure,
					};
				}
				Ok(true) => {}
				Err(error) => failure = Some(error),
			}
		}

		thread::sleep(Duration::from_millis(10));
	}
}

fn prepare_command(
	executable: &Path,
	directory: &Path,
	arguments: &[OsString],
) -> std::process::Command {
	let mut command = std::process::Command::new(executable);
	command
		.current_dir(directory)
		.args(arguments)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.env("GIT_TERMINAL_PROMPT", "0");

	for variable in [
		"GIT_DIR",
		"GIT_COMMON_DIR",
		"GIT_WORK_TREE",
		"GIT_INDEX_FILE",
		"GIT_OBJECT_DIRECTORY",
		"GIT_ALTERNATE_OBJECT_DIRECTORIES",
		"GIT_NAMESPACE",
		"GIT_SHALLOW_FILE",
		"GIT_QUARANTINE_PATH",
		"GIT_REPLACE_REF_BASE",
		"GIT_PREFIX",
		"GIT_CEILING_DIRECTORIES",
		"GIT_DISCOVERY_ACROSS_FILESYSTEM",
	] {
		command.env_remove(variable);
	}

	command
}
