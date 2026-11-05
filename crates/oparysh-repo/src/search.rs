//! Команда `oparysh search <pattern>` (этап 4.6 ROADMAP, DOCS §10).
//!
//! Поиск по имени/описанию/тегам пакета во всех настроенных репозиториях
//! (локальные — напрямую, сетевые — по кэшу индексов из `oparysh sync`).
//!
//! Шаблон:
//! - `/regex/` — регулярное выражение (регистр игнорируется), например `/^lib.*/`;
//! - иначе — glob: `*` — любая последовательность, `?` — один символ
//!   (регистр игнорируется), например `gcc*` или `*-git`.
//!
//! Совпадение ищется в имени, описании и тегах пакета.

use std::path::Path;

use regex::Regex;

use oparysh_core::config::RepoEntry;
use oparysh_core::index::RepoPkg;

use crate::error::{RepoError, RepoResult};
use crate::priority::{RepoSource, RepoSet};
use crate::{CachedRepo, LocalRepo};

/// Скомпилированный шаблон поиска.
#[derive(Debug, Clone)]
pub struct SearchPattern {
    original: String,
    regex: Regex,
}

impl SearchPattern {
    /// Разобрать шаблон: `/regex/` — регулярное выражение, иначе glob.
    pub fn parse(pattern: &str) -> RepoResult<Self> {
        let body = if pattern.len() >= 2 && pattern.starts_with('/') && pattern.ends_with('/') {
            pattern[1..pattern.len() - 1].to_string()
        } else {
            glob_to_regex(pattern)
        };
        let regex = Regex::new(&format!("(?i){body}")).map_err(|e| RepoError::BadPattern {
            pattern: pattern.to_string(),
            message: e.to_string(),
        })?;
        Ok(SearchPattern {
            original: pattern.to_string(),
            regex,
        })
    }

    /// Исходная строка шаблона (для сообщений).
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Совпадает ли пакет (по имени, описанию или тегам).
    pub fn matches(&self, pkg: &RepoPkg) -> bool {
        self.regex.is_match(&pkg.name)
            || self.regex.is_match(&pkg.desc)
            || pkg.tags.iter().any(|t| self.regex.is_match(t))
    }
}

/// Перевести glob в эквивалентное регулярное выражение.
///
/// Поддерживаются `*` (любая последовательность, включая пустую) и `?`
/// (ровно один символ); все остальные символы экранируются как литералы.
/// Частичное совпадение допустимо: `gcc*` матчит `gcc-libs`, а `gcc` —
/// тоже `gcc-libs` (поэтому для точного поиска используйте `/^gcc$/`).
fn glob_to_regex(glob: &str) -> String {
    let mut out = String::with_capacity(glob.len() * 2);
    for ch in glob.chars() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c if c.is_ascii_alphanumeric() || c == '_' => out.push(c),
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    out
}

/// Одно совпадение: пакет из конкретного репозитория.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Имя репозитория из конфига.
    pub repo: String,
    /// Имя пакета.
    pub name: String,
    /// Версия.
    pub version: String,
    /// Описание (может быть пустым).
    pub desc: String,
    /// Теги.
    pub tags: Vec<String>,
}

/// Результат поиска по всем репозиториям.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchOutcome {
    /// Совпадения в порядке приоритета репозиториев конфига.
    pub hits: Vec<SearchHit>,
    /// Репозитории, которые пришлось пропустить: `(имя, причина)`.
    /// Сетевой репозиторий без кэша индекса пропускается с подсказкой про `oparysh sync`.
    pub skipped: Vec<(String, String)>,
}

/// Найти пакеты по шаблону во всех настроенных репозиториях.
///
/// Локальные репозитории открываются напрямую; сетевые — читаются из
/// кэша индексов (`<cache>/repo/<name>/index.toml`), который создаётся
/// `oparysh sync`. Сетевой репозиторий без синхронизации не является
/// ошибкой — он попадает в [`SearchOutcome::skipped`].
pub fn search_repos(
    entries: &[RepoEntry],
    cache_dir: &Path,
    pattern: &SearchPattern,
) -> RepoResult<SearchOutcome> {
    let set = RepoSet::from_entries(entries)?;
    let mut outcome = SearchOutcome::default();

    for r in set.refs() {
        match &r.source {
            RepoSource::Local(root) => {
                let repo = LocalRepo::open(root)?;
                collect_hits(&mut outcome, &r.name, repo.packages(), pattern);
            }
            RepoSource::Network(_) => match CachedRepo::open(cache_dir, &r.name) {
                Ok(cached) => {
                    collect_hits(&mut outcome, &r.name, cached.packages(), pattern);
                }
                Err(_) => outcome.skipped.push((
                    r.name.clone(),
                    "не синхронизирован (выполните oparysh sync)".to_string(),
                )),
            },
        }
    }

    Ok(outcome)
}

fn collect_hits(outcome: &mut SearchOutcome, repo_name: &str, pkgs: &[RepoPkg], pattern: &SearchPattern) {
    for pkg in pkgs {
        if pattern.matches(pkg) {
            outcome.hits.push(SearchHit {
                repo: repo_name.to_string(),
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                desc: pkg.desc.clone(),
                tags: pkg.tags.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, desc: &str, tags: &[&str]) -> RepoPkg {
        RepoPkg {
            name: name.to_string(),
            version: "1.0".to_string(),
            build: 0,
            arch: vec![],
            desc: desc.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            deps: vec![],
            provides: vec![],
            conflicts: vec![],
            recipe_sha: "aa".to_string(),
            src_sha256: "bb".to_string(),
        }
    }

    #[test]
    fn glob_matches_name_desc_tags() {
        let gcc = pkg("gcc", "GNU Compiler Collection", &["devel", "toolchain"]);
        let vim = pkg("vim", "Vi IMproved", &["editor"]);

        let by_name = SearchPattern::parse("gc*").unwrap();
        assert!(by_name.matches(&gcc));
        assert!(!by_name.matches(&vim));

        let by_desc = SearchPattern::parse("*compiler*").unwrap();
        assert!(by_desc.matches(&gcc));
        assert!(!by_desc.matches(&vim));

        let by_tag = SearchPattern::parse("toolchain").unwrap();
        assert!(by_tag.matches(&gcc));
        assert!(!by_tag.matches(&vim));

        // Регистр игнорируется.
        let ci = SearchPattern::parse("GCC").unwrap();
        assert!(ci.matches(&gcc));

        // `?` — ровно один символ.
        let q = SearchPattern::parse("vi?").unwrap();
        assert!(q.matches(&vim));
        assert!(!q.matches(&gcc));
    }

    #[test]
    fn regex_matches_with_slashes() {
        let gcc = pkg("gcc", "GNU Compiler Collection", &["devel"]);
        let re = SearchPattern::parse("/^g.*c$/").unwrap();
        assert!(re.matches(&gcc));
        let no = SearchPattern::parse("/^x/").unwrap();
        assert!(!no.matches(&gcc));
    }

    #[test]
    fn bad_patterns() {
        assert!(matches!(
            SearchPattern::parse("/[unclosed/"),
            Err(RepoError::BadPattern { .. })
        ));
    }

    #[test]
    fn search_over_local_and_cached_network() {
        let base = std::env::temp_dir().join(format!("oparysh_search_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let local = base.join("local");
        let cache = base.join("cache");

        // Локальный репо с gcc.
        std::fs::create_dir_all(local.join("recipes").join("gcc").join("1.0")).unwrap();
        std::fs::create_dir_all(local.join("dist").join("gcc").join("1.0")).unwrap();
        std::fs::write(
            local.join(crate::INDEX_FILE),
            "[repo]\nname = \"local\"\n\n[[pkg]]\nname = \"gcc\"\nversion = \"1.0\"\n\
             desc = \"GNU Compiler Collection\"\ntags = [\"toolchain\"]\n\
             recipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
        )
        .unwrap();

        // Кэш сетевого репо с clang.
        let net_cache = cache.join("repo").join("stable");
        std::fs::create_dir_all(&net_cache).unwrap();
        std::fs::write(
            net_cache.join(crate::INDEX_FILE),
            "[repo]\nname = \"stable\"\n\n[[pkg]]\nname = \"clang\"\nversion = \"19.0\"\n\
             desc = \"Clang compiler\"\ntags = [\"toolchain\"]\n\
             recipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
        )
        .unwrap();

        let entries = vec![
            RepoEntry { name: "local".into(), path: Some(local.display().to_string()), url: None, key: None },
            RepoEntry { name: "stable".into(), path: None, url: Some("https://repo.example.com".into()), key: None },
            // Не синхронизирован — должен попасть в skipped.
            RepoEntry { name: "unsynced".into(), path: None, url: Some("https://repo2.example.com".into()), key: None },
        ];

        let pattern = SearchPattern::parse("*compiler*").unwrap();
        let out = search_repos(&entries, &cache, &pattern).unwrap();
        let names: Vec<&str> = out.hits.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, ["gcc", "clang"]); // порядок приоритета конфига
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].0, "unsynced");

        std::fs::remove_dir_all(&base).unwrap();
    }
}
