use std::io::{self, Read};

#[derive(Default)]
pub(super) struct CapturedOutput {
	pub bytes: Vec<u8>,
	pub truncated: bool,
}

/// Drain both pipes continuously on separate owned workers. Presentation cannot
/// block Git, and retained diagnostics have a fixed bound even for a noisy helper.
fn capture(mut reader: impl Read) -> io::Result<CapturedOutput> {
	const LIMIT: usize = 1024 * 1024;
	let mut bytes = Vec::new();
	let mut buffer = [0; 8192];
	let mut truncated = false;

	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			return Ok(CapturedOutput { bytes, truncated });
		}

		let retained = count.min(LIMIT.saturating_sub(bytes.len()));
		bytes.extend_from_slice(&buffer[..retained]);
		truncated |= retained < count;
	}
}

pub(super) fn capture_pipe<R: Read>(
	receiver: std::sync::mpsc::Receiver<R>,
) -> io::Result<CapturedOutput> {
	match receiver.recv() {
		Ok(reader) => capture(reader),
		// Launch/setup failure drops the sender before a pipe exists.
		Err(_) => Ok(CapturedOutput::default()),
	}
}
