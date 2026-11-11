use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{
	common::Progress,
	invocation::{Events, blocking_action},
};

use super::{CreateInput, CreateOutcome};

pub async fn create(
	input: CreateInput,
	context: InvocationContext<Progress>,
) -> Result<CreateOutcome, CreateOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: CreateInput,
	context: InvocationContext<Progress>,
) -> Result<CreateOutcome, CreateOutcome> {
	let name = input.task.clone();
	let rejected = |problem| CreateOutcome::rejected(name.clone(), problem);
	let request = input.normalize().map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let store = Store::open(&request.tasks_root, &request.state_dir).map_err(rejected)?;
	let runner = Runner::default();

	let result = workyard_core::create(
		request,
		&runner,
		&control,
		&Events(context, "create"),
		&store,
	)
	.await;
	match result {
		Ok((task, record)) => {
			let outcome = CreateOutcome::from_task(task, record);
			if outcome.operation.outcome == OperationOutcome::Failed {
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}

#[cfg(all(test, unix))]
mod tests {
	use std::{
		fs::{self, OpenOptions},
		io::Write,
		os::unix::fs::OpenOptionsExt,
		sync::mpsc,
		time::{Duration, Instant},
	};

	#[test]
	fn blocked_state_work_does_not_block_host_polling() {
		let workspace = tempfile::tempdir().unwrap();
		let state = workspace.path().join(".workyard");
		fs::create_dir(&state).unwrap();
		let manifest = state.join("workspace.json");
		let path = std::ffi::CString::new(manifest.as_os_str().as_encoded_bytes()).unwrap();
		// A real synchronous state read blocks until the host releases its writer.
		assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

		let (entered, ready) = smol::channel::bounded(1);
		let (release, wait) = mpsc::channel();
		let writer = std::thread::spawn(move || {
			let deadline = Instant::now() + Duration::from_secs(5);
			let mut pipe = loop {
				match OpenOptions::new()
					.write(true)
					.custom_flags(libc::O_NONBLOCK)
					.open(&manifest)
				{
					Ok(pipe) => break pipe,
					Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
						assert!(Instant::now() < deadline, "action never opened state");
						std::thread::sleep(Duration::from_millis(5));
					}
					Err(error) => panic!("open state fixture: {error}"),
				}
			};
			entered.send_blocking(()).unwrap();
			// Test-only watchdog releases the read even if the host is blocked.
			let host_responded = wait.recv_timeout(Duration::from_secs(5)).is_ok();
			pipe.write_all(b"invalid state fixture").unwrap();
			host_responded
		});

		let input = crate::create::CreateInput {
			task: "feat/responsive".into(),
			repositories: vec![crate::create::RepositoryInput {
				path: trine::NativePath::new(workspace.path()).unwrap(),
				name: None,
				remote: None,
				from: None,
			}],
			tasks_root: Some(trine::NativePath::new(workspace.path()).unwrap()),
			state_dir: None,
			checkout: false,
			no_fetch: true,
			pull: false,
			remote: None,
			from: None,
		};
		let context = trine::InvocationContext::unobserved(trine::Cancellation::new());
		let host = async move {
			ready.recv().await.unwrap();
			// A watchdog release can close the receiver before this future polls.
			let _ = release.send(());
		};
		let (result, ()) = smol::block_on(smol::future::zip(
			crate::create::create(input, context),
			host,
		));
		assert!(writer.join().unwrap(), "state read blocked host polling");
		assert!(result.is_err(), "fixture must reach the invalid state read");
	}
}
