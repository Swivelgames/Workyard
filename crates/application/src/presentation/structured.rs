//! Structured interface defaults selected by Workyard.
use crate::contracts::Progress;

/// Queue capacity is Workyard presentation policy, not a Trine default.
pub(crate) fn progress_binding<O, E>() -> trine::structured::StructuredBinding<O, E, Progress>
where
	O: serde::Serialize + Sync + 'static,
	E: serde::Serialize + Sync + 'static,
{
	let capacity = std::num::NonZeroUsize::new(16).expect("nonzero capacity");
	trine::structured::StructuredBinding::serde_progress(capacity)
}
