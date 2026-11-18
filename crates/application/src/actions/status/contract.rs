use serde::Serialize;

#[derive(Serialize)]
#[serde(transparent)]
pub struct StatusOutcome(pub crate::observation::Observation);

/// Placement used when rendering commands is local to the registered view.
#[derive(Serialize)]
#[serde(transparent)]
pub(crate) struct PresentedStatus {
	pub outcome: StatusOutcome,
	#[serde(skip)]
	pub explicit_placement: bool,
}
