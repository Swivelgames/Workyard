use trine::{Cancellation, InvocationContext, NativePath};
use workyard_application::{CreateInput, CreateOutcome, Progress, RepositoryInput, create};

fn invalid_request() -> CreateInput {
	CreateInput {
		task: "../invalid".into(),
		repositories: vec![RepositoryInput {
			path: NativePath::new(std::path::PathBuf::from("unused-repository")).unwrap(),
			name: None,
			remote: None,
			from: None,
		}],
		tasks_root: None,
		state_dir: None,
		checkout: false,
		no_fetch: true,
		pull: false,
		remote: None,
		from: None,
	}
}

#[test]
fn root_and_capability_imports_support_direct_invocation() {
	let context = || InvocationContext::<Progress>::unobserved(Cancellation::new());

	// Both public paths reach validation before filesystem or Git effects.
	let root: Result<CreateOutcome, CreateOutcome> =
		smol::block_on(create(invalid_request(), context()));
	let scoped: Result<create::CreateOutcome, create::CreateOutcome> =
		smol::block_on(create::create(invalid_request(), context()));

	for result in [root, scoped] {
		let outcome = result.err().expect("invalid task must be rejected");
		assert!(outcome.record.is_none());
		assert!(outcome.participants.is_empty());
		assert!(outcome.problem.is_some());
	}
}
