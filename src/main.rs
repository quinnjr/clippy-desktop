// Release builds on Windows run as a GUI program so double-clicking the
// executable does not open a console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use clippy_app::options::{self, Command, USAGE};

fn main() {
    match options::parse(std::env::args().skip(1)) {
        Ok(Command::Help) => println!("{USAGE}"),
        Ok(Command::Version) => println!("clippy-app {}", env!("CARGO_PKG_VERSION")),
        Ok(Command::Run(options)) => {
            if let Err(err) = clippy_app::run(options) {
                eprintln!("clippy-app: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("clippy-app: {err}\n\n{USAGE}");
            std::process::exit(2);
        }
    }
}
