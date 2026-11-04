#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemCode {
	BranchExists,
	BranchInUse,
	Cancelled,
	DamagedState,
	DefaultBranchUnknown,
	GitFailed,
	GitOutputLimit,
	IncompatibleState,
	InvalidGitOutput,
	InvalidInput,
	InvalidParticipantName,
	InvalidRemote,
	InvalidTask,
	LaunchFailed,
	ParticipantNameConflict,
	ParticipantNameRequired,
	PathOverlap,
	PublicationUncertain,
	RemoteRequired,
	RepositoryAliasConflict,
	RepositoryChanged,
	ResourceBusy,
	SerializationFailed,
	SourceMissing,
	StaleIdentity,
	SupervisionFailed,
	TaskExists,
	WorkspaceBindingConflict,
	IoError,
}

impl ProblemCode {
	pub fn as_str(self) -> &'static str {
		match self {
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
