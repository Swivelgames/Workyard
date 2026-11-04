//! Process-group signalling and completion observation.
use std::{
	io,
	process::{Child, Command},
};
use workyard_core::Problem;

pub struct Managed {
	pub child: Child,
	pid: u32,
}
impl Managed {
	pub fn spawn(mut command: Command) -> Result<Self, Problem> {
		use std::os::unix::process::CommandExt;
		command.process_group(0);
		let child = command.spawn().map_err(|e| Problem::io("Launch Git", e))?;
		let pid = child.id();
		Ok(Self { child, pid })
	}

	pub fn interrupt(&mut self) -> Result<(), Problem> {
		interrupt_group(self.pid)
	}

	pub fn alive(&self) -> Result<bool, Problem> {
		group_alive(self.pid)
	}
}

fn interrupt_group(pid: u32) -> Result<(), Problem> {
	// SAFETY: pid is the fresh group leader created by this invocation.
	let result = unsafe { libc::kill(-(pid as i32), libc::SIGINT) };
	if result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
		return Ok(());
	}
	Err(Problem::io(
		"Interrupt Git process group",
		io::Error::last_os_error(),
	))
}

fn group_alive(pid: u32) -> Result<bool, Problem> {
	// Check the group first. Most commands have no surviving descendants and
	// should not depend on access to unrelated processes in /proc.
	// SAFETY: signal zero only observes the invocation's process group.
	if unsafe { libc::kill(-(pid as i32), 0) } != 0 {
		let error = io::Error::last_os_error();
		if error.raw_os_error() == Some(libc::ESRCH) {
			return Ok(false);
		}

		if error.raw_os_error() != Some(libc::EPERM) {
			return Err(Problem::io("Inspect process group", error));
		}
	}
	#[cfg(target_os = "linux")]
	{
		linux_group_alive(pid)
	}
	#[cfg(not(target_os = "linux"))]
	{
		// SAFETY: signal zero only observes the invocation's process group.
		if unsafe { libc::kill(-(pid as i32), 0) } == 0 {
			return Ok(true);
		}

		let error = io::Error::last_os_error();
		if error.raw_os_error() == Some(libc::ESRCH) {
			return Ok(false);
		}
		Err(Problem::io("Inspect Git process group", error))
	}
}

#[cfg(target_os = "linux")]
fn linux_group_alive(pid: u32) -> Result<bool, Problem> {
	// Orphan zombies can remain under container PID 1. They cannot write and
	// must not keep an otherwise settled operation locked forever.
	let mut observed_member = false;
	for entry in std::fs::read_dir("/proc").map_err(|e| Problem::io("Inspect process group", e))? {
		let entry = entry.map_err(|e| Problem::io("Inspect process entry", e))?;
		if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
			continue;
		}

		let stat = match std::fs::read_to_string(entry.path().join("stat")) {
			Ok(stat) => stat,
			Err(error)
				if matches!(
					error.kind(),
					io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
				) =>
			{
				continue;
			}
			Err(error) => return Err(Problem::io("Inspect process state", error)),
		};
		let Some((_, fields)) = stat.rsplit_once(')') else {
			continue;
		};
		let fields: Vec<_> = fields.split_whitespace().collect();
		if fields.len() <= 2 || fields[2].parse::<u32>().ok() != Some(pid) {
			continue;
		}

		observed_member = true;
		if fields[0] != "Z" && fields[0] != "X" {
			return Ok(true);
		}
	}
	// If /proc hides every member, retain protection until the group disappears.
	Ok(!observed_member)
}
