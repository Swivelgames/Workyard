//! Length-frame native fields so unrelated paths and selections cannot concatenate to the same digest.
use crate::contracts::removal::Disclosure;
use sha2::{Digest, Sha256};
use std::{
	collections::{BTreeMap, BTreeSet},
	fmt::Write,
};
use workyard_core::{AmendRequest, RemovalObservation};

#[derive(Clone, Default)]
pub(super) struct Fingerprint(Sha256);

impl Fingerprint {
	pub fn field(&mut self, bytes: &[u8]) {
		self.0.update((bytes.len() as u64).to_le_bytes());
		self.0.update(bytes);
	}

	pub fn participant(disclosure: &Disclosure, observation: &RemovalObservation) -> [u8; 32] {
		let mut fingerprint = Self::default();
		fingerprint.field(observation.repository.identity.as_bytes());
		fingerprint.field(observation.administration_identity.as_bytes());
		fingerprint.field(&observation.worktree_entry.0.to_le_bytes());
		fingerprint.field(&observation.worktree_entry.1.to_le_bytes());
		if let Some(path) = &observation.destination {
			fingerprint.field(path.as_os_str().as_encoded_bytes());
		}
		fingerprint.optional(observation.branch.as_deref());
		fingerprint.field(disclosure.history.as_bytes());
		fingerprint.field(&observation.dirty_token);
		fingerprint.0.finalize().into()
	}

	pub fn amendment(&mut self, request: &AmendRequest) {
		let options = request.options;
		self.field(&[options.checkout as u8, options.no_fetch as u8]);
		self.optional(request.remote.as_deref());
		self.optional(request.from.as_deref());

		// Use normalized identities and stable order across partial application of the recipe.
		let names: BTreeSet<_> = request.remove.iter().collect();
		self.field(&(names.len() as u64).to_le_bytes());
		for name in names {
			self.field(name.as_bytes());
		}

		let included: BTreeMap<_, _> = request
			.repositories
			.iter()
			.map(|repo| (&repo.expected_identity, repo))
			.collect();
		self.field(&(included.len() as u64).to_le_bytes());
		for (identity, repo) in included {
			self.optional(identity.as_deref());
			self.optional(repo.name.as_deref());
			self.optional(repo.remote.as_deref());
			self.optional(repo.from.as_deref());
		}
	}

	pub fn into_token(self) -> String {
		let mut token = String::with_capacity(64);
		for byte in self.0.finalize() {
			write!(&mut token, "{byte:02x}").expect("write to string");
		}
		token
	}

	fn optional(&mut self, text: Option<&str>) {
		self.field(&[u8::from(text.is_some())]);
		if let Some(text) = text {
			self.field(text.as_bytes());
		}
	}
}
