use std::{ffi::OsString, process::ExitCode};
use trine::{Cancellation, application::CliViewContext, cli::Parsed, rpc};

fn main() -> ExitCode {
	match run() {
		Ok(code) => ExitCode::from(code),
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
	let application = workyard_application::application()?;
	let mut arguments: Vec<OsString> = std::env::args_os().collect();
	if arguments.len() == 1 {
		println!("workyard {}", env!("CARGO_PKG_VERSION"));
		arguments.push("--help".into());
	}

	if arguments.get(1).is_some_and(|argument| argument == "rpc") {
		if arguments.len() != 2 {
			return Err("workyard rpc accepts no command arguments".into());
		}

		return run_rpc(&application);
	}

	match application.parse_cli(arguments)? {
		Parsed::Display(text) => {
			print!("{text}");
			Ok(0)
		}
		Parsed::Input(prepared) => Ok(prepared
			.run(CliViewContext::default(), signal_cancellation()?)?
			.exit_code()),
	}
}

fn run_rpc(
	application: &trine::application::Application,
) -> Result<u8, Box<dyn std::error::Error>> {
	let cancellation = signal_cancellation()?;
	let (reader, writer) = rpc::stdio::open()?;
	let report = smol::block_on(rpc::serve(
		application,
		reader,
		writer,
		rpc::Config::default(),
		cancellation,
		|_| {},
	))?;
	if let Some(failure) = report.failure {
		return Err(failure.into());
	}

	Ok(0)
}

fn signal_cancellation() -> Result<Cancellation, ctrlc::Error> {
	let cancellation = Cancellation::new();
	let signal = cancellation.clone();
	ctrlc::set_handler(move || signal.cancel())?;
	Ok(cancellation)
}
