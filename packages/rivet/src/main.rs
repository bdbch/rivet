fn main() {
  let args: Vec<String> = std::env::args().collect();
  match rivet::run_with_args(&args) {
    Ok(rivet::CmdResult::Ok) => {}
    Ok(rivet::CmdResult::CheckStatus(status)) => {
      // Map check status to exit codes for CI scripting:
      //   0 = nothing pending or ready (success)
      //   1 = ready to publish (trigger downstream)
      match status {
        rivet::CheckStatus::PendingReleases => std::process::exit(0),
        rivet::CheckStatus::ReadyToRelease => std::process::exit(1),
        rivet::CheckStatus::NothingToRelease => std::process::exit(0),
      }
    }
    Err(e) => {
      // run_with_args uses Cli::try_parse_from which converts clap errors
      // (including --help, --version, and invalid args) into RivetError::Cli.
      // Those propagate here as Err; internal errors arrive the same way.
      eprintln!("Error: {:#}", e);
      std::process::exit(1);
    }
  }
}
