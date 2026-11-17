use workyard_core::Problem;

pub(crate) fn choose_remote(
	remotes: &[String],
	requested: Option<&str>,
) -> Result<Option<String>, Problem> {
	if let Some(requested) = requested {
		if !remotes.iter().any(|remote| remote == requested) {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidRemote,
				format!("Remote {requested:?} is not configured."),
			));
		}
		return Ok(Some(requested.into()));
	}

	if remotes.iter().any(|remote| remote == "origin") {
		return Ok(Some("origin".into()));
	}
	match remotes {
		[] => Ok(None),
		[remote] => Ok(Some(remote.clone())),
		_ => Err(Problem::new(
			workyard_core::ProblemCode::RemoteRequired,
			"Several remotes are configured; supply --remote or --repo-remote.",
		)),
	}
}
