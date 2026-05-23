fn main() {
  let args: Vec<String> = std::env::args().collect();
  match oxrls::run_with_args(&args) {
    Ok(oxrls::CmdResult::Ok) => {}
    Ok(oxrls::CmdResult::CheckStatus(status)) => {
      // Map check status to exit codes for CI scripting:
      //   0 = nothing pending or ready (success)
      //   1 = ready to publish (trigger downstream)
      match status {
        oxrls::CheckStatus::PendingReleases => std::process::exit(0),
        oxrls::CheckStatus::ReadyToRelease => std::process::exit(1),
        oxrls::CheckStatus::NothingToRelease => std::process::exit(0),
      }
    }
    Err(e) => {
      // run_with_args uses Cli::try_parse_from which converts clap errors
      // (including --help, --version, and invalid args) into OxrlsError::Cli.
      // Those propagate here as Err; internal errors arrive the same way.
      eprintln!("Error: {:#}", e);
      std::process::exit(1);
    }
  }
}
