use serde::Serialize;

/// Task health and participant observations returned by status.
#[derive(Serialize)]
#[serde(transparent)]
pub struct StatusOutcome(pub crate::observation::Observation);
