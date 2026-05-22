fn main() {
  let args: Vec<String> = std::env::args().collect();
  match oxrls::run_with_args(&args) {
    Ok(()) => {}
    Err(e) => {
      // run_with_args uses Cli::parse_from which handles --help, --version,
      // and invalid args by printing the message and calling process::exit
      // with the appropriate code. We only reach here for internal errors.
      eprintln!("Error: {:#}", e);
      std::process::exit(1);
    }
  }
}
