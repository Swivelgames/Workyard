use trine::{InvocationContext, NativePath};
use workyard_core::{Control, OperationOutcome, ProblemCode};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{
	contracts::Progress,
	execution::{Events, blocking_action},
};

use super::{CreateInput, CreateOutcome, contract::ExistingTaskPreview};

/// Establish the task and its participant worktrees.
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

	let cancellation = context.cancellation().clone();
	let control = Control::new({
		let cancellation = cancellation.clone();
		move || cancellation.is_cancelled()
	});
	control.check().map_err(rejected)?;

	let checkout_hint = super::retry::checkout(&input);
	let starting_branch_example = super::retry::starting_branch_example(&input);
	let explicit_workspace = input.workspace.clone();
	let request = input.prepare(&control).await.map_err(rejected)?;
	let workspace = request
		.state_dir
		.parent()
		.and_then(|path| NativePath::new(path).ok());
	let store =
		Store::open_existing(&request.state_dir, Some(&request.tasks_root)).map_err(rejected)?;
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
		Ok(result) => {
			let registration =
				crate::projects::register(&store, &result.task.admitted_roots(None), &control)
					.await;
			let mut outcome = CreateOutcome::from_task(result.task, result.record);
			outcome.explicit_workspace = explicit_workspace;
			outcome.operation = result.operation.into();
			outcome.problem = result.problem.map(Into::into);
			outcome.registration_problem = registration.map(Into::into);
			if outcome.registration_problem.is_some()
				|| outcome.operation.outcome == OperationOutcome::Failed
			{
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => {
			let code = problem.code;
			let mut outcome = rejected(problem);
			match code {
				ProblemCode::TaskExists => {
					outcome.existing_task = Some(ExistingTaskPreview {
						workspace,
						cancellation,
					});
					outcome.explicit_workspace = explicit_workspace;
				}
				ProblemCode::BranchExists => outcome.checkout_hint = checkout_hint,
				ProblemCode::DefaultBranchUnknown => {
					outcome.starting_branch_example = starting_branch_example;
				}
				_ => {}
			}
			Err(outcome)
		}
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
		workyard_state::initialize_workspace(workspace.path(), None).unwrap();
		let repo = workspace.path().join("api");
		make_repository(&repo);

		let blocked_record = state.join("tasks/other.json");
		let path = std::ffi::CString::new(blocked_record.as_os_str().as_encoded_bytes()).unwrap();
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
					.open(&blocked_record)
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
			projects: vec![],
			repositories: vec![crate::contracts::RepositoryInput {
				path: trine::NativePath::new(repo).unwrap(),
				name: None,
				remote: None,
				from: None,
			}],
			workspace: Some(trine::NativePath::new(workspace.path()).unwrap()),
			checkout: false,
			no_fetch: true,
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

	fn make_repository(path: &std::path::Path) {
		fs::create_dir(path).unwrap();
		for args in [
			&["init", "-b", "main"][..],
			&["commit", "--allow-empty", "-m", "fixture"],
		] {
			let result = std::process::Command::new("git")
				.current_dir(path)
				.args([
					"-c",
					"user.name=Test",
					"-c",
					"user.email=test@example.invalid",
				])
				.args(args)
				.output()
				.unwrap();
			assert!(result.status.success(), "{result:?}");
		}
	}
}
