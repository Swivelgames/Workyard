//! Keep the shared CLI/RPC input intact while carrying the MCP client's explicit scope.
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Default)]
pub(crate) enum RemovalConsent {
	#[default]
	CallerOptions,
	ReviewDiscard {
		approval: Option<String>,
	},
}

#[derive(Deserialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct RemovalRequest<I> {
	pub input: I,
	#[serde(skip)]
	pub consent: RemovalConsent,
}

impl<I> From<I> for RemovalRequest<I> {
	fn from(input: I) -> Self {
		Self {
			input,
			consent: RemovalConsent::CallerOptions,
		}
	}
}

impl<I> RemovalRequest<I> {
	pub fn review_discard(input: I, approval: Option<String>) -> Self {
		Self {
			input,
			consent: RemovalConsent::ReviewDiscard { approval },
		}
	}
}
