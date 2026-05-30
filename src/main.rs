use std::io::{self, Read};

use boundline_adapter_speckit::{
    EmitHookRequest, ExecuteStageRequest, PreflightRequest, SpeckitProfileError,
    bootstrap_status_line, describe_response, emit_hook_response, execute_stage_response,
    preflight_response, success_envelope,
};
use clap::{Parser, Subcommand};
use thiserror::Error;

/// CLI entrypoint for the known Speckit workflow bridge.
#[derive(Debug, Parser)]
#[command(name = "boundline-adapter-speckit")]
#[command(about = "Known Speckit workflow bridge for Boundline")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// Supported Speckit workflow-bridge subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the Speckit capability manifest.
    Describe,
    /// Validate and normalize Speckit config.
    Preflight,
    /// Execute one claimed stage through the Speckit workflow bridge.
    ExecuteStage,
    /// Observe one host hook through the Speckit workflow bridge.
    EmitHook,
}

/// Terminal errors surfaced by the Speckit workflow-bridge binary.
#[derive(Debug, Error)]
enum CliError {
    #[error("failed to read stdin: {0}")]
    ReadStdin(#[source] io::Error),
    #[error("failed to parse stdin JSON: {0}")]
    ParseJson(#[source] serde_json::Error),
    #[error("failed to write stdout JSON: {0}")]
    WriteJson(#[source] serde_json::Error),
    #[error(transparent)]
    Profile(#[from] SpeckitProfileError),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Describe) => write_json(&success_envelope(describe_response())),
        Some(Command::Preflight) => {
            let request = read_json_from_stdin::<PreflightRequest>()?;
            write_json(&success_envelope(preflight_response(&request)))
        }
        Some(Command::ExecuteStage) => {
            let request = read_json_from_stdin::<ExecuteStageRequest>()?;
            let response = execute_stage_response(&request)?;
            write_json(&success_envelope(response))
        }
        Some(Command::EmitHook) => {
            let request = read_json_from_stdin::<EmitHookRequest>()?;
            write_json(&success_envelope(emit_hook_response(&request)))
        }
        None => {
            println!("{}", bootstrap_status_line());
            Ok(())
        }
    }
}

fn read_json_from_stdin<T>() -> Result<T, CliError>
where
    T: serde::de::DeserializeOwned,
{
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(CliError::ReadStdin)?;
    serde_json::from_str(&input).map_err(CliError::ParseJson)
}

fn write_json<T>(response: &T) -> Result<(), CliError>
where
    T: serde::Serialize,
{
    serde_json::to_writer_pretty(io::stdout(), response).map_err(CliError::WriteJson)
}
