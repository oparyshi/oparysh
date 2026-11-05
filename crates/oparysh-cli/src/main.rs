use clap::{CommandFactory, Parser};
use oparysh_core::config::OparyshConfig;
use oparysh_db::rebuild_db_from_backup;
use oparysh_repo::RepoError;
use oparysh_verifier::verify_tarball;
use std::path::{Path, PathBuf};

/// Код возврата для ошибки oparysh-repo по DOCS §11.
fn repo_exit_code(e: &RepoError) -> i32 {
    match e {
        RepoError::Network { .. } | RepoError::BadRepoName { .. } => 5,
        RepoError::RecipeShaMismatch { .. } | RepoError::SourceShaMismatch { .. } => 6,
        RepoError::PkgNotFound { .. }
        | RepoError::NameNotFound { .. }
        | RepoError::RepoNotFound { .. } => 3,
        _ => 1,
    }
}

/// Прочитать конфиг: файл из `OPARYSH_CONFIG` или `/etc/oparysh/oparysh.conf`;
/// отсутствие файла — это дефолтный конфиг (для sync это норма, репозитории
/// настраиваются в нём же).
fn load_config(explicit: Option<&Path>) -> OparyshConfig {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("OPARYSH_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/etc/oparysh/oparysh.conf"));
    OparyshConfig::parse(&std::fs::read_to_string(&path).unwrap_or_default())
        .expect("не удалось разобрать oparysh.conf")
}

/// Каталог кэша: `OPARYSH_CACHE_DIR` перекрывает конфиг (DOCS §12).
fn cache_dir(config: &OparyshConfig) -> PathBuf {
    if let Some(dir) = std::env::var_os("OPARYSH_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(&config.general.cache_dir)
}

/// `oparysh sync [repo…]`: скачать/обновить index.toml репозиториев из конфига.
fn run_sync(repos: &[String]) -> oparysh_repo::RepoResult<Vec<oparysh_repo::SyncOutcome>> {
    let config = load_config(None);
    oparysh_repo::sync_repos(&config.repositories.entries, &cache_dir(&config), repos)
}

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
    /// Скачать/обновить index.toml всех (или указанных) репозиториев (stage 4.4)
    Sync {
        /// Ограничить синхронизацию перечисленными репозиториями
        #[arg(value_name = "REPO")]
        repos: Vec<String>,
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
        Some(Command::Sync { repos }) => match run_sync(&repos) {
            Ok(outcomes) => {
                for o in &outcomes {
                    match o.kind {
                        oparysh_repo::SyncKind::Local => println!(
                            "local  {} — {} пакетов ({})",
                            o.name,
                            o.packages,
                            o.index_path.display()
                        ),
                        oparysh_repo::SyncKind::Network => println!(
                            "synced {} — {} пакетов ({})",
                            o.name,
                            o.packages,
                            o.index_path.display()
                        ),
                    }
                }
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(repo_exit_code(&e));
            }
        },
        None => {
            let mut cmd = Cli::command();
            cmd.print_help().unwrap();
            std::process::exit(0);
        }
    }
}
