//! Приоритет репозиториев из секции `[repositories]` конфига (этап 4.3 ROADMAP, DOCS §6/§7).
//!
//! Репозитории перечисляются в конфиге списком; **порядок в конфиге задаёт
//! приоритет**: чем раньше запись, тем выше приоритет (DOCS §7 п.3 — «кандидаты
//! берутся со всех включённых репозиториев по приоритету»).
//!
//! Внутри одной записи возможны и локальный путь, и URL; тогда выигрывает
//! **локальный** источник (`local > network`).

use std::path::PathBuf;

use oparysh_core::config::RepoEntry;

use crate::error::{RepoError, RepoResult};
use crate::LocalRepo;

/// Источник одной записи репозитория.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoSource {
    /// Локальный каталог-репозиторий (`path = "…"`).
    Local(PathBuf),
    /// Сетевой репозиторий (`url = "https://…"`), индекс берётся из кэша.
    Network(String),
}

impl RepoSource {
    /// Определить источник записи конфига.
    ///
    /// Правила (DOCS §6, этап 4.3): запись должна иметь хотя бы один из
    /// `path`/`url`; при наличии обоих локальный путь **выигрывает**.
    pub fn from_entry(entry: &RepoEntry) -> RepoResult<RepoSource> {
        match (&entry.path, &entry.url) {
            (Some(path), _) => Ok(RepoSource::Local(PathBuf::from(path))),
            (None, Some(url)) => Ok(RepoSource::Network(url.clone())),
            (None, None) => Err(RepoError::EntryInvalid {
                name: entry.name.clone(),
                message: "запись должна иметь либо path, либо url".to_string(),
            }),
        }
    }
}

/// Ссылка на один настроенный репозиторий.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    /// Имя репозитория из конфига (используется как имя каталога кэша).
    pub name: String,
    /// Определённый источник записи (правило `local > network` применено).
    pub source: RepoSource,
    /// Путь к открытому ключу GPG для проверки подписи index.toml (опционально).
    pub key: Option<String>,
    /// URL или путь к `database.tar.zst` — оффлайн-снапшоту (этап 13.1).
    pub snapshot: Option<String>,
}

impl RepoRef {
    /// Корень локального репозитория, если источник локальный.
    pub fn local_root(&self) -> Option<&std::path::Path> {
        match &self.source {
            RepoSource::Local(root) => Some(root.as_path()),
            RepoSource::Network(_) => None,
        }
    }

    /// URL сетевого репозитория, если источник сетевой.
    pub fn network_url(&self) -> Option<&str> {
        match &self.source {
            RepoSource::Local(_) => None,
            RepoSource::Network(url) => Some(url.as_str()),
        }
    }
}

/// Упорядоченный набор репозиториев из конфига: индекс в векторе = приоритет
/// (0 — самый высокий).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoSet {
    /// Записи в порядке приоритета конфига.
    refs: Vec<RepoRef>,
}

impl RepoSet {
    /// Построить набор из записей `[repositories]` конфига.
    ///
    /// Каждая запись валидируется (правила см. [`RepoSource::from_entry`]);
    /// порядок сохраняется как есть — он и есть приоритет.
    pub fn from_entries(entries: &[RepoEntry]) -> RepoResult<Self> {
        let mut refs = Vec::with_capacity(entries.len());
        for entry in entries {
            let source = RepoSource::from_entry(entry)?;
            refs.push(RepoRef {
                name: entry.name.clone(),
                source,
                key: entry.key.clone(),
                snapshot: entry.snapshot.clone(),
            });
        }
        Ok(RepoSet { refs })
    }

    /// Построить набор прямо из конфига (его `[repositories].entries`).
    pub fn from_config(config: &oparysh_core::config::OparyshConfig) -> RepoResult<Self> {
        Self::from_entries(&config.repositories.entries)
    }

    /// Все записи в порядке приоритета.
    pub fn refs(&self) -> &[RepoRef] {
        &self.refs
    }

    /// Пустой набор (репозитории не настроены).
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    /// Количество репозиториев.
    pub fn len(&self) -> usize {
        self.refs.len()
    }

    /// Запись по имени (первая в порядке приоритета).
    pub fn find(&self, name: &str) -> Option<&RepoRef> {
        self.refs.iter().find(|r| r.name == name)
    }

    /// Только локальные репозитории, в порядке приоритета.
    pub fn local(&self) -> impl Iterator<Item = (usize, &RepoRef)> {
        self.refs
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.source, RepoSource::Local(_)))
    }

    /// Только сетевые репозитории, в порядке приоритета.
    pub fn network(&self) -> impl Iterator<Item = (usize, &RepoRef)> {
        self.refs
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.source, RepoSource::Network(_)))
    }

    /// Поиск пакета и версии по локальным репозиториям в порядке приоритета.
    ///
    /// Возвращает первый репозиторий, которому принадлежит запись `[[pkg]]`
    /// с указанными именем и версией: `Some((индекс_приоритета, корень))`.
    /// Если пакет не найден ни в одном из них — `None` (код **3** вернёт
    /// вызывающий код, когда точно знает, что искать больше негде).
    ///
    /// Сетевые репозитории не сканируются (для них есть кэш индексов —
    /// этап 4.2); метод ниже, [`RepoSet::lookup_cached`], ищет и там.
    pub fn lookup_local(&self, name: &str, version: &str) -> RepoResult<Option<(usize, PathBuf)>> {
        for (prio, r) in self.local() {
            let root = r.local_root().expect("локальный из local()");
            let repo = LocalRepo::open(root)?;
            if repo.pkg(name, version).is_ok() {
                return Ok(Some((prio, root.to_path_buf())));
            }
        }
        Ok(None)
    }

    /// Поиск пакета по всем репозиториям в порядке приоритета: локальные —
    /// напрямую, сетевые — по кэшу индексов (`<cache>/repo/<name>/index.toml`).
    ///
    /// Возвращает `(индекс_приоритета, корень/имя-в-кэше)` первого попадания.
    pub fn lookup_cached(
        &self,
        name: &str,
        version: &str,
        cache_dir: &std::path::Path,
    ) -> RepoResult<Option<(usize, PathBuf)>> {
        for (prio, r) in self.refs.iter().enumerate() {
            match &r.source {
                RepoSource::Local(root) => {
                    let repo = LocalRepo::open(root)?;
                    if repo.pkg(name, version).is_ok() {
                        return Ok(Some((prio, root.clone())));
                    }
                }
                RepoSource::Network(_) => {
                    let cached = crate::CachedRepo::open(cache_dir, &r.name)?;
                    if cached.index.pkg.iter().any(|p| p.name == name && p.version == version) {
                        return Ok(Some((
                            prio,
                            crate::cached_index_path(cache_dir, &r.name)
                                .parent()
                                .expect("у index_path есть родитель")
                                .to_path_buf(),
                        )));
                    }
                }
            }
        }
        Ok(None)
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

    #[test]
    fn order_is_priority() {
        let set = RepoSet::from_entries(&[
            entry("local", Some("/srv/oparysh-repo"), None),
            entry("stable", None, Some("https://repo.example.com/stable")),
        ])
        .unwrap();

        assert_eq!(set.len(), 2);
        let refs = set.refs();
        // Порядок конфига сохранён: первый — самый приоритетный.
        assert_eq!(refs[0].name, "local");
        assert_eq!(refs[0].local_root(), Some(std::path::Path::new("/srv/oparysh-repo")));
        assert_eq!(refs[1].name, "stable");
        assert_eq!(refs[1].network_url(), Some("https://repo.example.com/stable"));

        // Фильтры local/network.
        assert_eq!(set.local().map(|(_, r)| r.name.clone()).collect::<Vec<_>>(), ["local"]);
        assert_eq!(set.network().map(|(_, r)| r.name.clone()).collect::<Vec<_>>(), ["stable"]);
        assert_eq!(set.find("stable").unwrap().name, "stable");
        assert!(set.find("нет").is_none());
    }

    #[test]
    fn local_beats_network_in_one_entry() {
        // У записи есть и path, и url — источник локальный (local > network).
        let src = RepoSource::from_entry(&entry(
            "dup",
            Some("/srv/repo"),
            Some("https://repo.example.com"),
        ))
        .unwrap();
        assert_eq!(src, RepoSource::Local(PathBuf::from("/srv/repo")));
    }

    #[test]
    fn entry_without_path_and_url_is_invalid() {
        let err = RepoSet::from_entries(&[entry("broken", None, None)]).unwrap_err();
        assert!(matches!(err, RepoError::EntryInvalid { .. }), "{err}");
    }

    #[test]
    fn empty_set_is_ok() {
        let set = RepoSet::from_entries(&[]).unwrap();
        assert!(set.is_empty());
        assert_eq!(set.lookup_local("hello", "1.0").unwrap(), None);
        assert_eq!(set.find("hello"), None);
    }

    /// Мини-репозиторий с index.toml, содержащим один пакет name-version.
    /// Вызовы для одного каталога накапливают записи `[[pkg]]` (append).
    fn mini_repo(dir: &std::path::Path, name: &str, version: &str) {
        std::fs::create_dir_all(dir.join("recipes").join(name).join(version)).unwrap();
        std::fs::create_dir_all(dir.join("dist").join(name).join(version)).unwrap();
        let mut index = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(crate::INDEX_FILE))
            .unwrap();
        use std::io::Write;
        let header = if std::fs::metadata(dir.join(crate::INDEX_FILE)).unwrap().len() == 0 {
            format!("[repo]\nname = \"{}\"\n\n", dir.file_name().unwrap().to_string_lossy())
        } else {
            String::new()
        };
        write!(
            index,
            "{}[[pkg]]\nname = \"{}\"\nversion = \"{}\"\nrecipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
            header, name, version
        )
        .unwrap();
    }

    #[test]
    fn local_lookup_prioritizes_first_repo() {
        let base = std::env::temp_dir().join(format!("oparysh_repo_prio_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let first = base.join("first");
        let second = base.join("second");
        // hello-1.0 есть в обоих; kitty-2.0 — только во втором.
        mini_repo(&first, "hello", "1.0");
        mini_repo(&second, "hello", "1.0");
        mini_repo(&second, "kitty", "2.0");

        let set = RepoSet::from_entries(&[
            entry("first", Some(first.to_str().unwrap()), None),
            entry("second", Some(second.to_str().unwrap()), None),
        ])
        .unwrap();

        // hello-1.0 найден в первом (самом приоритетном) репо.
        let (prio, root) = set.lookup_local("hello", "1.0").unwrap().unwrap();
        assert_eq!(prio, 0);        assert_eq!(root, first);
        // Отсутствующий в первом найден во втором.
        let (prio, root) = set.lookup_local("kitty", "2.0").unwrap().unwrap();
        assert_eq!(prio, 1);
        assert_eq!(root, second);
        // Незнакомый пакет — нигде.
        assert_eq!(set.lookup_local("нет", "1.0").unwrap(), None);

        // Поменяй порядок — поменялся и победитель.
        let reversed = RepoSet::from_entries(&[
            entry("second", Some(second.to_str().unwrap()), None),
            entry("first", Some(first.to_str().unwrap()), None),
        ])
        .unwrap();
        assert_eq!(reversed.lookup_local("hello", "1.0").unwrap().unwrap().0, 0);
        assert_eq!(
            reversed.lookup_local("hello", "1.0").unwrap().unwrap().1,
            second
        );

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn broken_local_repo_is_an_error() {
        let dir = std::env::temp_dir().join(format!("oparysh_repo_prio_bad_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let set = RepoSet::from_entries(&[entry("bad", Some(dir.to_str().unwrap()), None)]).unwrap();
        // index.toml отсутствует — NotARepo, а не тихий None.
        assert!(matches!(
            set.lookup_local("hello", "1.0"),
            Err(RepoError::NotARepo { .. })
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cached_lookup_includes_network_repos() {
        let base = std::env::temp_dir().join(format!("oparysh_repo_prio_net_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let local = base.join("local");
        let cache = base.join("cache");
        mini_repo(&local, "hello", "1.0");

        // В кэше сетевого репо — kitty-2.0 (как если бы сделал fetch_index).
        let net_cache = cache.join("repo").join("stable");
        std::fs::create_dir_all(&net_cache).unwrap();
        std::fs::write(
            net_cache.join(crate::INDEX_FILE),
            "[repo]\nname = \"stable\"\n\n[[pkg]]\nname = \"kitty\"\nversion = \"2.0\"\nrecipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
        )
        .unwrap();

        let set = RepoSet::from_entries(&[
            entry("local", Some(local.to_str().unwrap()), None),
            entry("stable", None, Some("https://repo.example.com/stable")),
        ])
        .unwrap();

        // kitty найден в (кэше) сетевого репо — приоритет 1.
        let (prio, path) = set.lookup_cached("kitty", "2.0", &cache).unwrap().unwrap();
        assert_eq!(prio, 1);
        assert_eq!(path, net_cache);
        // Поменяем приоритет: сетевое репо первым, но hello там нет —
        // всё равно найдётся в локальном, приоритет 1.
        let reversed = RepoSet::from_entries(&[
            entry("stable", None, Some("https://repo.example.com/stable")),
            entry("local", Some(local.to_str().unwrap()), None),
        ])
        .unwrap();
        let (prio, _) = reversed.lookup_cached("hello", "1.0", &cache).unwrap().unwrap();
        assert_eq!(prio, 1);

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn from_config_reads_entries() {
        let config = oparysh_core::config::OparyshConfig::parse(
            "[repositories]\n[[repositories.entries]]\nname = \"local\"\npath = \"/srv/repo\"\n",
        )
        .unwrap();
        let set = RepoSet::from_config(&config).unwrap();
        assert_eq!(set.refs()[0].name, "local");
    }
}
