//! oparysh-repo: чтение репозитория-каталога (этап 4.1 ROADMAP, раздел 4 DOCS).
//!
//! Репозиторий — статический каталог (локальный или раздаваемый по HTTP(S)):
//!
//! ```text
//! <root>/
//! ├── index.toml
//! ├── recipes/<name>/<ver>/pkg.toml
//! └── dist/<name>/<ver>/<name>-<ver>.tar.xz
//! ```
//!
//! Здесь реализовано чтение **локального** репозитория-каталога: разбор
//! `index.toml` (типы — `oparysh-core::index`), проверка наличия `recipes/`
//! и `dist/`, поиск рецепта по `name`/`version` и сверка `recipe_sha` /
//! `src_sha256` с индексом (коды возврата 3 / 6 по DOCS §11).

use std::fs;
use std::path::{Path, PathBuf};

use oparysh_core::index::{RepoIndex, RepoPkg};
use oparysh_verifier::sha256_file;

mod error;

pub use error::{RepoError, RepoResult};

mod net;

pub use net::{
    cached_index_path, repo_cache_dir, CachedRepo, NetworkRepo, REPO_CACHE_DIR,
};

mod priority;

pub use priority::{RepoRef, RepoSet, RepoSource};

pub const INDEX_FILE: &str = "index.toml";
pub const RECIPES_DIR: &str = "recipes";
pub const DIST_DIR: &str = "dist";
pub const RECIPE_FILE: &str = "pkg.toml";

/// Локальный репозиторий-каталог: `index.toml` + `recipes/` + `dist/`.
#[derive(Debug, Clone)]
pub struct LocalRepo {
    /// Корень репозитория.
    root: PathBuf,
    /// Разобранный индекс.
    index: RepoIndex,
    /// Путь к файлу index.toml (для сообщений об ошибках).
    index_path: PathBuf,
}

impl LocalRepo {
    /// Выглядит ли каталог как репозиторий (`index.toml` на месте).
    pub fn is_repo(root: &Path) -> bool {
        root.join(INDEX_FILE).is_file()
    }

    /// Открыть репозиторий-каталог: прочитать и разобрать `index.toml`,
    /// убедиться, что `recipes/` и `dist/` существуют.
    pub fn open(root: &Path) -> RepoResult<Self> {
        let index_path = root.join(INDEX_FILE);
        if !index_path.is_file() {
            return Err(RepoError::NotARepo {
                path: root.display().to_string(),
                message: format!("отсутствует {INDEX_FILE}"),
            });
        }

        for sub in [RECIPES_DIR, DIST_DIR] {
            let dir = root.join(sub);
            if !dir.is_dir() {
                return Err(RepoError::NotARepo {
                    path: root.display().to_string(),
                    message: format!("отсутствует каталог {sub}/"),
                });
            }
        }

        let raw = fs::read_to_string(&index_path)?;
        let index =
            RepoIndex::parse(&raw).map_err(|source| RepoError::IndexParse {
                path: index_path.display().to_string(),
                source,
            })?;

        Ok(LocalRepo {
            root: root.to_path_buf(),
            index,
            index_path,
        })
    }

    /// Корень репозитория.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Разобранный индекс репозитория.
    pub fn index(&self) -> &RepoIndex {
        &self.index
    }

    /// Путь к index.toml.
    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    /// Все записи `[[pkg]]` индекса.
    pub fn packages(&self) -> &[RepoPkg] {
        &self.index.pkg
    }

    /// Записи пакета по имени (все версии).
    pub fn pkg_versions(&self, name: &str) -> Vec<&RepoPkg> {
        self.index
            .pkg
            .iter()
            .filter(|p| p.name == name)
            .collect()
    }

    /// Найти конкретную запись по имени и версии.
    pub fn pkg(&self, name: &str, version: &str) -> Result<&RepoPkg, RepoError> {
        self.index
            .pkg
            .iter()
            .find(|p| p.name == name && p.version == version)
            .ok_or(RepoError::PkgNotFound {
                name: name.to_string(),
                version: version.to_string(),
            })
    }

    /// Путь рецепта `recipes/<name>/<ver>/pkg.toml`.
    pub fn recipe_path(&self, name: &str, version: &str) -> PathBuf {
        self.root
            .join(RECIPES_DIR)
            .join(name)
            .join(version)
            .join(RECIPE_FILE)
    }

    /// Каталог тарболов `dist/<name>/<ver>/`.
    pub fn dist_dir(&self, name: &str, version: &str) -> PathBuf {
        self.root.join(DIST_DIR).join(name).join(version)
    }

    /// Тарбол источника в `dist/<name>/<ver>/`. В каталоге ожидается ровно
    /// один файл (например `<name>-<ver>.tar.xz`).
    pub fn dist_file(&self, name: &str, version: &str) -> RepoResult<PathBuf> {
        let dir = self.dist_dir(name, version);
        let mut files: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                files.push(entry.path());
            }
        }
        let path = match files.len() {
            1 => files.pop().expect("len == 1"),
            _ => {
                return Err(RepoError::AmbiguousDist {
                    name: name.to_string(),
                    version: version.to_string(),
                    path: dir.display().to_string(),
                })
            }
        };
        Ok(path)
    }

    /// Прочитать рецепт пакета и сверить его sha256 с `recipe_sha` из индекса.
    pub fn read_recipe(&self, name: &str, version: &str) -> RepoResult<String> {
        let pkg = self.pkg(name, version)?;
        let recipe = self.recipe_path(name, version);
        if !recipe.is_file() {
            return Err(RepoError::RecipeMissing {
                name: name.to_string(),
                version: version.to_string(),
                path: recipe.display().to_string(),
            });
        }

        let content = fs::read(&recipe)?;
        let actual = match sha256_file(&recipe) {
            Ok(h) => h,
            Err(_) => "?".to_string(),
        };
        if !actual.eq_ignore_ascii_case(&pkg.recipe_sha) {
            return Err(RepoError::RecipeShaMismatch {
                path: recipe.display().to_string(),
                expected: pkg.recipe_sha.clone(),
                actual,
            });
        }

        // Хэш сверён — содержимое возвращаем lossy, чтобы не ронять клиент
        // на нестандартной кодировке.
        Ok(String::from_utf8_lossy(&content).into_owned())
    }

    /// Проверить тарбол источника пакета против `src_sha256` из индекса.
    pub fn verify_source(&self, name: &str, version: &str) -> RepoResult<()> {
        let pkg = self.pkg(name, version)?;
        let dir = self.dist_dir(name, version);
        if !dir.is_dir() || self.dist_file(name, version).is_err() {
            return Err(RepoError::DistMissing {
                name: name.to_string(),
                version: version.to_string(),
                path: dir.display().to_string(),
            });
        }
        let path = self.dist_file(name, version)?;
        let actual = sha256_file(&path).map_err(|_| RepoError::SourceShaMismatch {
            path: path.display().to_string(),
            expected: pkg.src_sha256.clone(),
            actual: "?".to_string(),
        })?;
        if actual.eq_ignore_ascii_case(&pkg.src_sha256) {
            Ok(())
        } else {
            Err(RepoError::SourceShaMismatch {
                path: path.display().to_string(),
                expected: pkg.src_sha256.clone(),
                actual,
            })
        }
    }

    /// Проверить весь репозиторий: для каждой записи `[[pkg]]` сверяются
    /// `recipe_sha` рецепта и `src_sha256` тарбола.
    pub fn verify(&self) -> RepoResult<()> {
        for pkg in &self.index.pkg {
            self.read_recipe(&pkg.name, &pkg.version)?;
            self.verify_source(&pkg.name, &pkg.version)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oparysh_verifier::sha256_file;
    use sha2::{Digest, Sha256};

    /// Хэш содержимого строки (для fixture-файлов).
    fn sha_str(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    /// Мини-репозиторий с одним пакетом hello-1.0: recipes/, dist/ и корректные
    /// recipe_sha/src_sha256 для index.toml.
    fn fixture() -> (PathBuf, String, String) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("oparysh_repo_local_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let recipe = "[pkg]\nname = \"hello\"\nversion = \"1.0\"\n";
        write_file(
            &dir.join("recipes/hello/1.0/pkg.toml"),
            recipe.as_bytes(),
        );
        write_file(&dir.join("dist/hello/1.0/hello-1.0.tar.xz"), b"sources");

        let mut hasher = Sha256::new();
        hasher.update(recipe.as_bytes());
        let recipe_sha = hex::encode(hasher.finalize());
        let src_sha = sha256_file(&dir.join("dist/hello/1.0/hello-1.0.tar.xz")).unwrap();
        (dir, recipe_sha, src_sha)
    }

    /// index.toml с одной записью hello-1.0 и заданными хэшами.
    fn write_index(dir: &Path, recipe_sha: &str, src_sha: &str) {
        write(
            &dir.join(INDEX_FILE),
            &format!(
                "[repo]\nname = \"test\"\n\n[[pkg]]\nname = \"hello\"\nversion = \"1.0\"\n\
                 recipe_sha = \"{recipe_sha}\"\nsrc_sha256 = \"{src_sha}\"\n"
            ),
        );
    }

    #[test]
    fn open_ok() {
        let (dir, recipe_sha, src_sha) = fixture();
        write_index(&dir, &recipe_sha, &src_sha);

        let repo = LocalRepo::open(&dir).unwrap();
        assert_eq!(repo.packages().len(), 1);
        let pkg = repo.pkg("hello", "1.0").unwrap();
        assert_eq!(pkg.recipe_sha, recipe_sha);
        assert!(matches!(
            repo.pkg("hello", "9.9"),
            Err(RepoError::PkgNotFound { .. })
        ));
        assert_eq!(repo.pkg_versions("hello").len(), 1);
        assert_eq!(
            repo.recipe_path("hello", "1.0"),
            dir.join("recipes/hello/1.0/pkg.toml")
        );
        assert_eq!(
            repo.dist_dir("hello", "1.0"),
            dir.join("dist/hello/1.0")
        );
        assert!(repo.dist_file("hello", "1.0").is_ok());
        assert_eq!(sha_str(&repo.read_recipe("hello", "1.0").unwrap()), recipe_sha);
        assert!(LocalRepo::is_repo(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn not_a_repo() {
        let dir = std::env::temp_dir().join(format!("oparysh_repo_norepo_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // нет index.toml
        assert!(matches!(
            LocalRepo::open(&dir),
            Err(RepoError::NotARepo { .. })
        ));
        // index.toml есть, но нет recipes/
        write(&dir.join(INDEX_FILE), "[repo]\nname = \"t\"\n");
        assert!(matches!(
            LocalRepo::open(&dir),
            Err(RepoError::NotARepo { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn broken_toml() {
        let (dir, _, _) = fixture();
        write(&dir.join(INDEX_FILE), "[[[не TOML");
        assert!(matches!(
            LocalRepo::open(&dir),
            Err(RepoError::IndexParse { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn verify_ok_and_sha_mismatch() {
        let (dir, recipe_sha, src_sha) = fixture();
        write_index(&dir, &recipe_sha, &src_sha);
        let repo = LocalRepo::open(&dir).unwrap();
        assert!(repo.read_recipe("hello", "1.0").is_ok());
        assert!(repo.verify_source("hello", "1.0").is_ok());
        assert!(repo.verify().is_ok());

        // неверный recipe_sha
        write_index(&dir, "deadbeef", &src_sha);
        let repo = LocalRepo::open(&dir).unwrap();
        assert!(matches!(
            repo.read_recipe("hello", "1.0"),
            Err(RepoError::RecipeShaMismatch { .. })
        ));

        // неверный src_sha256
        write_index(&dir, &recipe_sha, "deadbeef");
        let repo = LocalRepo::open(&dir).unwrap();
        assert!(matches!(
            repo.verify_source("hello", "1.0"),
            Err(RepoError::SourceShaMismatch { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ambiguous_dist() {
        let (dir, recipe_sha, src_sha) = fixture();
        write_file(
            &dir.join("dist/hello/1.0/hello-1.0-other.tar.xz"),
            b"other",
        );
        write_index(&dir, &recipe_sha, &src_sha);
        let repo = LocalRepo::open(&dir).unwrap();
        assert!(matches!(
            repo.dist_file("hello", "1.0"),
            Err(RepoError::AmbiguousDist { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_recipe_and_dist() {
        let (dir, recipe_sha, src_sha) = fixture();
        write_index(&dir, &recipe_sha, &src_sha);
        // нет pkg.toml
        fs::remove_file(dir.join("recipes/hello/1.0/pkg.toml")).unwrap();
        let repo = LocalRepo::open(&dir).unwrap();
        assert!(matches!(
            repo.read_recipe("hello", "1.0"),
            Err(RepoError::RecipeMissing { .. })
        ));

        // нет тарбола
        write(&dir.join("recipes/hello/1.0/pkg.toml"), "[pkg]\n");
        fs::remove_dir_all(dir.join("dist")).unwrap();
        assert!(matches!(
            repo.verify_source("hello", "1.0"),
            Err(RepoError::DistMissing { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }
}
