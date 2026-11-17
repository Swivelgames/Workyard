use serde::Serialize;

#[derive(Serialize)]
#[serde(transparent)]
pub struct ListOutcome(pub crate::observation::Observation);
