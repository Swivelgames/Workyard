use serde::Serialize;

/// Recorded task summaries returned by list.
#[derive(Serialize)]
#[serde(transparent)]
pub struct ListOutcome(pub crate::observation::Observation);
