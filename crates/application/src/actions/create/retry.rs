//! Rebuild the original selection and overrides for CLI retry examples.
use super::CreateInput;
use crate::guidance::commands::argument;
use trine::NativePath;

pub(super) fn checkout(input: &CreateInput) -> Option<String> {
	command(input, "checkout", input.from.as_deref())
}

pub(super) fn starting_branch_example(input: &CreateInput) -> Option<String> {
	let action = if input.checkout { "checkout" } else { "create" };
	command(input, action, Some("main"))
}

fn command(input: &CreateInput, action: &str, from: Option<&str>) -> Option<String> {
	let mut command = format!("workyard {action}");
	for (flag, value) in [
		("--workspace", native(input.workspace.as_ref())?),
		("--remote", input.remote.as_deref()),
		("--from", from),
	] {
		option(&mut command, flag, value)?;
	}
	if input.no_fetch {
		command.push_str(" --no-fetch");
	}

	for repository in &input.repositories {
		option(&mut command, "--repo", native(Some(&repository.path))?)?;
		option(&mut command, "--repo-name", repository.name.as_deref())?;
		option(&mut command, "--repo-remote", repository.remote.as_deref())?;
		option(&mut command, "--repo-from", repository.from.as_deref())?;
	}

	let projects = input
		.projects
		.iter()
		.map(|project| project.as_path().to_str())
		.collect::<Option<Vec<_>>>()?;
	if projects.iter().any(|project| project.starts_with('-')) {
		command.push_str(" --");
	}
	command.push_str(&format!(" {}", argument(&input.task)?));
	for project in projects {
		command.push_str(&format!(" {}", argument(project)?));
	}
	Some(command)
}

fn native(value: Option<&NativePath>) -> Option<Option<&str>> {
	match value {
		Some(path) => Some(Some(path.as_path().to_str()?)),
		None => Some(None),
	}
}

fn option(command: &mut String, flag: &str, value: Option<&str>) -> Option<()> {
	if let Some(value) = value {
		command.push_str(&format!(" {flag}={}", argument(value)?));
	}
	Some(())
}
