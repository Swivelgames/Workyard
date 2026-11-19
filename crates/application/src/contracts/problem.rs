use serde::Serialize;
use trine::NativePath;
use workyard_core::{Problem, ProjectRemedy};

#[derive(Serialize)]
pub struct SuggestedOverride {
	pub path: NativePath,
	pub name: String,
}

#[derive(Serialize)]
pub struct ProblemOutcome {
	pub code: String,
	pub message: String,
	pub remedy: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub candidates: Vec<NativePath>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub suggested_override: Option<SuggestedOverride>,
}

impl From<Problem> for ProblemOutcome {
	fn from(problem: Problem) -> Self {
		let mut outcome = Self {
			code: problem.code.as_str().into(),
			message: problem.message,
			remedy: problem.remedy,
			candidates: Vec::new(),
			suggested_override: None,
		};
		let Some(detail) = problem.project_remedy else {
			return outcome;
		};
		match *detail {
			ProjectRemedy::Choose(paths) => {
				outcome.candidates = paths
					.into_iter()
					.map(|p| NativePath::new(p).expect("filesystem path"))
					.collect();
			}
			ProjectRemedy::Rename { source, name } => {
				let path = NativePath::new(source).expect("filesystem path");
				if let Some(fragment) = override_arguments(&path, &name) {
					outcome.remedy =
						format!("Use this override in the original command: {fragment}");
				}
				outcome.suggested_override = Some(SuggestedOverride { path, name });
			}
		}
		outcome
	}
}

fn override_arguments(path: &NativePath, name: &str) -> Option<String> {
	use crate::guidance::commands::argument;
	let path = argument(path.as_path().to_str()?)?;
	let name = argument(name)?;
	Some(format!("--repo {path} --repo-name {name}"))
}
