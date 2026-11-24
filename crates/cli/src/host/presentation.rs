//! Resolve the human-facing identity before rendering headings and Clap usage.
use clap::Command;
use clap_lex::OsStrExt;
use std::{ffi::OsString, io, path::Path};
use trine::{
	Application,
	cli::{completion, output_mode::json_requested},
};
use workyard_application::identity::Identity;

pub(super) fn load(app: &Application, args: &[OsString]) -> io::Result<Option<&'static Identity>> {
	if json_requested(args)
		|| completion::requested()
		|| args.get(1).is_some_and(|arg| arg == "rpc" || arg == "mcp")
	{
		return Ok(None);
	}

	if args.get(1).is_some_and(|arg| arg == "init")
		&& let Some((root, settings)) = init_settings(app, args)
	{
		return Identity::select(Some(&root), settings).map(Some);
	}
	Identity::select(selected_workspace(args), Identity::default()).map(Some)
}

// Use the same Clap projection as init to handle native paths and option=value.
fn init_settings(app: &Application, args: &[OsString]) -> Option<(std::path::PathBuf, Identity)> {
	let command = app.cli_command();
	let matches = command.clone().try_get_matches_from(args).ok()?;
	let init = matches.subcommand_matches("init")?;
	let spec = command.find_subcommand("init")?;
	let value = |long: Option<&str>| {
		let arg = spec.get_arguments().find(|arg| match long {
			Some(long) => arg.get_long() == Some(long),
			None => arg.is_positional(),
		})?;
		init.get_raw(arg.get_id().as_str())?.next()
	};

	let root = value(None).map(Path::new).unwrap_or(Path::new("."));
	let settings = Identity {
		display_name: value(Some("display-name"))
			.and_then(|v| v.to_str())
			.map(str::to_owned),
		invocation_prefix: value(Some("invocation-prefix"))
			.and_then(|v| v.to_str())
			.map(str::to_owned),
	};
	Some((root.to_owned(), settings))
}

// Read just the native workspace selector before Clap produces help or diagnostics.
fn selected_workspace(args: &[OsString]) -> Option<&Path> {
	let mut args = args.iter().skip(1).take_while(|word| *word != "--");
	while let Some(word) = args.next() {
		if word == "--workspace" {
			return args.next().map(Path::new);
		}
		if let Some(path) = word.as_os_str().strip_prefix("--workspace=") {
			return Some(Path::new(path));
		}
	}
	None
}

pub(super) fn configure_help(command: Command) -> Command {
	command
		.bin_name(Identity::prefix().to_owned())
		.mut_subcommands(|sub| {
			let init = sub.get_name() == "init";
			let sub = match sub.get_after_help() {
				Some(help) if Identity::managed() => {
					let examples = help
						.to_string()
						.replace("workyard ", &format!("{} ", Identity::prefix()));
					sub.after_help(examples)
				}
				_ => sub,
			};
			sub.mut_args(|arg| {
				let hidden = match arg.get_long() {
					Some("display-name" | "invocation-prefix") => init,
					Some("workspace") => Identity::managed(),
					_ => false,
				};
				if hidden { arg.hide(true) } else { arg }
			})
		})
}
