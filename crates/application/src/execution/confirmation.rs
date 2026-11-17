//! Interface-neutral decision wording. Trine receives the established escaped message.
use crate::contracts::removal::DecisionRequirement;
use crate::guidance::text::escaped as text;

pub(crate) fn confirmation_message(question: &DecisionRequirement) -> String {
	let participant = text(&question.participant);
	match question.kind.as_str() {
		"remove_task" | "discard_task" => format!(
			"Are you sure you want to remove {}?",
			text(question.task.as_deref().unwrap_or("this task"))
		),
		"retained_history" => format!("Remove {participant}?"),
		"discard_files" => format!("Discard changes and remove {participant}?"),
		"lost_changes" => "Recreate worktrees from retained branches?".into(),
		_ => text(&question.message),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn confirmation_preserves_wording_and_escapes_untrusted_fields_once() {
		let mut question = DecisionRequirement {
			task: Some("feat/界\n".into()),
			kind: "remove_task".into(),
			participant: "api\u{1b}[31m".into(),
			message: "Unknown\tdecision".into(),
		};

		assert_eq!(
			confirmation_message(&question),
			"Are you sure you want to remove feat/界\\n?"
		);
		question.kind = "discard_files".into();
		assert_eq!(
			confirmation_message(&question),
			"Discard changes and remove api\\u{1b}[31m?"
		);
		question.kind = "future_decision".into();
		assert_eq!(confirmation_message(&question), "Unknown\\tdecision");

		// Encoding the request message must not rewrite the structured disclosure.
		assert_eq!(question.participant, "api\u{1b}[31m");
		assert_eq!(question.task.as_deref(), Some("feat/界\n"));
	}
}
