use serde::Serialize;

#[derive(Serialize)]
#[serde(transparent)]
pub struct StatusOutcome(pub crate::observation::Observation);
