mod host;

use std::{ffi::OsString, process::ExitCode};
use trine::{
	application::{CliViewContext, json::JsonResponse},
	cli::output_mode::json_requested,
	error::{Failure, FailureKind},
};

fn main() -> ExitCode {
	let mut arguments: Vec<OsString> = std::env::args_os().collect();
	let output = CliViewContext::default();
	if arguments.len() == 1 {
		let version = format!("workyard {}\n", env!("CARGO_PKG_VERSION"));
		if output.stdout.write_all(version.as_bytes()).is_err() {
			return ExitCode::FAILURE;
		}
		arguments.push("--help".into());
	}

	let json = json_requested(&arguments);
	match host::run(arguments, output.clone()) {
		Ok(code) => ExitCode::from(code),
		Err(error) => {
			report_failure(&output, json, error);
			ExitCode::FAILURE
		}
	}
}

fn report_failure(output: &CliViewContext, json: bool, error: Failure) {
	let message = std::error::Error::source(&error)
		.map(ToString::to_string)
		.unwrap_or_else(|| error.to_string());
	if !json || error.kind == FailureKind::OutputFailed {
		let _ = output.stderr.write_all(format!("{message}\n").as_bytes());
		return;
	}

	let response = JsonResponse::failure(error.kind, message);
	if let Err(error) = response.write(&output.stdout) {
		let _ = output.stderr.write_all(format!("{error}\n").as_bytes());
	}
}
