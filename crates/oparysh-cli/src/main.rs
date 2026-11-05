use clap::{CommandFactory, Parser};
use oparysh_core::config::OparyshConfig;
use oparysh_core::config_edit;
use oparysh_db::{rebuild_db_from_backup, Database, Package};
use oparysh_repo::RepoError;
use oparysh_verifier::verify_tarball;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Код возврата для ошибки oparysh-repo по DOCS §11.
fn repo_exit_code(e: &RepoError) -> i32 {
    match e {
        RepoError::Network { .. } | RepoError::BadRepoName { .. } => 5,
        RepoError::RecipeShaMismatch { .. } | RepoError::SourceShaMismatch { .. } => 6,
        RepoError::BadPattern { .. } => 2,
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

/// Каталог базы: `OPARYSH_DB_DIR` перекрывает конфиг (DOCS §12).
fn db_dir(config: &OparyshConfig) -> PathBuf {
    if let Some(dir) = std::env::var_os("OPARYSH_DB_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(&config.general.db_dir)
}

/// Открыть локальную базу; ошибка — код возврата **8** (DOCS §11).
fn open_db(config: &OparyshConfig) -> Database {
    match Database::open(&db_dir(config)) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(8);
        }
    }
}

/// Разобрать `pkg[@ver]` на имя и опциональную версию.
fn split_pkg_spec(spec: &str) -> (String, Option<String>) {
    match spec.rsplit_once('@') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => {
            (name.to_string(), Some(version.to_string()))
        }
        _ => (spec.to_string(), None),
    }
}

/// Обратные зависимости: установленные пакеты, на которые никто не ссылается.
///
/// Это кандидаты в «сироты» (`list --orphaned`). Установщик (этап 8) ещё не
/// пишет причину установки, поэтому пакеты, поставленные явно, пока тоже
/// попадают сюда; учёт «установлен как зависимость» — этап 10.4.
fn orphaned_packages(db: &Database) -> oparysh_db::DbResult<Vec<Package>> {
    let pkgs = db.list_packages()?;
    let mut needed: HashSet<String> = HashSet::new();
    for p in &pkgs {
        for dep in db.get_deps(&p.name)? {
            needed.insert(dep.dep_name);
        }
    }
    Ok(pkgs
        .into_iter()
        .filter(|p| !needed.contains(&p.name))
        .collect())
}

/// Предупреждения о пропущенных репозиториях — в stderr.
fn print_skipped(skipped: &[(String, String)]) {
    for (name, reason) in skipped {
        eprintln!("WARN: репозиторий «{name}» пропущен: {reason}");
    }
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

/// `oparysh search <pattern>`: поиск по имени/описанию/тегам во всех репозиториях.
fn run_search(pattern: &str) -> oparysh_repo::RepoResult<oparysh_repo::SearchOutcome> {
    let config = load_config(None);
    let pattern = oparysh_repo::SearchPattern::parse(pattern)?;
    oparysh_repo::search_repos(&config.repositories.entries, &cache_dir(&config), &pattern)
}

/// `oparysh info <pkg>[@ver]`: метаданные пакета из репозиториев.
fn run_info(spec: &str) -> oparysh_repo::RepoResult<oparysh_repo::InfoOutcome> {
    let config = load_config(None);
    let (name, version) = split_pkg_spec(spec);
    oparysh_repo::info_repos(
        &config.repositories.entries,
        &cache_dir(&config),
        &name,
        version.as_deref(),
    )
}

/// Напечатать метаданные пакета для `oparysh info`.
fn print_info(info: &oparysh_repo::InfoOutcome) {
    let p = &info.pkg;
    let field = |label: &str, value: &str| {
        if !value.is_empty() {
            println!("{label:<13} {value}");
        }
    };
    println!("{:<13} {} {} (build {})", "ПАКЕТ", p.name, p.version, p.build);
    field("РЕПОЗИТОРИЙ", &info.repo);
    field("АРХИТЕКТУРА", &p.arch.join(", "));
    field("ОПИСАНИЕ", &p.desc);
    field("ТЕГИ", &p.tags.join(", "));
    field("ЗАВИСИМОСТИ", &p.deps.join(", "));
    field("PROVIDES", &p.provides.join(", "));
    field("CONFLICTS", &p.conflicts.join(", "));
    field("RECIPE_SHA", &p.recipe_sha);
    field("SRC_SHA256", &p.src_sha256);
}

/// `oparysh list`: установленные и/или доступные пакеты (этап 4.7).
fn run_list(installed: bool, available: bool, outdated: bool, orphaned: bool) {
    let config = load_config(None);

    if available {
        list_available(&config);
    } else if outdated {
        list_outdated(&config);
    } else if orphaned {
        list_orphaned(&config);
    } else {
        // Флаг `installed` и отсутствие флагов — установленные пакеты.
        let _ = installed;
        list_installed(&config);
    }
}

/// Печать одной строки пакета в формате «ИМЯ ВЕРСИЯ …».
fn print_package_row(p: &Package) {
    println!(
        "{:<24} {:<12} {:>5} {:<10} {:>12}  {}",
        p.name,
        p.version,
        p.build_rev,
        p.arch,
        p.size_bytes,
        format_date_utc(p.install_ts.max(0) as u64),
    );
}

/// `oparysh list --installed`: пакеты из локальной базы.
fn list_installed(config: &OparyshConfig) {
    let db = open_db(config);
    let pkgs = match db.list_packages() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(8);
        }
    };
    if pkgs.is_empty() {
        println!("нет установленных пакетов");
        return;
    }
    println!(
        "{name:<24} {version:<12} {build:>5} {arch:<10} {size:>12}  {date}",
        name = "ИМЯ", version = "ВЕРСИЯ", build = "BUILD", arch = "АРХ",
        size = "РАЗМЕР", date = "УСТАНОВЛЕН"
    );
    for p in &pkgs {
        print_package_row(p);
    }
}

/// `oparysh list --available`: записи `[[pkg]]` всех репозиториев.
fn list_available(config: &OparyshConfig) {
    match oparysh_repo::list_available(&config.repositories.entries, &cache_dir(config)) {
        Ok(out) => {
            print_skipped(&out.skipped);
            if out.entries.is_empty() {
                println!("нет доступных пакетов");
                return;
            }
            println!(
                "{name:<24} {version:<12} {repo:<16} ОПИСАНИЕ",
                name = "ИМЯ", version = "ВЕРСИЯ", repo = "РЕПО"
            );
            for e in &out.entries {
                println!(
                    "{:<24} {:<12} {:<16} {}",
                    e.pkg.name, e.pkg.version, e.repo, e.pkg.desc
                );
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(repo_exit_code(&e));
        }
    }
}

/// `oparysh list --outdated`: установленные пакеты с более свежей версией в репо.
fn list_outdated(config: &OparyshConfig) {
    let db = open_db(config);
    let pkgs = match db.list_packages() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(8);
        }
    };
    let installed: Vec<(String, String)> =
        pkgs.iter().map(|p| (p.name.clone(), p.version.clone())).collect();

    match oparysh_repo::outdated_repos(
        &config.repositories.entries,
        &cache_dir(config),
        &installed,
    ) {
        Ok(out) => {
            print_skipped(&out.skipped);
            if out.entries.is_empty() {
                println!("все установленные пакеты актуальны");
                return;
            }
            println!(
                "{name:<24} {installed:<12} {available:<12} {repo:<16}",
                name = "ИМЯ", installed = "УСТАНОВЛЕНА", available = "ДОСТУПНА", repo = "РЕПО"
            );
            for e in &out.entries {
                println!(
                    "{:<24} {:<12} {:<12} {:<16}",
                    e.name, e.installed, e.available, e.repo
                );
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(repo_exit_code(&e));
        }
    }
}

/// `oparysh list --orphaned`: установленные пакеты, на которые никто не ссылается.
fn list_orphaned(config: &OparyshConfig) {
    let db = open_db(config);
    let pkgs = match orphaned_packages(&db) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(8);
        }
    };
    if pkgs.is_empty() {
        println!("сирот не найдено");
        return;
    }
    println!(
        "{name:<24} {version:<12}",
        name = "ИМЯ", version = "ВЕРСИЯ"
    );
    for p in &pkgs {
        println!("{:<24} {:<12}", p.name, p.version);
    }
    println!();
    println!("(ни один установленный пакет от них не зависит)");
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
    /// Поиск по имени/описанию/тегам (glob или /regex/) (stage 4.6)
    Search {
        /// Шаблон: glob (`gcc*`, `*-git`) или регулярное выражение в слэшах (`/^lib.*/`)
        pattern: String,
    },
    /// Показать метаданные пакета: зависимости, provides, версию, репозиторий (stage 4.7)
    Info {
        /// Пакет в виде `pkg` или `pkg@ver`
        pkg: String,
    },
    /// Список установленных/доступных пакетов (stage 4.7)
    #[command(group = clap::ArgGroup::new("list_scope").multiple(false)
        .args(["installed", "available", "outdated", "orphaned"]))]
    List {
        /// Только установленные пакеты (по умолчанию)
        #[arg(long)]
        installed: bool,
        /// Все пакеты из репозиториев
        #[arg(long)]
        available: bool,
        /// Установленные пакеты с более свежей версией в репозитории
        #[arg(long)]
        outdated: bool,
        /// Установленные пакеты, от которых никто не зависит
        #[arg(long)]
        orphaned: bool,
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
        Some(Command::Search { pattern }) => match run_search(&pattern) {
            Ok(outcome) => {
                for (name, reason) in &outcome.skipped {
                    eprintln!("WARN: репозиторий «{name}» пропущен: {reason}");
                }
                if outcome.hits.is_empty() {
                    println!("ничего не найдено по шаблону «{pattern}»");
                } else {
                    println!("{name:<24} {version:<12} {repo:<16} ОПИСАНИЕ",
                        name = "ИМЯ", version = "ВЕРСИЯ", repo = "РЕПО");
                    for h in &outcome.hits {
                        println!("{:<24} {:<12} {:<16} {}", h.name, h.version, h.repo, h.desc);
                    }
                }
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(repo_exit_code(&e));
            }
        },
        Some(Command::Info { pkg }) => match run_info(&pkg) {
            Ok(info) => {
                print_skipped(&info.skipped);
                print_info(&info);
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(repo_exit_code(&e));
            }
        },
        Some(Command::List {
            installed,
            available,
            outdated,
            orphaned,
        }) => run_list(installed, available, outdated, orphaned),
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
