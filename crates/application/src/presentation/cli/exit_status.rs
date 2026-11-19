//! CLI exit policy shared by human and JSON presentations.
use crate::contracts::OperationReport;
use workyard_core::OperationOutcome;

pub(crate) fn operation<T>(
	result: &Result<T, T>,
	report: impl FnOnce(&T) -> &OperationReport,
) -> u8 {
	let outcome = match result {
		Ok(outcome) | Err(outcome) => outcome,
	};
	u8::from(result.is_err() || report(outcome).outcome != OperationOutcome::Succeeded)
}

pub(crate) fn result<O, E>(result: &Result<O, E>) -> u8 {
	u8::from(result.is_err())
}
