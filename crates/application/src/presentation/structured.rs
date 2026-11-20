//! Structured interface defaults selected by Workyard.
use crate::contracts::Progress;

/// Each subscribed Workyard view can buffer this many participant events.
pub(crate) const EVENT_CAPACITY: std::num::NonZeroUsize = std::num::NonZeroUsize::new(16).unwrap();

pub(crate) fn progress_binding<O, E>() -> trine::structured::StructuredBinding<O, E, Progress>
where
	O: serde::Serialize + Sync + 'static,
	E: serde::Serialize + Sync + 'static,
{
	trine::structured::StructuredBinding::serde_progress(EVENT_CAPACITY)
}
