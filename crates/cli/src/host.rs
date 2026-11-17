//! Host commands and presentation selection around the shared action catalog.
use clap::{Arg, Command, error::ErrorKind};
use std::ffi::OsString;
use trine::{
	Cancellation,
	application::{Application, CliViewContext},
	cli::{Parsed, completion, output_mode::json_requested},
	error::{Failure, FailureKind},
	rpc,
};

pub(crate) fn run(arguments: Vec<OsString>, output: CliViewContext) -> Result<u8, Failure> {
	let application = workyard_application::application()
		.map_err(|error| Failure::message(FailureKind::ContractViolation, error.to_string()))?;
	let mut command = command(&application);
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
		Some(("rpc", _)) => {
			reject_json(&arguments, "rpc")?;
			run_rpc(&application)
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

fn command(application: &Application) -> Command {
	let shell =
		Arg::new("shell")
			.required(true)
			.value_parser(["bash", "zsh", "fish", "powershell"]);
	let completion = Command::new("completion")
		.about("Print a shell completion script")
		.arg(shell);
	let rpc = Command::new("rpc").about("Serve JSON-RPC over standard input/output");

	application
		.cli_command()
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

fn run_rpc(application: &Application) -> Result<u8, Failure> {
	let cancellation = signal_cancellation()?;
	let (reader, writer) =
		rpc::stdio::open().map_err(|error| Failure::new(FailureKind::Unavailable, error))?;
	let report = smol::block_on(rpc::serve(
		application,
		reader,
		writer,
		rpc::Config::default(),
		cancellation,
		|_| {},
	))
	.map_err(|error| Failure::new(FailureKind::OutputFailed, error))?;
	if let Some(failure) = report.failure {
		return Err(Failure::message(FailureKind::Unavailable, failure));
	}
	Ok(0)
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
