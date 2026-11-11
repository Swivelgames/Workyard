use super::Reader;
use crate::{exists, task_filename};
use workyard_core::{ProblemCode, TaskSnapshot};

impl Reader {
	pub(super) fn inspect_task_gate(&self, snapshot: &mut TaskSnapshot) {
		let Some(name) = &snapshot.name else {
			return;
		};
		let path = self
			.state
			.join("locks")
			.join(task_filename(&name.to_ascii_lowercase()))
			.with_extension("lock");
		match exists(&path) {
			Ok(false) => {}
			Err(problem) => snapshot.problems.push(problem),
			Ok(true) => match crate::Lock::read_existing(&path) {
				Ok(_guard) => {}
				Err(problem) if problem.code == ProblemCode::ResourceBusy => snapshot.busy = true,
				Err(problem) => snapshot.problems.push(problem),
			},
		}
	}
}
