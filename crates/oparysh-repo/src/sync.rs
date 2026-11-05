//! Команда `oparysh sync [repo…]` (этап 4.4 ROADMAP, DOCS §10).
//!
//! Синхронизация — скачивание/обновление `index.toml` **всех** сетевых
//! репозиториев из `[repositories]` конфига (или только указанных в
//! аргументах). Локальные репозитории скачивать нечего — они только
//! отмечаются в отчёте.
//!
//! Порядок обхода — порядок приоритета конфига (этап 4.3). Скачанный индекс
//! разбирается **до** записи в кэш ([`NetworkRepo::fetch_index`]): битый
//! index.toml не перекроет прошлый корректный.

use std::path::{Path, PathBuf};

use oparysh_core::config::RepoEntry;

use crate::error::{RepoError, RepoResult};
use crate::net::{CachedRepo, NetworkRepo};
use crate::priority::{RepoSource, RepoSet};

/// Источник успешной синхронизации.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncKind {
    /// Локальный репозиторий — скачивать нечего, проверено наличие.
    Local,
    /// Сетевой репозиторий — index.toml скачан и разобран.
    Network,
}

/// Отчёт по одному репозиторию.
#[derive(Debug, Clone)]
pub struct SyncOutcome {
    /// Имя репозитория из конфига.
    pub name: String,
    /// Локальный или сетевой.
    pub kind: SyncKind,
    /// Пакетов в (свежем) индексе.
    pub packages: usize,
    /// Куда лёг index.toml (для сетевого — путь в кэше; для локального —
    /// путь в самом репозитории).
    pub index_path: PathBuf,
}

/// Синхронизировать репозитории: все (`names` пуст) или только указанные.
///
/// Проходит записи `[repositories]` в порядке приоритета; сетевые —
/// `NetworkRepo::fetch_index` (скачать → разобрать → записать в кэш),
/// локальные — открыть `index.toml` на месте.
///
/// Неизвестное имя в `names` — [`RepoError::RepoNotFound`] (код **3**).
pub fn sync_repos(
    entries: &[RepoEntry],
    cache_dir: &Path,
    names: &[String],
) -> RepoResult<Vec<SyncOutcome>> {
    // Все имена заранее известны из конфига — проверим их до похода в сеть.
    if !names.is_empty() {
        let set = RepoSet::from_entries(entries)?;
        let unknown: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|n| set.find(n).is_none())
            .collect();
        if let Some(name) = unknown.into_iter().next() {
            return Err(RepoError::RepoNotFound {
                name: name.to_string(),
            });
        }
    }

    let set = RepoSet::from_entries(entries)?;
    let mut outcomes = Vec::new();
    for r in set.refs() {
        // Имена проверены выше (RepoNotFound), здесь просто отбираем
        // записи, попавшие в фильтр.
        if !names.is_empty() && !names.iter().any(|n| n == &r.name) {
            continue;
        }
        match &r.source {
            RepoSource::Local(root) => {
                let repo = crate::LocalRepo::open(root)?;
                outcomes.push(SyncOutcome {
                    name: r.name.clone(),
                    kind: SyncKind::Local,
                    packages: repo.packages().len(),
                    index_path: repo.index_path().to_path_buf(),
                });
            }
            RepoSource::Network(url) => {
                let repo = NetworkRepo::new(url.clone());
                let (index_path, index) = repo.fetch_index(cache_dir)?;
                outcomes.push(SyncOutcome {
                    name: r.name.clone(),
                    kind: SyncKind::Network,
                    packages: index.pkg.len(),
                    index_path,
                });
            }
        }
    }
    Ok(outcomes)
}

/// Прочитать индекс скачанного ранее репозитория из кэша
/// ([`CachedRepo::open`]) — то, чем `sync` завершает работу запасом на
/// будущие команды (`search`/`install` по кэшу индексов).
pub fn open_synced(cache_dir: &Path, repo_name: &str) -> RepoResult<CachedRepo> {
    CachedRepo::open(cache_dir, repo_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oparysh_core::config::RepoEntry;

    fn entry(name: &str, path: Option<String>, url: Option<String>) -> RepoEntry {
        RepoEntry {
            name: name.to_string(),
            path,
            url,
            key: None,
        }
    }

    /// Локальный мини-репозиторий с одним пакетом.
    fn local_repo(base: &Path, name: &str, pkg: &str) -> PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(dir.join("recipes").join(pkg).join("1.0")).unwrap();
        std::fs::create_dir_all(dir.join("dist").join(pkg).join("1.0")).unwrap();
        std::fs::write(
            dir.join(crate::INDEX_FILE),
            format!(
                "[repo]\nname = \"{name}\"\n\n[[pkg]]\nname = \"{pkg}\"\nversion = \"1.0\"\n\
                 recipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n"
            ),
        )
        .unwrap();
        dir
    }

    #[test]
    fn local_only_sync() {
        let base = std::env::temp_dir().join(format!("oparysh_sync_l_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = local_repo(&base, "main", "hello");
        let cache = base.join("cache");

        let out = sync_repos(
            &[entry("main", Some(dir.display().to_string()), None)],
            &cache,
            &[],
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, SyncKind::Local);
        assert_eq!(out[0].name, "main");
        assert_eq!(out[0].packages, 1);
        assert_eq!(out[0].index_path, dir.join(crate::INDEX_FILE));
        // Для локального репо ничего в кэше не появляется.
        assert!(!crate::repo_cache_dir(&cache).exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn unknown_repo_name_is_code3() {
        let entries = &[entry("stable", None, Some("https://repo.example.com".into()))];
        let err = sync_repos(entries, Path::new("/tmp/oparysh_sync_none"), &["нет".into()])
            .unwrap_err();
        assert!(matches!(err, RepoError::RepoNotFound { ref name } if name == "нет"), "{err}");
    }

    #[test]
    fn network_sync_caches_index() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let base = std::env::temp_dir().join(format!("oparysh_sync_n_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let local = local_repo(&base, "local", "hello");
        let cache = base.join("cache");

        // Мини-HTTP-сервер с ответом «index.toml stable».
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let body = "[repo]\nname = \"stable\"\n\n[[pkg]]\nname = \"gcc\"\nversion = \"14.2\"\n\
                    recipe_sha = \"9f2c\"\nsrc_sha256 = \"b04a\"\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            sock.write_all(response.as_bytes()).unwrap();
        });

        let url = format!("http://{addr}");
        let out = sync_repos(
            &[
                entry("local", Some(local.display().to_string()), None),
                entry("stable", None, Some(url)),
            ],
            &cache,
            &[],
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(out.len(), 2);
        // Первый — локальный (приоритет конфига), второй — скачанный.
        assert_eq!(out[0].kind, SyncKind::Local);
        assert_eq!(out[1].kind, SyncKind::Network);
        assert_eq!(out[1].packages, 1);
        assert_eq!(
            out[1].index_path,
            crate::net::cached_index_path(&cache, "stable")
        );
        assert!(out[1].index_path.is_file());
        // После sync кэш индекса открыт как CachedRepo.
        let cached = open_synced(&cache, "stable").unwrap();
        assert_eq!(cached.packages().len(), 1);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
