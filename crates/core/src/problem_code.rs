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

impl std::str::FromStr for ProblemCode {
	type Err = ();

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		match value {
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
			"workspace_binding_conflict" => Ok(Self::WorkspaceBindingConflict),
			"io_error" => Ok(Self::IoError),
			_ => Err(()),
		}
	}
}
