//! Реальный источник пакетов для резолвера поверх `[repositories]` конфига
//! (этап 5.1 ROADMAP, DOCS §7 п.3).
//!
//! Кандидаты собираются в порядке приоритета репозиториев: локальные —
//! открытием каталога, сетевые — по кэшу индексов (`oparysh sync`);
//! сетевой репозиторий без кэша пропускается (как в `oparysh info`/`list`).
//! Рецепты читаются с проверкой хэшей: локальные — `recipe_sha` из индекса,
//! сетевые — из кэша или скачиваются с проверкой (`NetworkRepo::fetch_recipe`).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use oparysh_core::config::RepoEntry;
use oparysh_core::recipe::Recipe;
use oparysh_core::version::Version;
use oparysh_repo::{CachedRepo, LocalRepo, NetworkRepo, RepoSet, RepoSource};

use crate::collect::{Candidate, PackageSource};
use crate::error::{ResolveError, ResolveResult};

/// Источник пакетов поверх настроенных репозиториев.
pub struct RepoPackageSource {
    set: RepoSet,
    cache_dir: PathBuf,
}

impl RepoPackageSource {
    /// Построить по записям `[repositories]` конфига и каталогу кэша.
    pub fn new(entries: &[RepoEntry], cache_dir: &Path) -> ResolveResult<Self> {
        Ok(RepoPackageSource {
            set: RepoSet::from_entries(entries)?,
            cache_dir: cache_dir.to_path_buf(),
        })
    }

    /// Известный размер тарбола источника для выбранного кандидата.
    ///
    /// Локальные репозитории читают размер файла из `dist/`; сетевые до
    /// скачивания обычно неизвестны (`None`), поэтому план честно показывает
    /// размер только для локальных источников.
    pub fn source_size(&self, candidate: &Candidate) -> Option<u64> {
        match self.source_of(&candidate.repo)? {
            RepoSource::Local(root) => LocalRepo::open(root).ok()?.dist_file(
                &candidate.name,
                &candidate.version_str,
            ).ok()?.metadata().ok().map(|m| m.len()),
            RepoSource::Network(_) => None,
        }
    }

    fn source_of(&self, repo_name: &str) -> Option<&RepoSource> {
        self.set.find(repo_name).map(|r| &r.source)
    }
}

impl PackageSource for RepoPackageSource {
    fn candidates(&self, name: &str) -> ResolveResult<Vec<Candidate>> {
        let mut out = Vec::new();
        for r in self.set.refs() {
            match &r.source {
                RepoSource::Local(root) => {
                    let repo = LocalRepo::open(root)?;
                    for p in repo.pkg_versions(name) {
                        out.push(candidate(&r.name, p)?);
                    }
                }
                RepoSource::Network(_) => {
                    // Сетевой репо без кэша пропускаем — индекса просто нет.
                    let cached = match CachedRepo::open(&self.cache_dir, &r.name) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    for p in cached.packages().iter().filter(|p| p.name == name) {
                        out.push(candidate(&r.name, p)?);
                    }
                }
            }
        }
        Ok(out)
    }

    fn providers(&self, virtual_name: &str) -> ResolveResult<Vec<Candidate>> {
        let mut out = Vec::new();
        for r in self.set.refs() {
            let packages: Vec<oparysh_core::index::RepoPkg> = match &r.source {
                RepoSource::Local(root) => LocalRepo::open(root)?.packages().to_vec(),
                RepoSource::Network(_) => match CachedRepo::open(&self.cache_dir, &r.name) {
                    Ok(c) => c.index.pkg.clone(),
                    Err(_) => continue,
                },
            };
            for p in &packages {
                // Читаем рецепт (с проверкой хэша) и смотрим provides.virtual.
                let recipe = match self.recipe(&Candidate {
                    name: p.name.clone(),
                    version: Version::parse(&p.version)?,
                    version_str: p.version.clone(),
                    build: p.build,
                    repo: r.name.clone(),
                }) {
                    Ok(r) => r,
                    Err(_) => continue, // рецепт без хэша/битый пропускаем
                };
                if recipe
                    .provides
                    .r#virtual
                    .iter()
                    .any(|v| v == virtual_name)
                {
                    out.push(Candidate {
                        name: p.name.clone(),
                        version: Version::parse(&p.version)?,
                        version_str: p.version.clone(),
                        build: p.build,
                        repo: r.name.clone(),
                    });
                }
            }
        }
        Ok(out)
    }

    fn recipe(&self, candidate: &Candidate) -> ResolveResult<Recipe> {
        let ver = &candidate.version_str;
        let raw = match self.source_of(&candidate.repo) {
            Some(RepoSource::Local(root)) => {
                LocalRepo::open(root)?.read_recipe(&candidate.name, ver)?
            }
            Some(RepoSource::Network(url)) => {
                let cached = CachedRepo::open(&self.cache_dir, &candidate.repo)?;
                let cached_recipe = oparysh_repo::cached_index_path(&self.cache_dir, &candidate.repo)
                    .parent()
                    .expect("у index_path есть родитель")
                    .join(oparysh_repo::RECIPES_DIR)
                    .join(&candidate.name)
                    .join(ver)
                    .join(oparysh_repo::RECIPE_FILE);
                if cached_recipe.is_file() {
                    read_and_verify(&cached, &cached_recipe, &candidate.name, ver)?
                } else {
                    // Скачать и проверить recipe_sha (внутри fetch_recipe).
                    NetworkRepo::new(url.clone()).fetch_recipe(
                        &self.cache_dir,
                        &cached.index,
                        &candidate.name,
                        ver,
                    )?;
                    read_and_verify(&cached, &cached_recipe, &candidate.name, ver)?
                }
            }
            None => {
                return Err(ResolveError::Unresolved {
                    req: candidate.name.clone(),
                    reason: format!("репозиторий {} не найден в [repositories]", candidate.repo),
                })
            }
        };

        Recipe::parse(&raw).map_err(|message| ResolveError::RecipeParse {
            name: candidate.name.clone(),
            version: ver.clone(),
            message,
        })
    }
}

fn candidate(
    repo: &str,
    p: &oparysh_core::index::RepoPkg,
) -> ResolveResult<Candidate> {
    Ok(Candidate {
        name: p.name.clone(),
        version: Version::parse(&p.version)?,
        version_str: p.version.clone(),
        build: p.build,
        repo: repo.to_string(),
    })
}

/// Прочитать закэшированный рецепт и сверить sha256 с записью индекса.
fn read_and_verify(
    cached: &CachedRepo,
    path: &Path,
    name: &str,
    version: &str,
) -> ResolveResult<String> {
    let raw = std::fs::read_to_string(path).map_err(oparysh_repo::RepoError::from)?;
    let expected = cached
        .index
        .pkg
        .iter()
        .find(|p| p.name == name && p.version == version)
        .map(|p| p.recipe_sha.as_str())
        .unwrap_or("");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(oparysh_repo::RepoError::RecipeShaMismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        }
        .into());
    }
    Ok(raw)
}
