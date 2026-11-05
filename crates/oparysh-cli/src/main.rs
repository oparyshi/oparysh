use clap::{CommandFactory, Parser};
use oparysh_core::config::OparyshConfig;
use oparysh_core::config_edit;
use oparysh_db::{rebuild_db_from_backup, Database, Package};
use oparysh_repo::RepoError;
use oparysh_resolver::{plan::render_plan, RepoPackageSource, ResolveOptions};
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

/// Код возврата для ошибки resolver по DOCS §11.
fn resolve_exit_code(e: &oparysh_resolver::ResolveError) -> i32 {
    use oparysh_resolver::ResolveError;
    match e {
        ResolveError::Repo(e) => repo_exit_code(e),
        ResolveError::Unresolved { .. }
        | ResolveError::VersionConflict { .. }
        | ResolveError::Conflict { .. }
        | ResolveError::BuildCycle { .. } => 4,
        ResolveError::AmbiguousProvider { .. } => 2,
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

/// Разобрать `pkg[@ver]` в требование: `@` означает точную версию.
fn dep_req_from_spec(spec: &str) -> Result<oparysh_core::version::DepReq, String> {
    let (name, version) = split_pkg_spec(spec);
    let raw = match version {
        Some(version) => format!("{name}={version}"),
        None => name,
    };
    oparysh_core::version::DepReq::parse(&raw).map_err(|e| e.to_string())
}

/// Разрешить цели с интерактивным выбором неоднозначного поставщика.
/// Возвращает граф, источник и итоговые опции поставщика.
fn resolve_cli(
    specs: &[String],
    features: &[String],
    config: &OparyshConfig,
    assume_yes: bool,
    report_errors: bool,
) -> Result<(oparysh_resolver::RequirementSet, RepoPackageSource), i32> {
    let cache = cache_dir(config);
    let source = match RepoPackageSource::new(&config.repositories.entries, &cache) {
        Ok(source) => source,
        Err(e) => {
            if report_errors { eprintln!("ERROR: {e}"); }
            return Err(resolve_exit_code(&e));
        }
    };
    let targets = specs
        .iter()
        .map(|spec| dep_req_from_spec(spec))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            if report_errors { eprintln!("ERROR: {e}"); }
            2
        })?;

    let db = open_db(config);
    let mut provider = None;
    loop {
        let options = ResolveOptions {
            features: features.to_vec(),
            installed: Some(&db),
            provider: provider.clone(),
        };
        match oparysh_resolver::resolve(&targets, &source, &options) {
            Ok(set) => return Ok((set, source)),
            Err(oparysh_resolver::ResolveError::AmbiguousProvider {
                virtual_name,
                providers,
            }) => {
                if assume_yes || !report_errors {
                    if report_errors {
                        eprintln!("ERROR: неоднозначный поставщик виртуального пакета {virtual_name}: {}; выберите поставщика интерактивно", providers.join(", "));
                    }
                    return Err(2);
                }
                eprintln!("Выберите поставщика виртуального пакета {virtual_name}:");
                for (index, provider_name) in providers.iter().enumerate() {
                    eprintln!("  {}) {provider_name}", index + 1);
                }
                eprint!("Ваш выбор [1-{}]: ", providers.len());
                if std::io::Write::flush(&mut std::io::stderr()).is_err() {
                    return Err(1);
                }
                let mut choice = String::new();
                if std::io::stdin().read_line(&mut choice).is_err() {
                    return Err(1);
                }
                let index = choice.trim().parse::<usize>().ok().and_then(|n| n.checked_sub(1));
                let selected = index.and_then(|i| providers.get(i)).cloned();
                match selected {
                    Some(selected) => provider = Some(selected),
                    None => {
                        eprintln!("ERROR: некорректный выбор");
                        return Err(2);
                    }
                }
            }
            Err(e) => {
                if report_errors { eprintln!("ERROR: {e}"); }
                return Err(resolve_exit_code(&e));
            }
        }
    }
}

/// Известные размеры исходников для всех ожидающих сборки узлов.
fn source_sizes(
    set: &oparysh_resolver::RequirementSet,
    source: &RepoPackageSource,
) -> std::collections::BTreeMap<String, Option<u64>> {
    oparysh_resolver::pending_build_order(set)
        .into_iter()
        .map(|name| {
            let node = &set.nodes[&name];
            let candidate = oparysh_resolver::Candidate {
                name: node.name.clone(),
                version: node.version.clone(),
                version_str: node.version_str.clone(),
                build: node.build,
                repo: node.repo.clone(),
            };
            (name, source.source_size(&candidate))
        })
        .collect()
}

/// Показать план и запросить подтверждение. Возвращает true, если можно продолжать.
fn confirm_plan(
    plan: &str,
    assume_yes: bool,
) -> bool {
    println!("{plan}");
    if assume_yes {
        return true;
    }
    print!(" Proceed? [y/N] ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// `oparysh plan`: разрешить цели и показать план без изменений в системе.
fn run_plan(specs: &[String], features: &[String], assume_yes: bool) {
    let config = load_config(None);
    let (set, source) = match resolve_cli(specs, features, &config, assume_yes, true) {
        Ok(value) => value,
        Err(code) => std::process::exit(code),
    };
    let plan = render_plan(
        &set,
        config.general.jobs,
        &source_sizes(&set, &source),
    );
    if !confirm_plan(&plan, assume_yes) {
        println!("Отменено; изменения не внесены");
        return;
    }
    println!("План подтверждён; установка будет выполнена командами этапов 6–8");
}

/// Рекурсивная печать дочерних узлов с защитой от runtime-циклов.
/// `prefix` — отступ, соответствующий уровню `parent`.
fn print_dep_children(
    set: &oparysh_resolver::RequirementSet,
    parent: &str,
    prefix: &str,
    stack: &mut Vec<String>,
) {
    let Some(node) = set.nodes.get(parent) else { return };
    for (index, dep) in node.dependencies.iter().enumerate() {
        let is_last = index + 1 == node.dependencies.len();
        let branch = if is_last { "└─ " } else { "├─ " };
        let Some(dep_node) = set.nodes.get(dep) else {
            println!("{prefix}{branch}{dep}");
            continue;
        };
        let marker = if dep_node.installed { " (installed)" } else { "" };
        let virtual_marker = dep_node
            .provided_for
            .as_deref()
            .map(|v| format!(" provides {v}"))
            .unwrap_or_default();
        println!("{prefix}{branch}{} {}{}{}", dep_node.name, dep_node.version_str, marker, virtual_marker);
        let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
        if stack.iter().any(|n| n == dep) {
            println!("{child_prefix}(cycle)");
            continue;
        }
        stack.push(dep.clone());
        print_dep_children(set, dep, &child_prefix, stack);
        stack.pop();
    }
}

/// Печать корня и всего дерева зависимостей.
fn print_dep_tree(set: &oparysh_resolver::RequirementSet, root: &str) {
    let Some(node) = set.nodes.get(root) else { return };
    let marker = if node.installed { " (installed)" } else { "" };
    let virtual_marker = node
        .provided_for
        .as_deref()
        .map(|v| format!(" provides {v}"))
        .unwrap_or_default();
    println!("{} {}{}{}", node.name, node.version_str, marker, virtual_marker);
    print_dep_children(set, root, "", &mut vec![root.to_string()]);
}

/// Обратные зависимости среди установленных пакетов.
fn reverse_deps_installed(db: &Database, target: &str) -> oparysh_db::DbResult<Vec<(String, String)>> {
    let mut result = Vec::new();
    for pkg in db.list_packages()? {
        for dep in db.get_deps(&pkg.name)? {
            if dep.dep_name == target {
                result.push((pkg.name.clone(), dep.req));
            }
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

/// Транзитивные обратные зависимости среди установленных пакетов.
fn reverse_deps_closure(db: &Database, target: &str) -> oparysh_db::DbResult<Vec<String>> {
    let mut result = Vec::new();
    let mut stack = vec![target.to_string()];
    let mut seen = HashSet::new();
    while let Some(current) = stack.pop() {
        for (dependent, _) in reverse_deps_installed(db, &current)? {
            if seen.insert(dependent.clone()) {
                result.push(dependent.clone());
                stack.push(dependent);
            }
        }
    }
    result.sort();
    Ok(result)
}

/// `oparysh deps [--tree] <pkg>`: показать зависимости пакета.
fn run_deps(spec: &str, tree: bool) {
    let config = load_config(None);
    let specs = [spec.to_string()];
    match resolve_cli(&specs, &[], &config, false, false) {
        Ok((set, _)) => {
            if tree {
                let root = set
                    .nodes
                    .iter()
                    .find(|(_, n)| n.is_target)
                    .map(|(name, _)| name.clone());
                if let Some(root) = root {
                    print_dep_tree(&set, &root);
                }
            } else {
                println!("{name:<24} {version:<12} {kind:<10} REQUIRED BY", name="ИМЯ", version="ВЕРСИЯ", kind="ТИП");
                for node in set.nodes.values().filter(|n| !n.is_target) {
                    let kind = match (node.as_build, node.as_run) {
                        (true, true) => "build/run",
                        (true, false) => "build",
                        (false, true) => "run",
                        _ => "dependency",
                    };
                    let required_by = node.required_by.iter().filter_map(|(p, _)| p.as_deref()).collect::<Vec<_>>().join(", ");
                    println!("{:<24} {:<12} {:<10} {}", node.name, node.version_str, kind, required_by);
                }
            }
        }
        Err(_) => {
            // Репозиториев может не быть/пакета нет в индексе — покажем установленную БД.
            let db = open_db(&config);
            let installed = db.get_package(&split_pkg_spec(spec).0).ok().flatten();
            let Some(pkg) = installed else {
                std::process::exit(3);
            };
            println!("{} {} (installed; зависимости из БД)", pkg.name, pkg.version);
            match db.get_deps(&pkg.name) {
                Ok(deps) => {
                    if tree {
                        for dep in deps { println!("  {} {}", dep.dep_name, dep.req); }
                    } else {
                        for dep in deps { println!("{:<24} {:<12} {}", dep.dep_name, "", dep.req); }
                    }
                }
                Err(e) => { eprintln!("ERROR: {e}"); std::process::exit(8); }
            }
        }
    }
}

/// `oparysh rdeps <pkg>`: обратные зависимости среди установленных пакетов.
fn run_rdeps(spec: &str) {
    let config = load_config(None);
    let db = open_db(&config);
    let (name, _) = split_pkg_spec(spec);
    match reverse_deps_installed(&db, &name) {
        Ok(direct) => {
            if direct.is_empty() {
                println!("нет установленных пакетов, зависящих от {name}");
                return;
            }
            println!("{name:<24} {req:<20} ЗАВИСИТ", name="ПАКЕТ", req="ТРЕБОВАНИЕ");
            for (pkg, req) in direct {
                println!("{:<24} {:<20} {}", pkg, req, name);
            }
        }
        Err(e) => { eprintln!("ERROR: {e}"); std::process::exit(8); }
    }
}

/// `oparysh why <pkg>`: объяснить, зачем пакет установлен.
fn run_why(spec: &str) {
    let config = load_config(None);
    let db = open_db(&config);
    let (name, _) = split_pkg_spec(spec);
    let pkg = match db.get_package(&name) {
        Ok(Some(pkg)) => pkg,
        Ok(None) => {
            eprintln!("ERROR: пакет {name} не установлен");
            std::process::exit(3);
        }
        Err(e) => { eprintln!("ERROR: {e}"); std::process::exit(8); }
    };
    println!("{} {} установлен", pkg.name, pkg.version);
    match reverse_deps_closure(&db, &name) {
        Ok(users) if !users.is_empty() => {
            println!("Нужен установленным пакетам:");
            for user in users {
                println!("  - {user}");
            }
        }
        Ok(_) => {
            println!("Ни один установленный пакет от него не зависит; вероятно, он установлен явно.");
        }
        Err(e) => { eprintln!("ERROR: {e}"); std::process::exit(8); }
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
    /// Не спрашивать подтверждение у команд, которые его поддерживают
    #[arg(long, global = true)]
    yes: bool,
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
    /// Показать план разрешения зависимостей и запросить подтверждение (stage 5.7)
    Plan {
        /// Цели: `pkg`, `pkg@ver`, необязательные зависимости через `+feature`
        #[arg(value_name = "PKG", required = true)]
        specs: Vec<String>,
    },
    /// Дерево или список зависимостей пакета (stage 5.8)
    Deps {
        /// Печатать дерево вместо плоского списка
        #[arg(long)]
        tree: bool,
        /// Пакет в виде `pkg` или `pkg@ver`
        pkg: String,
    },
    /// Обратные зависимости среди установленных пакетов (stage 5.8)
    Rdeps {
        /// Пакет в виде `pkg` или `pkg@ver`
        pkg: String,
    },
    /// Объяснить, зачем установлен пакет (stage 5.8)
    Why {
        /// Пакет в виде `pkg` или `pkg@ver`
        pkg: String,
    },
    /// Управление репозиториями в конфиге (stage 4.5)
    Repo {
        #[command(subcommand)]
        action: RepoCommand,
    },
    /// Очистить кэши исходников/сборки/логов (stage 6.6)
    #[command(group = clap::ArgGroup::new("clean_scope").required(true).multiple(false)
        .args(["src", "build", "logs", "all"]))]
    Clean {
        /// Очистить кэш исходников <db_dir>/src/
        #[arg(long)]
        src: bool,
        /// Очистить рабочие каталоги сборки <db_dir>/build/
        #[arg(long)]
        build: bool,
        /// Очистить логи сборок <db_dir>/logs/
        #[arg(long)]
        logs: bool,
        /// Очистить всё: src + build + logs
        #[arg(long)]
        all: bool,
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

/// Отделить фичи `+name` от целей в аргументах `oparysh plan`.
fn split_plan_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    let mut specs = Vec::new();
    let mut features = Vec::new();
    for arg in args {
        if let Some(feature) = arg.strip_prefix('+') {
            if !feature.is_empty() {
                features.push(feature.to_string());
            }
        } else {
            specs.push(arg.clone());
        }
    }
    (specs, features)
}

/// `oparysh clean` (stage 6.6): удалить выбранные подкаталоги базы,
/// напечатав освобождённый объём. Ошибка удаления — общая ошибка **1**.
fn run_clean(db: &Path, src: bool, build: bool, logs: bool) {
    for (flag, dir) in [
        ("src", db.join("src")),
        ("build", db.join("build")),
        ("logs", db.join("logs")),
    ] {
        let selected = match flag {
            "src" => src,
            "build" => build,
            _ => logs,
        };
        if !selected || !dir.exists() {
            continue;
        }
        let bytes = dir_size(&dir).unwrap_or(0);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => println!("clean {flag:<5} {} — освобождено {} байт", dir.display(), bytes),
            Err(e) => {
                eprintln!("ERROR: не удалось удалить {}: {e}", dir.display());
                std::process::exit(1);
            }
        }
    }
}

/// Суммарный размер файлов каталога (рекурсивно).
fn dir_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += dir_size(&entry.path())?;
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

fn main() {
    let cli = Cli::parse();
    let assume_yes = cli.yes;
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
        Some(Command::Plan { specs }) => {
            let (specs, features) = split_plan_args(&specs);
            if specs.is_empty() {
                eprintln!("ERROR: укажите хотя бы один пакет");
                std::process::exit(2);
            }
            run_plan(&specs, &features, assume_yes);
        }
        Some(Command::Deps { tree, pkg }) => run_deps(&pkg, tree),
        Some(Command::Rdeps { pkg }) => run_rdeps(&pkg),
        Some(Command::Why { pkg }) => run_why(&pkg),
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
        Some(Command::Clean { src, build, logs, all }) => {
            run_clean(&db_dir(&load_config(None)), src || all, build || all, logs || all);
        }
        None => {
            let mut cmd = Cli::command();
            cmd.print_help().unwrap();
            std::process::exit(0);
        }
    }
}
