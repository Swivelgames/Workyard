use process_wrap::std::{ChildWrapper, CommandWrap};
use smol::io::AsyncReadExt;
use std::{io, process::Stdio, time::Duration};

/// Drain each pipe independently so a full stderr pipe cannot block stdout.
async fn drain(reader: impl io::Read + Send + 'static) -> io::Result<usize> {
	let mut reader = smol::Unblock::new(reader);
	let mut buffer = [0; 4096];
	let mut total = 0;
	loop {
		let count = reader.read(&mut buffer).await?;
		if count == 0 {
			return Ok(total);
		}
		total += count;
	}
}

fn spawn_fixture() -> io::Result<Box<dyn ChildWrapper>> {
	let mut command = CommandWrap::with_new(std::env::current_exe()?, |command| {
		command.arg("fixture").stdin(Stdio::null());
		command.stdout(Stdio::piped()).stderr(Stdio::piped());
	});
	#[cfg(unix)]
	command.wrap(process_wrap::std::ProcessGroup::leader());
	#[cfg(windows)]
	command.wrap(process_wrap::std::JobObject);
	command.spawn()
}

/// This controlled fixture terminates naturally; the smoke test needs no kill timer.
fn fixture() -> io::Result<()> {
	use io::Write;
	for _ in 0..128 {
		io::stdout().write_all(&[b'o'; 4096])?;
		io::stderr().write_all(&[b'e'; 4096])?;
	}
	Ok(())
}

async fn smoke() -> io::Result<()> {
	let mut child = spawn_fixture()?;
	let stdout = smol::spawn(drain(child.stdout().take().expect("piped stdout")));
	let stderr = smol::spawn(drain(child.stderr().take().expect("piped stderr")));
	let mut ticks = 0;
	let status = loop {
		if let Some(status) = child.try_wait()? {
			break status;
		}
		smol::Timer::after(Duration::from_millis(10)).await;
		ticks += 1;
	};
	let out = stdout.await?;
	let err = stderr.await?;
	if !status.success() || out != 524288 || err != 524288 {
		return Err(io::Error::other(format!(
			"unexpected result: {status}, stdout={out}, stderr={err}"
		)));
	}
	println!("PASS: drained {out} stdout bytes and {err} stderr bytes; executor ticks={ticks}");
	Ok(())
}

fn main() -> io::Result<()> {
	if std::env::args().nth(1).as_deref() == Some("fixture") {
		return fixture();
	}
	smol::block_on(smoke())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn drains_both_pipes() -> io::Result<()> {
		// Unit-test executables have their own argument parser; test a separate fixture.
		#[cfg(unix)]
		{
			let mut command = CommandWrap::with_new("sh", |command| {
				command.args(["-c", "printf stdout; printf stderr >&2"]);
				command.stdout(Stdio::piped()).stderr(Stdio::piped());
			});
			let mut child = command.spawn()?;
			smol::block_on(async {
				let out = smol::spawn(drain(child.stdout().take().unwrap()));
				let err = smol::spawn(drain(child.stderr().take().unwrap()));
				assert!(smol::unblock(move || child.wait()).await?.success());
				assert_eq!(out.await?, 6);
				assert_eq!(err.await?, 6);
				Ok::<_, io::Error>(())
			})?;
		}
		Ok(())
	}

	#[test]
	fn missing_executable_is_a_launch_error() {
		let mut command = CommandWrap::with_new("/nonexistent/workyard-prototype-fixture", |_| {});
		assert!(command.spawn().is_err());
	}

	#[cfg(unix)]
	#[test]
	fn cancellation_interrupts_process_group() -> io::Result<()> {
		use std::io::BufRead;
		let mut command = CommandWrap::with_new("sh", |command| {
			command.args(["-c", "trap 'exit 42' INT; echo ready; sleep 1; exit 43"]);
			command.stdout(Stdio::piped()).stderr(Stdio::null());
		});
		command.wrap(process_wrap::std::ProcessGroup::leader());
		let mut child = command.spawn()?;
		let mut ready = String::new();
		io::BufReader::new(child.stdout().take().unwrap()).read_line(&mut ready)?;
		assert_eq!(ready.trim(), "ready");
		child.signal(2)?; // SIGINT on supported Unix targets.
		let status = smol::block_on(smol::unblock(move || child.wait()))?;
		assert_eq!(status.code(), Some(42));
		Ok(())
	}
}
