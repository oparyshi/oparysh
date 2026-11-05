//! Генератор `index.toml` по каталогу рецептов (этап 4.8 ROADMAP, раздел 4 DOCS).
//!
//! Обходит `<root>/recipes/<name>/<ver>/pkg.toml`, сверяет наличие тарбола
//! в `<root>/dist/<name>/<ver>/` (ровно один файл) и вычисляет
//! `recipe_sha` / `src_sha256` (sha256). Результат — [`RepoIndex`],
//! готовый к сериализации в `index.toml`.
//!
//! Записи `[[pkg]]` сортируются по (name, version); `deps` = `run` + `build`
//! (без дублей, сначала run); `provides`/`conflicts` зеркалят рецепт.
//! `tags` в рецепте отсутствуют, поэтому в индекс не попадают.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use oparysh_core::index::{RepoIndex, RepoMeta, RepoPkg};
use oparysh_core::recipe::Recipe;
use oparysh_core::version::Version;
use oparysh_verifier::sha256_file;

use crate::{DIST_DIR, RECIPE_FILE, RECIPES_DIR};

/// Параметры генерации индекса.
#[derive(Debug, Clone)]
pub struct GenIndexOptions {
    /// Имя репозитория (`[repo] name`).
    pub name: String,
    /// Origin репозитория (`[repo] .origin`), например `https://repo.example.com/stable`.
    pub origin: Option<String>,
    /// Переопределить `updated` (unix-секунды); по умолчанию — текущее время.
    pub updated: Option<u64>,
}

impl GenIndexOptions {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            origin: None,
            updated: None,
        }
    }
}

/// Ошибки генерации индекса.
#[derive(Debug, Error)]
pub enum GenIndexError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("не удалось вычислить sha256 для {path}: {message}")]
    Sha256 { path: String, message: String },

    #[error("не удалось сериализовать индекс: {0}")]
    Render(String),

    #[error("не удалось разобрать рецепт {path}: {message}")]
    RecipeParse { path: String, message: String },

    #[error("отсутствует каталог recipes/ ({path})")]
    RecipesMissing { path: String },

    #[error("в dist/ нет тарбола для {name}:{version} (каталог {path})")]
    DistMissing {
        name: String,
        version: String,
        path: String,
    },

    #[error("несколько файлов в dist/ для {name}:{version} (каталог {path})")]
    AmbiguousDist {
        name: String,
        version: String,
        path: String,
    },
}

/// Сгенерировать индекс по каталогу репозитория.
pub fn generate_index(root: &Path, options: &GenIndexOptions) -> Result<RepoIndex, GenIndexError> {
    let recipes_root = root.join(RECIPES_DIR);
    let mut recipe_files: Vec<PathBuf> = Vec::new();

    if !recipes_root.is_dir() {
        return Err(GenIndexError::RecipesMissing {
            path: recipes_root.display().to_string(),
        });
    }
    collect_recipes(&recipes_root, &mut recipe_files)?;

    let mut pkgs: Vec<RepoPkg> = Vec::with_capacity(recipe_files.len());
    for recipe_path in recipe_files {
        pkgs.push(build_entry(root, &recipe_path)?);
    }

    pkgs.sort_by(|a, b| {
        a.name.cmp(&b.name).then_with(|| {
            match (Version::parse(&a.version), Version::parse(&b.version)) {
                (Ok(va), Ok(vb)) => {
                    (va.major, va.minor, va.patch).cmp(&(vb.major, vb.minor, vb.patch))
                }
                _ => a.version.cmp(&b.version),
            }
        })
    });

    Ok(RepoIndex {
        repo: RepoMeta {
            name: options.name.clone(),
            origin: options.origin.clone(),
            updated: options.updated.unwrap_or_else(now_unix),
        },
        pkg: pkgs,
    })
}

/// Сериализовать индекс в TOML (`index.toml`).
pub fn render_index(index: &RepoIndex) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(index)
}

/// Сгенерировать `index.toml` и записать его в `output`.
/// Возвращает сгенерированный индекс.
pub fn write_index(
    root: &Path,
    options: &GenIndexOptions,
    output: &Path,
) -> Result<RepoIndex, GenIndexError> {
    let index = generate_index(root, options)?;
    let rendered = render_index(&index).map_err(|e| GenIndexError::Render(e.to_string()))?;
    fs::write(output, rendered)?;
    Ok(index)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn collect_recipes(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), GenIndexError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_recipes(&path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some(RECIPE_FILE) {
            out.push(path);
        }
    }
    Ok(())
}

fn sha256_hex(path: &Path) -> Result<String, GenIndexError> {
    sha256_file(path).map_err(|e| GenIndexError::Sha256 {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

fn build_entry(root: &Path, recipe_path: &Path) -> Result<RepoPkg, GenIndexError> {
    let raw = fs::read_to_string(recipe_path)?;
    let recipe = Recipe::parse(&raw).map_err(|message| GenIndexError::RecipeParse {
        path: recipe_path.display().to_string(),
        message,
    })?;

    let name = recipe.pkg.name.clone();
    let version = recipe.pkg.version.clone();

    let recipe_sha = sha256_hex(recipe_path)?;
    let dist = single_dist_file(root, &name, &version)?;
    let src_sha256 = sha256_hex(&dist)?;

    // deps: run, затем build; дубли убираем, порядок сохраняем.
    let mut seen = BTreeSet::new();
    let deps = recipe
        .deps
        .run
        .iter()
        .chain(recipe.deps.build.iter())
        .filter(|d| seen.insert(d.as_str()))
        .cloned()
        .collect();

    Ok(RepoPkg {
        name,
        version,
        build: recipe.pkg.build,
        arch: recipe.pkg.arch.clone(),
        desc: recipe.pkg.desc.clone(),
        tags: Vec::new(),
        deps,
        provides: recipe.provides.r#virtual.clone(),
        conflicts: recipe.conflicts.pkgs.clone(),
        recipe_sha,
        src_sha256,
    })
}

fn single_dist_file(root: &Path, name: &str, version: &str) -> Result<PathBuf, GenIndexError> {
    let dir = root.join(DIST_DIR).join(name).join(version);
    let mut files: Vec<PathBuf> = Vec::new();
    match fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                if entry.path().is_file() {
                    files.push(entry.path());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(GenIndexError::DistMissing {
                name: name.to_string(),
                version: version.to_string(),
                path: dir.display().to_string(),
            })
        }
        Err(e) => return Err(GenIndexError::Io(e)),
    }

    match files.len() {
        0 => Err(GenIndexError::DistMissing {
            name: name.to_string(),
            version: version.to_string(),
            path: dir.display().to_string(),
        }),
        1 => Ok(files.pop().expect("len == 1")),
        _ => Err(GenIndexError::AmbiguousDist {
            name: name.to_string(),
            version: version.to_string(),
            path: dir.display().to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oparysh_genindex_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join(RECIPES_DIR)).unwrap();
        fs::create_dir_all(dir.join(DIST_DIR)).unwrap();
        dir
    }

    fn write_pkg(root: &Path, name: &str, version: &str, extra_deps: &str) {
        let dir = root.join(RECIPES_DIR).join(name).join(version);
        fs::create_dir_all(&dir).unwrap();
        let recipe = format!(
            "[pkg]\nname = \"{name}\"\nversion = \"{version}\"\nbuild = 1\narch = [\"x86_64\"]\ndesc = \"test pkg {name}\"\n\n[source]\nurl = \"https://example.com/{name}-{version}.tar.gz\"\nsha256 = \"deadbeef\"\n\n[deps]\nrun = [\"zlib>=1.2\"{extra_deps}]\nbuild = [\"gcc>=11\", \"zlib>=1.2\"]\n\n[provides]\nvirtual = [\"{name}-virtual\"]\n\n[conflicts]\npkgs = [\"old-{name}<1\"]\n\n[build]\nsystem = \"make-only\"\n"
        );
        fs::write(dir.join(RECIPE_FILE), recipe).unwrap();
        let dist = root.join(DIST_DIR).join(name).join(version);
        fs::create_dir_all(&dist).unwrap();
        fs::write(dist.join(format!("{name}-{version}.tar.gz")), b"fake tarball").unwrap();
    }

    #[test]
    fn generates_index_sorted_and_hashed() {
        let root = temp_repo("ok");
        write_pkg(&root, "beta", "1.0.0", "");
        write_pkg(&root, "alpha", "2.0.0", "");

        let mut opts = GenIndexOptions::new("stable");
        opts.origin = Some("https://repo.example.com/stable".to_string());
        opts.updated = Some(1770000000);

        let index = generate_index(&root, &opts).unwrap();
        assert_eq!(index.pkg.len(), 2);
        assert_eq!(index.pkg[0].name, "alpha");
        assert_eq!(index.pkg[1].name, "beta");
        assert_eq!(index.repo.name, "stable");
        assert_eq!(index.repo.updated, 1770000000);

        let alpha = &index.pkg[0];
        // run + build, дубли убраны, сначала run
        assert_eq!(alpha.deps, vec!["zlib>=1.2", "gcc>=11"]);
        assert_eq!(alpha.provides, vec!["alpha-virtual"]);
        assert_eq!(alpha.conflicts, vec!["old-alpha<1"]);
        assert_eq!(alpha.recipe_sha.len(), 64);
        assert_eq!(alpha.src_sha256.len(), 64);

        let rendered = render_index(&index).unwrap();
        let parsed = RepoIndex::parse(&rendered).unwrap();
        assert_eq!(parsed, index);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_dist_is_error() {
        let root = temp_repo("missing_dist");
        write_pkg(&root, "alpha", "1.0.0", "");
        fs::remove_dir_all(root.join(DIST_DIR).join("alpha")).unwrap();

        let opts = GenIndexOptions::new("stable");
        let err = generate_index(&root, &opts).unwrap_err();
        assert!(matches!(err, GenIndexError::DistMissing { .. }));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn ambiguous_dist_is_error() {
        let root = temp_repo("ambiguous_dist");
        write_pkg(&root, "alpha", "1.0.0", "");
        let dist = root.join(DIST_DIR).join("alpha").join("1.0.0");
        fs::write(dist.join("second.tar.gz"), b"another").unwrap();

        let opts = GenIndexOptions::new("stable");
        let err = generate_index(&root, &opts).unwrap_err();
        assert!(matches!(err, GenIndexError::AmbiguousDist { .. }));

        fs::remove_dir_all(&root).ok();
    }
}
