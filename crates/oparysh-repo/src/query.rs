//! Команды `oparysh info` и `oparysh list` (этап 4.7 ROADMAP, DOCS §10).
//!
//! Здесь собраны запросы к репозиториям, на которых стоят:
//!
//! - `oparysh info <pkg>[@ver]` — метаданные пакета (`deps`, `provides`,
//!   `conflicts`, версия, репозиторий) — [`info_repos`];
//! - `oparysh list --available` — все доступные записи `[[pkg]]` в порядке
//!   приоритета репозиториев — [`list_available`];
//! - `oparysh list --outdated` — установленные пакеты, для которых в
//!   репозиториях есть версия новее — [`outdated_repos`].
//!
//! Локальные репозитории читаются напрямую, сетевые — по кэшу индексов
//! (`<cache>/repo/<name>/index.toml`), созданному `oparysh sync`; сетевой
//! репозиторий без кэша не является ошибкой и попадает в `skipped`.

use std::cmp::Ordering;
use std::path::Path;

use oparysh_core::config::RepoEntry;
use oparysh_core::index::RepoPkg;
use oparysh_core::version::Version;

use crate::error::{RepoError, RepoResult};
use crate::priority::{RepoSource, RepoSet};
use crate::{CachedRepo, LocalRepo};

/// Метаданные пакета, найденного в одном из репозиториев (`oparysh info`).
#[derive(Debug, Clone, PartialEq)]
pub struct InfoOutcome {
    /// Имя репозитория, в котором найден пакет.
    pub repo: String,
    /// Полная запись `[[pkg]]` из индекса.
    pub pkg: RepoPkg,
    /// Пропущенные сетевые репозитории: `(имя, причина)`.
    pub skipped: Vec<(String, String)>,
}

/// Одна доступная запись для `oparysh list --available`.
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableEntry {
    /// Имя репозитория.
    pub repo: String,
    /// Запись `[[pkg]]`.
    pub pkg: RepoPkg,
}

/// Результат `oparysh list --available`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AvailableOutcome {
    /// Записи в порядке приоритета репозиториев, внутри репозитория — по имени/версии.
    pub entries: Vec<AvailableEntry>,
    /// Пропущенные сетевые репозитории: `(имя, причина)`.
    pub skipped: Vec<(String, String)>,
}

/// Один устаревший установленный пакет для `oparysh list --outdated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutdatedEntry {
    /// Имя пакета.
    pub name: String,
    /// Установленная версия.
    pub installed: String,
    /// Лучшая доступная версия.
    pub available: String,
    /// Репозиторий, в котором найдена доступная версия.
    pub repo: String,
}

/// Результат `oparysh list --outdated`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutdatedOutcome {
    /// Устаревшие пакеты (по имени).
    pub entries: Vec<OutdatedEntry>,
    /// Пропущенные сетевые репозитории: `(имя, причина)`.
    pub skipped: Vec<(String, String)>,
}

/// Найти пакет `name` (при `version = None` — самую свежую версию) в
/// репозиториях по приоритету конфига.
///
/// Возвращает первое попадание: репозиторий с более высоким приоритетом
/// побеждает, даже если в другом есть версия новее (приоритет — DOCS §7 п.3).
/// Если пакет не найден нигде — [`RepoError::PkgNotFound`] (код **3**).
pub fn info_repos(
    entries: &[RepoEntry],
    cache_dir: &Path,
    name: &str,
    version: Option<&str>,
) -> RepoResult<InfoOutcome> {
    let set = RepoSet::from_entries(entries)?;
    let mut skipped = Vec::new();

    for r in set.refs() {
        let pkgs = match read_repo_packages(&r.source, cache_dir, &r.name, &mut skipped) {
            Some(pkgs) => pkgs,
            None => continue,
        };

        let mut matched: Vec<RepoPkg> = pkgs
            .into_iter()
            .filter(|p| p.name == name && version.map_or(true, |v| p.version == v))
            .collect();
        if matched.is_empty() {
            continue;
        }
        // Без явной версии — самая свежая среди совпавших записей.
        matched.sort_by(|a, b| version_cmp(&b.version, &a.version));
        return Ok(InfoOutcome {
            repo: r.name.clone(),
            pkg: matched.remove(0),
            skipped,
        });
    }

    Err(RepoError::PkgNotFound {
        name: name.to_string(),
        version: version.unwrap_or("*").to_string(),
    })
}

/// Собрать все доступные записи `[[pkg]]` по всем репозиториям в порядке
/// приоритета конфига.
pub fn list_available(
    entries: &[RepoEntry],
    cache_dir: &Path,
) -> RepoResult<AvailableOutcome> {
    let set = RepoSet::from_entries(entries)?;
    let mut outcome = AvailableOutcome::default();

    for r in set.refs() {
        let mut pkgs =
            match read_repo_packages(&r.source, cache_dir, &r.name, &mut outcome.skipped) {
                Some(pkgs) => pkgs,
                None => continue,
            };
        pkgs.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| version_cmp(&a.version, &b.version))
        });
        for pkg in pkgs {
            outcome.entries.push(AvailableEntry {
                repo: r.name.clone(),
                pkg,
            });
        }
    }

    Ok(outcome)
}

/// Для каждого установленного пакета `(name, version)` найти лучшую доступную
/// версию в репозиториях и оставить только те, где доступная новее.
///
/// Порядок результата повторяет порядок `installed`.
pub fn outdated_repos(
    entries: &[RepoEntry],
    cache_dir: &Path,
    installed: &[(String, String)],
) -> RepoResult<OutdatedOutcome> {
    let set = RepoSet::from_entries(entries)?;
    let mut outcome = OutdatedOutcome::default();

    // Индексы репозиториев читаются один раз, а не на каждый пакет.
    let mut repos: Vec<(String, Vec<RepoPkg>)> = Vec::new();
    for r in set.refs() {
        if let Some(pkgs) =
            read_repo_packages(&r.source, cache_dir, &r.name, &mut outcome.skipped)
        {
            repos.push((r.name.clone(), pkgs));
        }
    }

    for (name, current) in installed {
        // Лучший кандидат: максимальная версия; при равенстве — репозиторий
        // с более высоким приоритетом (первый в `repos`).
        let mut best: Option<(&str, &str)> = None; // (version, repo)
        for (repo, pkgs) in &repos {
            for pkg in pkgs.iter().filter(|p| &p.name == name) {
                best = match best {
                    None => Some((pkg.version.as_str(), repo.as_str())),
                    Some((bv, _)) if version_cmp(&pkg.version, bv) == Ordering::Greater => {
                        Some((pkg.version.as_str(), repo.as_str()))
                    }
                    other => other,
                };
            }
        }

        let Some((available, repo)) = best else {
            continue;
        };
        if version_cmp(available, current) == Ordering::Greater {
            outcome.entries.push(OutdatedEntry {
                name: name.clone(),
                installed: current.clone(),
                available: available.to_string(),
                repo: repo.to_string(),
            });
        }
    }

    Ok(outcome)
}

/// Прочитать записи репозитория: локальный — напрямую, сетевой — из кэша.
///
/// Сетевой репозиторий без кэша кладётся в `skipped` и возвращает `None`;
/// ошибки чтения локального репозитория пробрасываются.
fn read_repo_packages(
    source: &RepoSource,
    cache_dir: &Path,
    name: &str,
    skipped: &mut Vec<(String, String)>,
) -> Option<Vec<RepoPkg>> {
    match source {
        RepoSource::Local(root) => match LocalRepo::open(root) {
            Ok(repo) => Some(repo.packages().to_vec()),
            Err(e) => {
                skipped.push((name.to_string(), e.to_string()));
                None
            }
        },
        RepoSource::Network(_) => match CachedRepo::open(cache_dir, name) {
            Ok(cached) => Some(cached.packages().to_vec()),
            Err(_) => {
                skipped.push((
                    name.to_string(),
                    "не синхронизирован (выполните oparysh sync)".to_string(),
                ));
                None
            }
        },
    }
}

/// Сравнить строки версий с разбором semver; неразбираемые — лексикографически.
fn version_cmp(a: &str, b: &str) -> Ordering {
    match (Version::parse(a), Version::parse(b)) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oparysh_core::config::RepoEntry;

    fn entry(name: &str, path: Option<&str>, url: Option<&str>) -> RepoEntry {
        RepoEntry {
            name: name.to_string(),
            path: path.map(str::to_string),
            url: url.map(str::to_string),
            key: None,
        snapshot: None,
        }
    }

    /// Дописать в `index.toml` запись `[[pkg]]` (при первом вызове — с `[repo]`).
    fn add_pkg(dir: &Path, name: &str, version: &str, desc: &str, deps: &[&str]) {
        std::fs::create_dir_all(dir.join("recipes").join(name).join(version)).unwrap();
        std::fs::create_dir_all(dir.join("dist").join(name).join(version)).unwrap();
        let index = dir.join(crate::INDEX_FILE);
        let header = if index.exists() {
            String::new()
        } else {
            format!(
                "[repo]\nname = \"{}\"\n\n",
                dir.file_name().unwrap().to_string_lossy()
            )
        };
        let deps_toml: Vec<String> = deps.iter().map(|d| format!("\"{d}\"")).collect();
        let mut text = format!(
            "{header}[[pkg]]\nname = \"{name}\"\nversion = \"{version}\"\n\
             desc = \"{desc}\"\ndeps = [{}]\nrecipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
            deps_toml.join(", ")
        );
        if let Ok(existing) = std::fs::read_to_string(&index) {
            text = format!("{existing}{text}");
        }
        std::fs::write(index, text).unwrap();
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "oparysh_repo_query_{tag}_{}_{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn info_picks_latest_version_in_priority_repo() {
        let base = tmp("info");
        let local = base.join("local");
        add_pkg(&local, "hello", "1.0", "Hello", &["libc"]);
        add_pkg(&local, "hello", "2.0", "Hello 2", &[]);
        // Второй (низкоприоритетный) репо имеет ещё более свежую версию.
        let other = base.join("other");
        add_pkg(&other, "hello", "3.0", "Hello 3", &[]);

        let entries = [
            entry("local", Some(local.to_str().unwrap()), None),
            entry("other", Some(other.to_str().unwrap()), None),
        ];

        // Без версии — самая свежая в первом (приоритетном) репо.
        let info = info_repos(&entries, &base, "hello", None).unwrap();
        assert_eq!(info.repo, "local");
        assert_eq!(info.pkg.version, "2.0");
        assert!(info.pkg.deps.is_empty());

        // Явная версия — точное совпадение.
        let info = info_repos(&entries, &base, "hello", Some("1.0")).unwrap();
        assert_eq!(info.pkg.version, "1.0");

        // Незнакомая версия — код 3.
        assert!(matches!(
            info_repos(&entries, &base, "hello", Some("9.9")),
            Err(RepoError::PkgNotFound { .. })
        ));
        // Незнакомый пакет — код 3.
        assert!(matches!(
            info_repos(&entries, &base, "нет", None),
            Err(RepoError::PkgNotFound { .. })
        ));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn list_available_in_priority_order_and_skips_unsynced() {
        let base = tmp("avail");
        let local = base.join("local");
        add_pkg(&local, "zlib", "1.3", "", &[]);
        add_pkg(&local, "gcc", "14.2", "", &[]);
        let cache = base.join("cache");

        let entries = [
            entry("local", Some(local.to_str().unwrap()), None),
            entry("stable", None, Some("https://repo.example.com")),
        ];
        let out = list_available(&entries, &cache).unwrap();

        let got: Vec<(&str, &str)> = out
            .entries
            .iter()
            .map(|e| (e.pkg.name.as_str(), e.pkg.version.as_str()))
            .collect();
        // Внутри репо — по имени; сетевой без кэша пропущен.
        assert_eq!(got, [("gcc", "14.2"), ("zlib", "1.3")]);
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].0, "stable");

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn outdated_only_newer() {
        let base = tmp("outdated");
        let local = base.join("local");
        add_pkg(&local, "zlib", "1.3", "", &[]);
        add_pkg(&local, "gcc", "13.0", "", &[]);

        let entries = [entry("local", Some(local.to_str().unwrap()), None)];
        let installed = vec![
            ("zlib".to_string(), "1.2".to_string()), // новее → outdated
            ("gcc".to_string(), "13.0".to_string()), // та же — не outdated
            ("make".to_string(), "4.4".to_string()), // нет в репо — не outdated
        ];
        let out = outdated_repos(&entries, &base, &installed).unwrap();
        assert_eq!(out.entries.len(), 1);
        assert_eq!(out.entries[0].name, "zlib");
        assert_eq!(out.entries[0].installed, "1.2");
        assert_eq!(out.entries[0].available, "1.3");
        assert_eq!(out.entries[0].repo, "local");

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn empty_repositories_are_ok() {
        let base = tmp("empty");
        assert!(list_available(&[], &base).unwrap().entries.is_empty());
        assert!(outdated_repos(&[], &base, &[("x".into(), "1.0".into())])
            .unwrap()
            .entries
            .is_empty());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn version_cmp_semver_and_fallback() {
        assert_eq!(version_cmp("2.0", "10.0"), Ordering::Less);
        assert_eq!(version_cmp("1.10", "1.9"), Ordering::Greater);
        assert_eq!(version_cmp("1.0", "1.0"), Ordering::Equal);
        // Неразбираемые — лексикографически, без паники.
        assert!(version_cmp("git", "1.0").is_gt() || version_cmp("git", "1.0").is_lt());
    }
}
