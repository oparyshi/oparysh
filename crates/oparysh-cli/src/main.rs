use clap::{CommandFactory, Parser};
use oparysh_core::config::OparyshConfig;
use oparysh_core::config_edit;
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

/// Путь к конфигу: `OPARYSH_CONFIG` или `/etc/oparysh/oparysh.conf`.
fn config_path(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("OPARYSH_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/etc/oparysh/oparysh.conf"))
}

/// Прочитать конфиг: файл из `OPARYSH_CONFIG` или `/etc/oparysh/oparysh.conf`;
/// отсутствие файла — это дефолтный конфиг (для sync это норма, репозитории
/// настраиваются в нём же).
fn load_config(explicit: Option<&Path>) -> OparyshConfig {
    let path = config_path(explicit);
    // Коды возврата (DOCS §11): нечитаемый/битый конфиг — общая ошибка 1.
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    match OparyshConfig::parse(&text) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("ERROR: не удалось разобрать {}: {e}", path.display());
            std::process::exit(1);
        }
    }
}

/// Прочитать сырой текст конфига (для редактирования `repo add/del`);
/// отсутствующий файл трактуется как пустой.
fn load_config_text() -> (PathBuf, String) {
    let path = config_path(None);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    (path, text)
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

/// Форматировать unix-время как дату UTC (`YYYY-MM-DD`) без внешних зависимостей.
fn format_date_utc(ts: u64) -> String {
    if ts == 0 {
        return "-".to_string();
    }
    let days = (ts / 86_400) as i64;
    // Civil-from-days (Howard Hinnant), эпоха 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 1_469_096 } / 1_460_097;
    let doe = z - era * 1_460_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// `oparysh repo add <name> <url|path>`: добавить репозиторий в конфиг.
fn run_repo_add(name: &str, target: &str) -> Result<String, String> {
    let (path, text) = load_config_text();
    let mut entry = oparysh_core::config::RepoEntry {
        name: name.to_string(),
        path: None,
        url: None,
        key: None,
    };
    // URL содержит схему (`://`); всё остальное — путь к локальному каталогу.
    if target.contains("://") {
        entry.url = Some(target.to_string());
    } else {
        entry.path = Some(target.to_string());
    }
    let new_text = config_edit::add_repository(&text, entry).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&path, &new_text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(format!("репозиторий «{name}» добавлен в {}", path.display()))
}

/// `oparysh repo del <name>`: убрать репозиторий из конфига.
fn run_repo_del(name: &str) -> Result<String, String> {
    let (path, text) = load_config_text();
    let new_text = config_edit::remove_repository(&text, name).map_err(|e| e.to_string())?;
    std::fs::write(&path, &new_text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(format!("репозиторий «{name}» убран из {}", path.display()))
}

/// Сведения об одном репозитории для `repo list`: (источник, пакетов, обновлён).
fn repo_status(
    entry: &oparysh_core::config::RepoEntry,
    cache_dir: &Path,
) -> (String, String, String) {
    match oparysh_repo::RepoSource::from_entry(entry) {
        Ok(oparysh_repo::RepoSource::Local(root)) => match oparysh_repo::LocalRepo::open(&root) {
            Ok(repo) => (
                root.display().to_string(),
                repo.packages().len().to_string(),
                format_date_utc(repo.index().repo.updated),
            ),
            Err(_) => (root.display().to_string(), "-".to_string(), "ошибка чтения".to_string()),
        },
        Ok(oparysh_repo::RepoSource::Network(url)) => {
            match oparysh_repo::CachedRepo::open(cache_dir, &entry.name) {
                Ok(cached) => (
                    url,
                    cached.packages().len().to_string(),
                    format_date_utc(cached.index.repo.updated),
                ),
                Err(_) => (url, "-".to_string(), "не синхронизирован".to_string()),
            }
        }
        Err(e) => (format!("(невалидная запись: {e})"), "-".to_string(), "-".to_string()),
    }
}

/// `oparysh repo list`: показать настроенные репозитории.
fn run_repo_list() {
    let config = load_config(None);
    let cache = cache_dir(&config);
    let entries = &config.repositories.entries;
    if entries.is_empty() {
        println!("репозиториев не настроено; добавьте: oparysh repo add <name> <url|path>");
        return;
    }
    println!("{name:<16} {source:<48} {pkgs:>8}  {updated}",
        name = "ИМЯ", source = "ИСТОЧНИК", pkgs = "ПАКЕТОВ", updated = "ОБНОВЛЁН");
    for entry in entries {
        let (source, pkgs, updated) = repo_status(entry, &cache);
        println!("{:<16} {:<48} {:>8}  {}", entry.name, source, pkgs, updated);
    }
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
    /// Управление репозиториями в конфиге (stage 4.5)
    Repo {
        #[command(subcommand)]
        action: RepoCommand,
    },
}

#[derive(clap::Subcommand)]
enum RepoCommand {
    /// Добавить репозиторий в конфиг (url или путь к локальному каталогу)
    Add {
        /// Имя репозитория
        name: String,
        /// URL (https://…) или путь к локальному каталогу-репозиторию
        target: String,
    },
    /// Показать репозитории, кол-во пакетов, дату обновления
    List,
    /// Убрать репозиторий из конфига
    Del {
        /// Имя репозитория
        name: String,
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
        Some(Command::Repo { action }) => match action {
            RepoCommand::Add { name, target } => match run_repo_add(&name, &target) {
                Ok(msg) => println!("{msg}"),
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    std::process::exit(1);
                }
            },
            RepoCommand::List => run_repo_list(),
            RepoCommand::Del { name } => match run_repo_del(&name) {
                Ok(msg) => println!("{msg}"),
                Err(e) => {
                    let code = if e.contains("не найден") { 3 } else { 1 };
                    eprintln!("ERROR: {e}");
                    std::process::exit(code);
                }
            },
        },
        None => {
            let mut cmd = Cli::command();
            cmd.print_help().unwrap();
            std::process::exit(0);
        }
    }
}
