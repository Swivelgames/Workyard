//! Participant placement and the Git/state adapters used during relocation.
use crate::{Control, Participant, Problem, ProblemCode, Repository};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RelocationParticipant {
	pub task: String,
	pub original: Participant,
	pub destination: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredCopy {
	/// The path Git currently registers. A previous manual copy may still supply files.
	pub bound_at: PathBuf,
	pub path: PathBuf,
	pub administration: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryChoice {
	pub participant: String,
	pub destination: PathBuf,
	pub first: PathBuf,
	pub registered: PathBuf,
	pub occupied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryCandidates {
	pub choice: RecoveryChoice,
	pub first_digest: Option<String>,
	pub registered_digest: String,
}

pub trait RelocationGit {
	fn recovery_registration(
		&self,
		entry: &RelocationParticipant,
		undo: bool,
		_previous: Option<&std::path::Path>,
		control: &Control,
	) -> impl std::future::Future<Output = Result<Option<RegisteredCopy>, Problem>> + Send {
		self.displaced_copy(entry, undo, control)
	}

	fn displaced_copy(
		&self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_control: &Control,
	) -> impl std::future::Future<Output = Result<Option<RegisteredCopy>, Problem>> + Send {
		async { Ok(None) }
	}

	fn bind_displaced_copy(
		&self,
		entry: &RelocationParticipant,
		undo: bool,
		_registration: &RegisteredCopy,
		control: &Control,
	) -> impl std::future::Future<Output = Result<(), Problem>> + Send {
		self.bind_copy(entry, undo, control)
	}

	fn copy_is_bound(
		&self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_control: &Control,
	) -> impl std::future::Future<Output = Result<bool, Problem>> + Send {
		async {
			Err(Problem::new(
				ProblemCode::InvalidInput,
				"This Git adapter does not support copy relocation.",
			))
		}
	}

	fn bind_copy(
		&self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_control: &Control,
	) -> impl std::future::Future<Output = Result<(), Problem>> + Send {
		async {
			Err(Problem::new(
				ProblemCode::InvalidInput,
				"This Git adapter does not support copy relocation.",
			))
		}
	}

	fn relocation_relationship(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl std::future::Future<Output = Result<crate::Relationship, Problem>> + Send;
	fn repair_relocation(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl std::future::Future<Output = Result<(), Problem>> + Send;
}

pub trait RelocationState {
	fn prior_registered_copy(
		&self,
		_entry: &RelocationParticipant,
	) -> Result<Option<PathBuf>, Problem> {
		Ok(None)
	}
	fn accept_registered_endpoint(
		&mut self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_control: &Control,
	) -> Result<(), Problem> {
		Ok(())
	}

	fn settle_recovery_direction(
		&mut self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_registered: &RegisteredCopy,
		_control: &Control,
	) -> Result<(), Problem> {
		Ok(())
	}

	fn recovery_candidates(
		&self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_registered: &RegisteredCopy,
		_control: &Control,
	) -> Result<Option<RecoveryCandidates>, Problem> {
		Ok(None)
	}
	fn select_recovery(
		&mut self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_registered: &RegisteredCopy,
		_candidates: &RecoveryCandidates,
		_use_registered: bool,
	) -> Result<(), Problem> {
		Err(Problem::new(
			ProblemCode::InvalidInput,
			"This state adapter cannot resolve a displaced worktree.",
		))
	}
	fn install_recovery(
		&mut self,
		_entry: &RelocationParticipant,
		_undo: bool,
		_registered: &RegisteredCopy,
		_control: &Control,
	) -> Result<(), Problem> {
		Err(Problem::new(
			ProblemCode::InvalidInput,
			"This state adapter cannot install a displaced worktree.",
		))
	}

	fn is_copy(&self) -> bool {
		false
	}
	fn copy_participant_expected(&self, _entry: &RelocationParticipant) -> bool {
		true
	}
	fn participants(&self) -> Vec<RelocationParticipant>;
	fn activated(&self) -> bool;
	fn started(&self) -> bool;
	fn relocated_path(&self, path: &std::path::Path) -> Result<PathBuf, Problem>;
	fn verify(&self) -> Result<(), Problem>;
	fn begin(&mut self, participants: Vec<RelocationParticipant>) -> Result<(), Problem>;
	fn transfer(&mut self, undo: bool, control: &Control) -> Result<(), Problem>;
	fn pending_writer(&self) -> bool;
	fn pending_writer_description(&self) -> Option<String> {
		None
	}
	fn copy_writer(&mut self, _entry: &RelocationParticipant, _undo: bool) -> Result<(), Problem> {
		self.writer(true)
	}
	fn writer(&mut self, pending: bool) -> Result<(), Problem>;
	fn activate(&mut self, undo: bool) -> Result<(), Problem>;
	fn cleanup(&mut self) -> Result<(), Problem>;
}
