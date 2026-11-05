//! HTTP-клиент для сетевых репозиториев (этап 4.2 ROADMAP, DOCS §4).
//!
//! Сетевой репозиторий — статический каталог, раздаваемый по HTTP(S). При
//! синхронизации скачивается **только** `index.toml`; он кладётся в кэш:
//!
//! ```text
//! <cache_dir>/repo/<repo.name>/index.toml     # по умолчанию /usr/local/oparysh/cache/repo/
//! ```
//!
//! Скачивание атомарно: тело сначала пишется во временный файл (`*.part`),
//!	then переименовывается в `index.toml`, чтобы частичный файл никогда не
//! выглядел как готовый кэш. Сбой сети — код возврата **5** (DOCS §11).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use oparysh_core::index::RepoIndex;

use crate::error::{RepoError, RepoResult};
use crate::{INDEX_FILE, RECIPES_DIR, RECIPE_FILE};

/// Имя подкаталога кэша с индексами сетевых репозиториев.
pub const REPO_CACHE_DIR: &str = "repo";

/// Временное расширение при атомарной записи файла кэша.
const PART_EXT: &str = "part";

/// Каталог кэша сетевых репозиториев: `<cache_dir>/repo/`.
pub fn repo_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join(REPO_CACHE_DIR)
}

/// Путь index.toml репозитория `name` в кэше.
pub fn cached_index_path(cache_dir: &Path, repo_name: &str) -> PathBuf {
    repo_cache_dir(cache_dir).join(repo_name).join(INDEX_FILE)
}

/// Сетевой репозиторий: базовый URL каталога с `index.toml`.
#[derive(Debug, Clone)]
pub struct NetworkRepo {
    url: String,
}

impl NetworkRepo {
    /// Создать сетевой репозиторий по базовому URL (без хвостового `/`).
    pub fn new(url: impl Into<String>) -> Self {
        let mut url = url.into();
        while url.ends_with('/') {
            url.pop();
        }
        NetworkRepo { url }
    }

    /// Базовый URL репозитория.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Полный URL `index.toml`.
    pub fn index_url(&self) -> String {
        format!("{}/{}", self.url, INDEX_FILE)
    }

    /// Скачивает `index.toml` в кэш `<cache_dir>/repo/<repo.name>/`.
    ///
    /// Возвращает путь к файлу кэша (уже на диске) и разобранный индекс.
    /// Разбор выполняется **до** записи в кэш: битый index.toml не попадает
    /// в кэш и не перекрывает предыдущий корректный.
    pub fn fetch_index(&self, cache_dir: &Path) -> RepoResult<(PathBuf, RepoIndex)> {
        let url = self.index_url();
        let raw = self.download(&url)?;

        // Разбор индекса до записи в кэш — fail fast. UTF-8 с bad-байтами
        // потеряет их, но TOML-парсер всё равно откажется слушать такой файл.
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let index = RepoIndex::parse(&raw).map_err(|source| RepoError::IndexParse {
            path: url.clone(),
            source,
        })?;

        let dest_dir = repo_cache_dir(cache_dir).join(&index.repo.name);
        if !RepoName::validate(&index.repo.name) {
            return Err(RepoError::BadRepoName {
                name: index.repo.name.clone(),
            });
        }

        let dest = dest_dir.join(INDEX_FILE);
        let tmp = dest_dir.join(format!("{INDEX_FILE}.{PART_EXT}"));
        fs::create_dir_all(&dest_dir)?;
        fs::write(&tmp, raw.as_bytes())?;
        // Атомарно подменяем старый кэш новым.
        fs::rename(&tmp, &dest)?;

        Ok((dest, index))
    }

    /// Скачивает рецепт `recipes/<name>/<ver>/pkg.toml` из сети в кэш
    /// `<cache>/repo/<name>/recipes/…` и **проверяет `recipe_sha`** по
    /// индексу (DOCS §9, строка «файл рецепта … oparysh sync, при загрузке
    /// из сети»): несовпадение хэша — [`RepoError::RecipeShaMismatch`],
    /// код возврата **6**.
    ///
    /// Атомарно, как `fetch_index`: сначала файл `.part`, затем rename.
    /// Индекс — уже скачанный ранее ([`NetworkRepo::fetch_index`] или
    /// [`CachedRepo`]), он задаёт ожидаемый `recipe_sha`.
    pub fn fetch_recipe(
        &self,
        cache_dir: &Path,
        index: &RepoIndex,
        name: &str,
        version: &str,
    ) -> RepoResult<PathBuf> {
        let pkg = index
            .pkg
            .iter()
            .find(|p| p.name == name && p.version == version)
            .ok_or(RepoError::PkgNotFound {
                name: name.to_string(),
                version: version.to_string(),
            })?;

        let url = format!("{}/{}/{}/{}/{}", self.url, RECIPES_DIR, name, version, RECIPE_FILE);
        let raw = self.download(&url)?;
        let raw = String::from_utf8_lossy(&raw).into_owned();

        // Хэш скачанного — до записи в кэш: битый файл не выглядит как
        // готовый рецепт.
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(&pkg.recipe_sha) {
            return Err(RepoError::RecipeShaMismatch {
                path: url.clone(),
                expected: pkg.recipe_sha.clone(),
                actual,
            });
        }

        let dest_dir = repo_cache_dir(cache_dir)
            .join(&index.repo.name)
            .join(RECIPES_DIR)
            .join(name)
            .join(version);
        let dest = dest_dir.join(RECIPE_FILE);
        let tmp = dest_dir.join(format!("{RECIPE_FILE}.{PART_EXT}"));
        fs::create_dir_all(&dest_dir)?;
        fs::write(&tmp, raw.as_bytes())?;
        fs::rename(&tmp, &dest)?;
        Ok(dest)
    }

    /// Открыть репозиторий из кэша, если он уже скачан (как [`LocalRepo`], но
    /// обязательных `recipes/`/`dist/` — кэш индексов их не содержит).
    pub fn open_cached(&self, cache_dir: &Path, repo_name: &str) -> RepoResult<CachedRepo> {
        CachedRepo::open(cache_dir, repo_name)
    }

    fn download(&self, url: &str) -> RepoResult<Vec<u8>> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        let mut resp = agent
            .get(url)
            .call()
            .map_err(|source| RepoError::Network {
                url: url.to_string(),
                source,
            })?;

        // Статус уже проверен ureq (не-2xx = Err), но прочитать тело всё же
        // надо отдельно: неудача чтения тоже считается сетевым сбоем.
        resp.body_mut()
            .read_to_vec()
            .map_err(|source| RepoError::Network {
                url: url.to_string(),
                source,
            })
    }
}

/// Репозиторий в кэше (`<cache_dir>/repo/<name>/`): только разобрать index.toml.
///
/// В отличие от [`LocalRepo`] не требует `recipes/` и `dist/` — кэш индексов
/// в момент `sync` их ещё не содержит.
///
/// [`LocalRepo`]: crate::LocalRepo
#[derive(Debug)]
pub struct CachedRepo {
    pub index: RepoIndex,
    pub index_path: PathBuf,
}

impl CachedRepo {
    /// Прочитать index.toml репозитория из кэша.
    pub fn open(cache_dir: &Path, repo_name: &str) -> RepoResult<Self> {
        if !RepoName::validate(repo_name) {
            return Err(RepoError::BadRepoName {
                name: repo_name.to_string(),
            });
        }
        let index_path = cached_index_path(cache_dir, repo_name);
        let raw = fs::read_to_string(&index_path).map_err(|source| RepoError::NotARepo {
            path: index_path.display().to_string(),
            message: format!("index.toml в кэше отсутствует ({source}); выполните oparysh sync"),
        })?;
        let index = RepoIndex::parse(&raw).map_err(|source| RepoError::IndexParse {
            path: index_path.display().to_string(),
            source,
        })?;
        Ok(CachedRepo {
            index,
            index_path,
        })
    }

    /// Все записи `[[pkg]]` индекса.
    pub fn packages(&self) -> &[oparysh_core::index::RepoPkg] {
        &self.index.pkg
    }
}

/// Проверка имени репозитория: годится как имя каталога кэша.
struct RepoName;

impl RepoName {
    fn validate(name: &str) -> bool {
        !name.is_empty()
            && !name.starts_with('.')
            && !name
                .split(['/', '\\', ':'])
                .any(|part| part == ".." || part.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Фикстура-каталог кэша: `<tmp>/cache/`.
    fn cache_dir() -> PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join(format!("oparysh_repo_net_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Мини-HTTP-сервер: принимает одно соединение и отвечает заранее
    /// заготовленным ответом. Возвращает адрес и поток (для join).
    fn spawn_server(response: String) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            // Прочитать заголовки запроса (до пустой строки).
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            sock.write_all(response.as_bytes()).unwrap();
        });
        (addr, handle)
    }

    fn http_response(status: &str, body: &[u8]) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ) + &String::from_utf8_lossy(body)
    }

    const INDEX_BODY: &[u8] = b"[repo]\nname = \"stable\"\n\".origin\" = \"https://repo.example.com/stable\"\nupdated = 1770000000\n\n[[pkg]]\nname = \"hello\"\nversion = \"1.0\"\nrecipe_sha = \"9f2c\"\nsrc_sha256 = \"b04a\"\n";

    fn dest_part(path: &Path) -> PathBuf {
        let mut s = path.as_os_str().to_os_string();
        s.push(format!(".{PART_EXT}"));
        s.into()
    }

    #[test]
    fn fetch_ok() {
        let cache = cache_dir().join("cache");
        let (addr, server) = spawn_server(http_response("200 OK", INDEX_BODY));

        let repo = NetworkRepo::new(format!("http://{addr}"));
        let (path, index) = repo.fetch_index(&cache).unwrap();

        // index.toml лежит в <cache>/repo/stable/
        let expected = cache.join("repo").join("stable").join(INDEX_FILE);
        assert_eq!(path, expected);
        assert!(path.is_file());
        // Временный .part-файл не остаётся.
        assert!(!dest_part(&path).exists());
        // Индекс разобрался.
        assert_eq!(index.repo.name, "stable");
        assert_eq!(index.pkg.len(), 1);
        // Хвостовой слэш URL отрезан.
        assert_eq!(NetworkRepo::new("http://x/").url(), "http://x");
        server.join().unwrap();
        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }

    #[test]
    fn fetch_http_error() {
        let cache = cache_dir().join("cache");
        let (addr, server) = spawn_server(http_response("404 Not Found", b"none"));

        let repo = NetworkRepo::new(format!("http://{addr}"));
        // Сетевой сбой, кэш не записан.
        let err = repo.fetch_index(&cache).unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, RepoError::Network { .. }), "{msg}");
        assert!(!repo_cache_dir(&cache)
            .join("stable")
            .join(INDEX_FILE)
            .exists());
        server.join().unwrap();
        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }

    #[test]
    fn fetch_bad_index_not_cached() {
        let cache = cache_dir().join("cache");
        let body = b"[[[not toml";
        let (addr, server) = spawn_server(http_response("200 OK", body));

        let repo = NetworkRepo::new(format!("http://{addr}"));
        let err = repo.fetch_index(&cache).unwrap_err();
        assert!(matches!(err, RepoError::IndexParse { .. }), "{err}");
        // Битый индекс не попал в кэш (fail fast до записи).
        assert!(!repo_cache_dir(&cache).exists());
        server.join().unwrap();
        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }

    #[test]
    fn fetch_replaces_old_cache() {
        let cache = cache_dir().join("cache");
        let (addr, server) = spawn_server(http_response("200 OK", INDEX_BODY));
        let repo = NetworkRepo::new(format!("http://{addr}"));
        let (path, _old) = repo.fetch_index(&cache).unwrap();
        server.join().unwrap();

        // Вторая синхронизация с другим содержимым — кэш обновлён.
        let (addr2, server2) = spawn_server(http_response(
            "200 OK",
            b"[repo]\nname = \"stable\"\nupdated = 42\n\n[[pkg]]\nname = \"hello\"\nversion = \"2.0\"\nrecipe_sha = \"aa\"\nsrc_sha256 = \"bb\"\n",
        ));
        let repo2 = NetworkRepo::new(format!("http://{addr2}"));
        let (path2, index2) = repo2.fetch_index(&cache).unwrap();
        assert_eq!(path2, path);
        assert_eq!(index2.repo.updated, 42);
        server2.join().unwrap();
        // Временный .part-файл не остался.
        assert!(!dest_part(&path).exists());
        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }

    #[test]
    fn cached_repo_read_and_open_cached() {
        let cache = cache_dir().join("cache");

        // Кэша ещё нет — ошибка NotARepo с подсказкой выполнить sync.
        let err = NetworkRepo::new("http://example.invalid")
            .open_cached(&cache, "stable")
            .unwrap_err();
        assert!(matches!(err, RepoError::NotARepo { .. }), "{err}");

        // Пишем кэш вручную так, как его сделал бы fetch_index.
        let dir = repo_cache_dir(&cache).join("stable");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(INDEX_FILE), INDEX_BODY).unwrap();

        let cached = NetworkRepo::new("http://example.invalid")
            .open_cached(&cache, "stable")
            .unwrap();
        assert_eq!(cached.index_path, dir.join(INDEX_FILE));
        assert_eq!(cached.packages().len(), 1);
        assert_eq!(cached_index_path(&cache, "stable"), dir.join(INDEX_FILE));

        // Путь-траверс в имени репозитория отклоняется.
        let err2 = CachedRepo::open(&cache, "../traversal").unwrap_err();
        assert!(matches!(err2, RepoError::BadRepoName { .. }), "{err2}");
        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }

    /// Тело index.toml + sha256 скачанного рецепта hello-1.0 (для fetch_recipe).
    mod recipe_fixture {
        use sha2::{Digest, Sha256};

        pub const RECIPE: &[u8] = b"[pkg]\nname = \"hello\"\nversion = \"1.0\"\n";

        pub fn recipe_sha() -> String {
            let mut hasher = Sha256::new();
            hasher.update(RECIPE);
            hex::encode(hasher.finalize())
        }

        pub fn index_body() -> String {
            format!(
                "[repo]\nname = \"stable\"\n\n[[pkg]]\nname = \"hello\"\nversion = \"1.0\"\n\
                 recipe_sha = \"{}\"\nsrc_sha256 = \"b04a\"\n",
                recipe_sha()
            )
        }
    }

    #[test]
    fn fetch_recipe_ok_and_mismatch() {
        let cache = cache_dir().join("cache");
        let index =
            oparysh_core::index::RepoIndex::parse(&recipe_fixture::index_body()).unwrap();

        // Сервер отдаёт скачанный рецепт — тот же файл, что в index.toml.
        let (addr, server) =
            spawn_server(http_response("200 OK", recipe_fixture::RECIPE));
        let repo = NetworkRepo::new(format!("http://{addr}"));
        let path = repo.fetch_recipe(&cache, &index, "hello", "1.0").unwrap();
        server.join().unwrap();

        let expected = repo_cache_dir(&cache)
            .join("stable")
            .join(RECIPES_DIR)
            .join("hello")
            .join("1.0")
            .join(RECIPE_FILE);
        assert_eq!(path, expected);
        assert!(path.is_file());
        assert!(!dest_part(&path).exists());

        // Сервер отдаёт **подменённый** рецепт — sha не совпадает, код **6**,
        // кэш не пишется поверх корректного.
        fs::remove_dir_all(&cache).unwrap();
        let (addr2, server2) =
            spawn_server(http_response("200 OK", b"[pkg]\nname = \"evil\"\n"));
        let repo2 = NetworkRepo::new(format!("http://{addr2}"));
        let err = repo2
            .fetch_recipe(&cache, &index, "hello", "1.0")
            .unwrap_err();
        server2.join().unwrap();
        assert!(
            matches!(err, RepoError::RecipeShaMismatch { .. }),
            "{err}"
        );
        assert!(!expected.exists());

        // Пакета нет в индексе — PkgNotFound (код 3).
        let err2 = repo2
            .fetch_recipe(&cache, &index, "x", "9.9")
            .unwrap_err();
        assert!(matches!(err2, RepoError::PkgNotFound { .. }), "{err2}");

        fs::remove_dir_all(cache.parent().unwrap()).unwrap();
    }
}
