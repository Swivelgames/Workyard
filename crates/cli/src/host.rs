//! Host commands and presentation selection around the shared action catalog.
use clap::{Arg, Command, error::ErrorKind};
use std::ffi::OsString;
use trine::{
	Cancellation,
	application::{Application, CliViewContext, Output},
	cli::{Parsed, completion, output_mode::json_requested},
	error::{Failure, FailureKind},
	mcp, rpc,
};

pub(crate) fn run(arguments: Vec<OsString>, output: CliViewContext) -> Result<u8, Failure> {
	let mut application = workyard_application::application()
		.map_err(|error| Failure::message(FailureKind::ContractViolation, error.to_string()))?;
	application.cli_heading(heading(&output.stdout));

	let mut command = command(&application);
	if completion::requested() {
		let bytes = std::env::current_dir()
			.ok()
			.and_then(|cwd| completion::respond(command, arguments, &cwd).ok())
			.unwrap_or_default();
		write(&output, &bytes)?;
		return Ok(0);
	}

	let matches = match command.clone().try_get_matches_from(arguments.clone()) {
		Ok(matches) => matches,
		Err(error)
			if matches!(
				error.kind(),
				ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
			) =>
		{
			write(&output, error.to_string().as_bytes())?;
			return Ok(0);
		}
		Err(error) => return Err(Failure::new(FailureKind::InvalidInput, error)),
	};

	match matches.subcommand() {
		Some(("mcp", _)) => {
			reject_json(&arguments, "mcp")?;
			mcp::run_stdio(&application, signal_cancellation()?)?;
			Ok(0)
		}
		Some(("rpc", _)) => {
			reject_json(&arguments, "rpc")?;
			rpc::run_stdio(&application, signal_cancellation()?)?;
			Ok(0)
		}
		Some(("completion", matches)) => {
			reject_json(&arguments, "completion")?;
			let shell = matches.get_one::<String>("shell").expect("required shell");
			let shell = shell.parse().expect("validated shell");
			write(&output, &completion::generate(&mut command, shell))?;
			Ok(0)
		}
		_ => run_action(&application, arguments, output),
	}
}

fn heading(output: &Output) -> String {
	let version = env!("CARGO_PKG_VERSION");
	let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
	let dumb = std::env::var_os("TERM").is_some_and(|value| value == "dumb");
	if !output.is_terminal() || no_color || dumb {
		return format!("\n  🔨 Workyard (v{version})\n");
	}

	format!("\n  \x1b[33m🔨 \x1b[1mWorkyard\x1b[0m \x1b[36m(v{version})\x1b[0m\n")
}

fn command(application: &Application) -> Command {
	let shell =
		Arg::new("shell")
			.required(true)
			.value_parser(["bash", "zsh", "fish", "powershell"]);
	let completion = Command::new("completion")
		.about("Print a shell completion script")
		.arg(shell);
	let mcp = Command::new("mcp").about("Serve agent tools over standard input/output");
	let rpc = Command::new("rpc").about("Serve JSON-RPC over standard input/output");

	application
		.cli_command()
		.subcommand(mcp)
		.subcommand(rpc)
		.subcommand(completion)
}

fn reject_json(arguments: &[OsString], command: &str) -> Result<(), Failure> {
	if json_requested(arguments) {
		return Err(Failure::message(
			FailureKind::InvalidInput,
			format!("--json cannot be combined with {command}"),
		));
	}
	Ok(())
}

fn run_action(
	application: &Application,
	arguments: Vec<OsString>,
	output: CliViewContext,
) -> Result<u8, Failure> {
	let parsed = application
		.parse_cli(arguments)
		.map_err(|error| Failure::new(FailureKind::InvalidInput, error))?;
	match parsed {
		Parsed::Display(text) => {
			write(&output, text.as_bytes())?;
			Ok(0)
		}
		Parsed::Input(prepared) => {
			let report = prepared
				.run(output, signal_cancellation()?)
				.map_err(|error| Failure::message(error.kind(), error.to_string()))?;
			Ok(report.exit_code())
		}
	}
}

fn signal_cancellation() -> Result<Cancellation, Failure> {
	let cancellation = Cancellation::new();
	let signal = cancellation.clone();
	ctrlc::set_handler(move || signal.cancel())
		.map_err(|error| Failure::new(FailureKind::Unavailable, error))?;
	Ok(cancellation)
}

fn write(output: &CliViewContext, bytes: &[u8]) -> Result<(), Failure> {
	output
		.stdout
		.write_all(bytes)
		.map_err(|error| Failure::new(FailureKind::OutputFailed, error))
}
