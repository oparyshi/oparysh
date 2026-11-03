use clap::Parser;
use oparysh_verifier::{verify_tarball, VerifierError};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "oparysh", version, about = "Source-based package manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Verify a tarball's SHA-256 (stage 2)
    Verify {
        /// Путь к тарболу для проверки
        tarball: PathBuf,
        /// Ожидаемый SHA-256 (hex). Если не указан — вычисляется и печатается.
        #[arg(short, long)]
        expected: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Verify { tarball, expected }) => {
            if let Some(expected) = expected {
                match verify_tarball(Some(&expected), &tarball) {
                    Ok(()) => {
                        println!("OK: {} — sha256 совпадает", tarball.display());
                    }
                    Err(e) => {
                        eprintln!("ERROR: {e}");
                        std::process::exit(6);
                    }
                }
            } else {
                match oparysh_verifier::sha256_file(&tarball) {
                    Ok(hash) => {
                        println!("{hash}  {}", tarball.display());
                    }
                    Err(e) => {
                        eprintln!("ERROR: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        None => {
            println!("oparysh placeholder");
        }
    }
}
