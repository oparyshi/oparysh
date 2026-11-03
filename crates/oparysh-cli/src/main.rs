use clap::Parser;
use oparysh_db::rebuild_db_from_backup;
use oparysh_verifier::verify_tarball;
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
    /// Восстановить базу данных из резервной копии db.sqlite.bak (stage 3.6)
    RebuildDb {
        /// Директория базы (по умолчанию /var/lib/oparysh)
        #[arg(long, default_value = "/var/lib/oparysh")]
        db_dir: PathBuf,
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
        Some(Command::RebuildDb { db_dir }) => {
            match rebuild_db_from_backup(&db_dir) {
                Ok(bak) => {
                    println!("База восстановлена из {}", bak.display());
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    std::process::exit(8);
                }
            }
        }
        None => {
            println!("oparysh placeholder");
        }
    }
}
