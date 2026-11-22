/// Machine-readable classification carried by a Workyard problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemCode {
	/// No task matches the requested name.
	TaskUnknown,
	/// The requested membership change would leave the task without participants.
	EmptyTask,
	/// The operation requires a choice or explicit consent to continue.
	DecisionRequired,
	/// Creating the target branch would collide with an existing branch.
	BranchExists,
	/// Another worktree already has the requested branch checked out.
	BranchInUse,
	/// The caller requested cancellation.
	Cancelled,
	/// Stored data could not be validated or reconciled.
	DamagedState,
	/// The repository's default starting branch could not be determined.
	DefaultBranchUnknown,
	/// Git reported an unsuccessful exit.
	GitFailed,
	/// Git output exceeded the limit for reliable inspection.
	GitOutputLimit,
	/// The stored format or required state capability is unsupported.
	IncompatibleState,
	/// Git returned data the adapter could not interpret.
	InvalidGitOutput,
	/// An argument or combination of arguments is invalid.
	InvalidInput,
	/// The supplied name cannot be used for a participant directory.
	InvalidParticipantName,
	/// The selected Git remote is invalid for this repository.
	InvalidRemote,
	/// The proposed task name is invalid.
	InvalidTask,
	/// The supervised process could not start.
	LaunchFailed,
	/// Multiple participants would occupy the same task-local name.
	ParticipantNameConflict,
	/// A participant needs an explicit task-local name.
	ParticipantNameRequired,
	/// The requested locations overlap in an unsupported way.
	PathOverlap,
	/// Several discovered repositories match the project selector.
	ProjectAmbiguous,
	/// No discovered project matches the selector.
	ProjectUnknown,
	/// A write may be visible but its final publication could not be confirmed.
	PublicationUncertain,
	/// The operation requires an unambiguous remote selection.
	RemoteRequired,
	/// Multiple selections resolve to the same backing repository.
	RepositoryAliasConflict,
	/// The observed repository or worktree no longer matches expectations.
	RepositoryChanged,
	/// Another operation may still own the resource.
	ResourceBusy,
	/// Data could not be encoded for storage or delivery.
	SerializationFailed,
	/// The requested branch starting point could not be resolved.
	SourceMissing,
	/// The named filesystem entry no longer matches its expected identity.
	StaleIdentity,
	/// The process supervisor could not establish a reliable command outcome.
	SupervisionFailed,
	/// The requested task or its destination is already occupied.
	TaskExists,
	/// No initialized workspace was found for the supplied location.
	WorkspaceNotFound,
	/// Workspace state and requested workspace placement disagree.
	WorkspaceBindingConflict,
	/// An operating-system I/O operation failed.
	IoError,
}

impl ProblemCode {
	/// Return the stable spelling used in records or output.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::EmptyTask => "empty_task",
			Self::TaskUnknown => "task_unknown",
			Self::DecisionRequired => "decision_required",
			Self::BranchExists => "branch_exists",
			Self::BranchInUse => "branch_in_use",
			Self::Cancelled => "cancelled",
			Self::DamagedState => "damaged_state",
			Self::DefaultBranchUnknown => "default_branch_unknown",
			Self::GitFailed => "git_failed",
			Self::GitOutputLimit => "git_output_limit",
			Self::IncompatibleState => "incompatible_state",
			Self::InvalidGitOutput => "invalid_git_output",
			Self::InvalidInput => "invalid_input",
			Self::InvalidParticipantName => "invalid_participant_name",
			Self::InvalidRemote => "invalid_remote",
			Self::InvalidTask => "invalid_task",
			Self::LaunchFailed => "launch_failed",
			Self::ParticipantNameConflict => "participant_name_conflict",
			Self::ParticipantNameRequired => "participant_name_required",
			Self::PathOverlap => "path_overlap",
			Self::ProjectAmbiguous => "project_ambiguous",
			Self::ProjectUnknown => "project_unknown",
			Self::PublicationUncertain => "publication_uncertain",
			Self::RemoteRequired => "remote_required",
			Self::RepositoryAliasConflict => "repository_alias_conflict",
			Self::RepositoryChanged => "repository_changed",
			Self::ResourceBusy => "resource_busy",
			Self::SerializationFailed => "serialization_failed",
			Self::SourceMissing => "source_missing",
			Self::StaleIdentity => "stale_identity",
			Self::SupervisionFailed => "supervision_failed",
			Self::TaskExists => "task_exists",
			Self::WorkspaceNotFound => "workspace_not_found",
			Self::WorkspaceBindingConflict => "workspace_binding_conflict",
			Self::IoError => "io_error",
		}
	}
}

impl std::fmt::Display for ProblemCode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

impl std::str::FromStr for ProblemCode {
	type Err = ();

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		match value {
			"empty_task" => Ok(Self::EmptyTask),
			"task_unknown" => Ok(Self::TaskUnknown),
			"decision_required" => Ok(Self::DecisionRequired),
			"branch_exists" => Ok(Self::BranchExists),
			"branch_in_use" => Ok(Self::BranchInUse),
			"cancelled" => Ok(Self::Cancelled),
			"damaged_state" => Ok(Self::DamagedState),
			"default_branch_unknown" => Ok(Self::DefaultBranchUnknown),
			"git_failed" => Ok(Self::GitFailed),
			"git_output_limit" => Ok(Self::GitOutputLimit),
			"incompatible_state" => Ok(Self::IncompatibleState),
			"invalid_git_output" => Ok(Self::InvalidGitOutput),
			"invalid_input" => Ok(Self::InvalidInput),
			"invalid_participant_name" => Ok(Self::InvalidParticipantName),
			"invalid_remote" => Ok(Self::InvalidRemote),
			"invalid_task" => Ok(Self::InvalidTask),
			"launch_failed" => Ok(Self::LaunchFailed),
			"participant_name_conflict" => Ok(Self::ParticipantNameConflict),
			"participant_name_required" => Ok(Self::ParticipantNameRequired),
			"path_overlap" => Ok(Self::PathOverlap),
			"project_ambiguous" => Ok(Self::ProjectAmbiguous),
			"project_unknown" => Ok(Self::ProjectUnknown),
			"publication_uncertain" => Ok(Self::PublicationUncertain),
			"remote_required" => Ok(Self::RemoteRequired),
			"repository_alias_conflict" => Ok(Self::RepositoryAliasConflict),
			"repository_changed" => Ok(Self::RepositoryChanged),
			"resource_busy" => Ok(Self::ResourceBusy),
			"serialization_failed" => Ok(Self::SerializationFailed),
			"source_missing" => Ok(Self::SourceMissing),
			"stale_identity" => Ok(Self::StaleIdentity),
			"supervision_failed" => Ok(Self::SupervisionFailed),
			"task_exists" => Ok(Self::TaskExists),
			"workspace_not_found" => Ok(Self::WorkspaceNotFound),
			"workspace_binding_conflict" => Ok(Self::WorkspaceBindingConflict),
			"io_error" => Ok(Self::IoError),
			_ => Err(()),
		}
	}
}
