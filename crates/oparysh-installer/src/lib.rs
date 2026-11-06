//! Установщик (этап 8 ROADMAP, DOCS §8 шаги 4–7).
//!
//! Пайплайн установки:
//!   8.1 Псевдо-`DESTDIR`: результат install-шагов собирается в
//!       `<db_dir>/tmp/<pkg>-<ver>-destdir/` (см. [`destdir`], [`prepare_destdir`]);
//!   8.2 manifest scan: обход DESTDIR, список файлов + sha256 каждого;
//!       файлы вне `install.prefix` считаются «незаявленными» —
//!       предупреждение (`[files].unknown_warn = true`) либо ошибка
//!       (флаг `--strict`, см. [`scan_manifest`]);
//!   8.3 commit: перенос файлов в `destroot` (DOCS §6, `general.destroot`)
//!       и запись `packages` + `files` + `deps` + `build_info` в БД
//!       **единой транзакцией** ([`commit_package`] →
//!       [`oparysh_db::Database::commit_package`]);
//!   8.4 кэш рецепта в `recipes/<name>/<ver>/pkg.toml` и cleanup
//!       (`build/`, `tmp/`), пропускаемый при `--keep-build`
//!       (см. [`cache_recipe`], [`cleanup`]).

pub mod remove;
pub use remove::{
    REMOVE_BLOCKED_EXIT_CODE, REMOVE_DB_EXIT_CODE, REMOVE_NOT_FOUND_EXIT_CODE, RemoveError,
    RemoveOutcome, REASON_DEPENDENCY, REASON_EXPLICIT, autoremove, autoremove_candidates,
    remove_package,
};

use oparysh_core::recipe::Recipe;
use oparysh_core::version::DepReq;
use oparysh_db::{BuildInfo, Database, Dep, FileEntry, Package};
use oparysh_verifier::sha256_file;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Коды возврата установщика (DOCS §11): ошибки БД — 8, остальное — 1.
pub const DB_ERROR_EXIT_CODE: i32 = 8;
pub const INSTALL_ERROR_EXIT_CODE: i32 = 1;

/// Ошибка установщика.
#[derive(Debug)]
pub enum InstallError {
    /// Операции с файлами: создание каталогов, перенос, удаление.
    Io { path: PathBuf, source: std::io::Error },
    /// Незаявленные файлы при `--strict` (8.2).
    UnknownFiles { unknown: Vec<String> },
    /// DESTDIR пуст — нечего устанавливать.
    EmptyDestdir { path: PathBuf },
    /// Ошибка БД (`DbError`, код возврата 8).
    Db(oparysh_db::DbError),
    /// SHA-256 файла при обходе DESTDIR.
    Hash { path: PathBuf, source: oparysh_verifier::VerifierError },
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            InstallError::UnknownFiles { unknown } => write!(
                f,
                "незаявленные файлы вне install.prefix ({}): {}",
                unknown.len(),
                unknown.join(", ")
            ),
            InstallError::EmptyDestdir { path } => {
                write!(f, "DESTDIR {} пуст — нечего устанавливать", path.display())
            }
            InstallError::Db(e) => write!(f, "БД: {e}"),
            InstallError::Hash { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<oparysh_db::DbError> for InstallError {
    fn from(e: oparysh_db::DbError) -> Self {
        InstallError::Db(e)
    }
}

impl InstallError {
    /// Код возврата по DOCS §11.
    pub fn exit_code(&self) -> i32 {
        match self {
            InstallError::Db(_) => DB_ERROR_EXIT_CODE,
            _ => INSTALL_ERROR_EXIT_CODE,
        }
    }
}

fn io_err<T>(path: &Path, r: std::io::Result<T>) -> Result<T, InstallError> {
    r.map_err(|source| InstallError::Io { path: path.to_path_buf(), source })
}

// ━━━━━━━━━━━━━━━━━━ 8.1: Псево-DESTDIR ━━━━━━━━━━━━━━━━━━

/// Каталог псевдо-DESTDIR: `<db_dir>/tmp/<name>-<ver>-destdir/` (DOCS §3, §8).
pub fn destdir(db_dir: &Path, name: &str, version: &str) -> PathBuf {
    db_dir.join("tmp").join(format!("{name}-{version}-destdir"))
}

/// Создать (с возможной очисткой) каталог DESTDIR для новой сборки.
/// Существующий каталог удаляется вместе с содержимым.
pub fn prepare_destdir(path: &Path) -> Result<(), InstallError> {
    if path.exists() {
        io_err(path, std::fs::remove_dir_all(path))?;
    }
    io_err(path, std::fs::create_dir_all(path))?;
    Ok(())
}

// ━━━━━━━━━━━━━━━━━━ 8.2: manifest scan ━━━━━━━━━━━━━━━━━━

/// Один файл манифеста.
#[derive(Debug, Clone)]
pub struct ManifestEntry {
    /// Путь относительно DESTDIR (как `make install` его записал).
    pub rel: String,
    /// Исходный путь внутри DESTDIR.
    pub src: PathBuf,
    /// Куда файл будет перенесён (`destroot` + `rel`).
    pub dest: PathBuf,
    /// SHA-256 содержимого на момент установки.
    pub sha256: String,
    /// Размер в байтах.
    pub size: u64,
}

/// Результат manifest scan (8.2).
#[derive(Debug, Default)]
pub struct Manifest {
    /// Все файлы DESTDIR, отсортированные по `rel`.
    pub files: Vec<ManifestEntry>,
    /// «Незаявленные» файлы — вне `install.prefix`.
    pub unknown: Vec<String>,
    /// Содержит ли DESTDIR хотя бы один файл.
    pub is_empty: bool,
}

impl Manifest {
    /// Суммарный размер всех файлов.
    pub fn size_bytes(&self) -> i64 {
        self.files.iter().map(|f| f.size as i64).sum()
    }
}

/// Режим реакции на незаявленные файлы.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanPolicy {
    /// `[files].unknown_warn` из рецепта — предупреждать о незаявленных файлах.
    pub unknown_warn: bool,
    /// `--strict` — незаявленные файлы = ошибка.
    pub strict: bool,
}

/// Обход DESTDIR: список файлов + sha256 каждого (8.2).
///
/// Файл считается «незаявленным», если его путь относительно DESTDIR не
/// начинается с `install.prefix` рецепта — то есть `make install` записал
/// его мимо заявленного дерева установки (например, при `--prefix=/opt/x`
/// и `install.prefix="/usr/local"`).
///
/// При `strict` незаявленные файлы → `InstallError::UnknownFiles`;
/// иначе при `unknown_warn` они печатаются в stderr; иначе игнорируются.
/// Сам манифест включает все файлы независимо от их статуса.
pub fn scan_manifest(
    destdir_path: &Path,
    destroot: &Path,
    prefix: &str,
    policy: ScanPolicy,
) -> Result<Manifest, InstallError> {
    let prefix_rel = prefix.trim_matches('/');
    let mut manifest = Manifest::default();
    walk(destdir_path, destdir_path, destroot, prefix_rel, &mut manifest)?;
    manifest.files.sort_by(|a, b| a.rel.cmp(&b.rel));
    manifest.unknown.sort();
    manifest.is_empty = manifest.files.is_empty();

    if !manifest.unknown.is_empty() {
        if policy.strict {
            return Err(InstallError::UnknownFiles {
                unknown: manifest.unknown.clone(),
            });
        }
        if policy.unknown_warn {
            for path in &manifest.unknown {
                eprintln!("WARN: незаявленный файл вне {prefix}: {path}");
            }
        }
    }
    Ok(manifest)
}

/// Рекурсивный обход DESTDIR.
fn walk(
    root: &Path,
    dir: &Path,
    destroot: &Path,
    prefix_rel: &str,
    manifest: &mut Manifest,
) -> Result<(), InstallError> {
    for entry in io_err(dir, std::fs::read_dir(dir))? {
        let entry = io_err(dir, entry)?;
        let path = entry.path();
        let meta = io_err(&path, std::fs::symlink_metadata(&path))?;
        let rel_path = path
            .strip_prefix(root)
            .expect("entry — потомок root");
        let rel_str = rel_path.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");

        if meta.file_type().is_symlink() || meta.is_file() {
            let claimed = prefix_rel.is_empty()
                || rel_str.starts_with(&format!("{prefix_rel}/"))
                || rel_str == prefix_rel;
            let entry = ManifestEntry {
                rel: rel_str,
                src: path.clone(),
                dest: destroot.join(rel_path),
                sha256: manifest_sha(&path, meta.file_type().is_symlink())?,
                size: if meta.is_file() { meta.len() } else { 0 },
            };
            if !claimed {
                manifest.unknown.push(entry.rel.clone());
            }
            manifest.files.push(entry);
        } else if meta.is_dir() {
            walk(root, &path, destroot, prefix_rel, manifest)?;
        }
    }
    Ok(())
}

/// SHA-256 файла; для битого симлинка — `sha256(symlink-target-string)`.
fn manifest_sha(path: &Path, is_symlink: bool) -> Result<String, InstallError> {
    if is_symlink {
        if let Ok(data) = std::fs::read(path) {
            return Ok(hex_sha(&data));
        }
        let target = std::fs::read_link(path).unwrap_or_default();
        return Ok(hex_sha(target.as_os_str().as_encoded_bytes()));
    }
    sha256_file(path).map_err(|source| InstallError::Hash {
        path: path.to_path_buf(),
        source,
    })
}

fn hex_sha(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ━━━━━━━━━━━━━━━━━━ 8.3: commit ━━━━━━━━━━━━━━━━━━

/// Собранные флаги/шаги сборки — для записи `build_info`.
#[derive(Debug, Clone, Default)]
pub struct BuildRecord {
    pub cc: String,
    pub cflags: String,
    pub makeopts: String,
    pub profile: String,
    pub cmd: String,
}

/// Зависимости пакета на момент установки (снимок `deps`).
#[derive(Debug, Clone, Default)]
pub struct DepSnapshot {
    /// Требования запуска (`deps.run`) → таблица `deps`.
    pub run: Vec<String>,
    /// Требования сборки (`deps.build`) — не попадают в граф зависимостей.
    pub build: Vec<String>,
}

/// Итог commit (8.3).
#[derive(Debug)]
pub struct CommitOutcome {
    pub name: String,
    pub version: String,
    pub build_rev: u32,
    pub files: usize,
    pub size_bytes: i64,
}

/// Перенести файлы манифеста из DESTDIR в `destroot` (8.3).
///
/// Каталоги создаются по мере необходимости; существующий файл с тем же
/// путём заменяется. DESTDIR-каталог остаётся пустым после переноса
/// (вызывающий обычно вызывает [`cleanup`]).
///
/// Возвращает суммарный размер перенесённых файлов в байтах.
pub fn move_to_destroot(manifest: &Manifest) -> Result<i64, InstallError> {
    let mut total = 0u64;
    for file in &manifest.files {
        if let Some(parent) = file.dest.parent() {
            io_err(parent, std::fs::create_dir_all(parent))?;
        }
        // Заменяем существующий файл с тем же путём.
        if std::fs::symlink_metadata(&file.dest).is_ok() {
            io_err(&file.dest, std::fs::remove_file(&file.dest))?;
        }
        let src = file.src.clone();
        let dest = file.dest.clone();
        if io_err(&dest, std::fs::rename(&src, &dest)).is_err() {
            let is_symlink = std::fs::symlink_metadata(&src)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                let target = io_err(&src, std::fs::read_link(&src))?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &dest)
                    .map_err(|source| InstallError::Io { path: dest.clone(), source })?;
                #[cfg(not(unix))]
                io_err(&dest, std::fs::copy(&src, &dest))?;
                io_err(&src, std::fs::remove_file(&src))?;
            } else {
                io_err(&dest, std::fs::copy(&src, &dest))?;
                io_err(&src, std::fs::remove_file(&src))?;
            }
        }
        total += file.size;
    }
    Ok(total as i64)
}

/// Записать установленный пакет в БД единой транзакцией (8.3).
///
/// `pkg_hash` — sha256 рецепта (кэшируется в `packages.pkg_hash`).
/// Возвращает описание результата. Запись идёт через
/// [`Database::commit_package`] (packages + files + deps + build_info в одной
/// транзакции; существующие строки пакета перезаписываются каскадом).
pub fn commit_package(
    db: &mut Database,
    recipe: &Recipe,
    manifest: &Manifest,
    deps: &DepSnapshot,
    build: &BuildRecord,
    pkg_hash: &str,
    reason: &str,
) -> Result<CommitOutcome, InstallError> {
    commit_package_with_build_rev(db, recipe, manifest, deps, build, pkg_hash, reason, None)
}

/// Вариант [`commit_package`] с явным `build_rev` (этапы 10.1/10.2):
/// при пересборке той же версии `build_rev` увеличивается (+1), при
/// смене версии берётся `recipe.pkg.build`. `None` — как обычно, из рецепта.
#[allow(clippy::too_many_arguments)]
pub fn commit_package_with_build_rev(
    db: &mut Database,
    recipe: &Recipe,
    manifest: &Manifest,
    deps: &DepSnapshot,
    build: &BuildRecord,
    pkg_hash: &str,
    reason: &str,
    build_rev: Option<u32>,
) -> Result<CommitOutcome, InstallError> {
    let name = recipe.pkg.name.clone();

    let mut dep_rows: Vec<Dep> = Vec::with_capacity(deps.run.len());
    for spec in &deps.run {
        let (dep_name, req) = split_dep_spec(spec);
        dep_rows.push(Dep { pkg: name.clone(), dep_name, req });
    }
    let files: Vec<FileEntry> = manifest
        .files
        .iter()
        .map(|f| FileEntry {
            pkg: name.clone(),
            path: f.dest.display().to_string(),
            sha256: f.sha256.clone(),
        })
        .collect();
    let info = BuildInfo {
        pkg: name.clone(),
        cc: build.cc.clone(),
        cflags: build.cflags.clone(),
        makeopts: build.makeopts.clone(),
        profile: build.profile.clone(),
        cmd: build.cmd.clone(),
    };
    let pkg = Package {
        name: name.clone(),
        version: recipe.pkg.version.clone(),
        build_rev: build_rev.unwrap_or(recipe.pkg.build),
        arch: recipe.pkg.arch.first().cloned().unwrap_or_else(|| "any".to_string()),
        pkg_hash: pkg_hash.to_string(),
        install_ts: now_ts(),
        size_bytes: manifest.size_bytes(),
        reason: reason.to_string(),
    };
    db.commit_package(&pkg, &files, &dep_rows, Some(&info))?;

    Ok(CommitOutcome {
        name: pkg.name,
        version: pkg.version,
        build_rev: pkg.build_rev,
        files: manifest.files.len(),
        size_bytes: pkg.size_bytes,
    })
}

/// Разобрать зависимость из рецепта (`"zlib"`, `"pcre2>=10.32"`,
/// `"name@ver"`): вернуть (имя, req или "" ).
fn split_dep_spec(spec: &str) -> (String, String) {
    let spec = spec.trim();
    let normalized = match spec.rsplit_once('@') {
        Some((name, ver)) if !name.is_empty() && !ver.is_empty() => format!("{name}={ver}"),
        _ => spec.to_string(),
    };
    match DepReq::parse(&normalized) {
        Ok(d) => (d.name, d.req.map(|r| r.to_string()).unwrap_or_default()),
        Err(_) => (spec.to_string(), String::new()),
    }
}

fn now_ts() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// ━━━━━━━━━━━━━━━━━━ 8.4: кэш рецепта и cleanup ━━━━━━━━━━━━━━━━━━

/// Сохранить рецепт: `<db_dir>/recipes/<name>/<ver>/pkg.toml` (DOCS §3).
/// Возвращает путь записи.
pub fn cache_recipe(
    db_dir: &Path,
    name: &str,
    version: &str,
    recipe_toml: &str,
) -> Result<PathBuf, InstallError> {
    let path = db_dir.join("recipes").join(name).join(version).join("pkg.toml");
    if let Some(parent) = path.parent() {
        io_err(parent, std::fs::create_dir_all(parent))?;
    }
    io_err(&path, std::fs::write(&path, recipe_toml))?;
    Ok(path)
}

/// Отчёт cleanup (8.4).
#[derive(Debug, Default)]
pub struct CleanupOutcome {
    /// Удалён ли каталог сборки `<db_dir>/build/<name>-<ver>/`.
    pub build_removed: bool,
    /// Удалён ли каталог DESTDIR `<db_dir>/tmp/<name>-<ver>-destdir/`.
    pub destdir_removed: bool,
}

/// Удалить рабочие каталоги (8.4): `build/<name>-<ver>/` (пропускается при
/// `keep_build`, т.е. `general.keep_build = true` или `--keep-build`) и
/// `tmp/<name>-<ver>-destdir/` (всегда, после успешного переноса).
pub fn cleanup(
    db_dir: &Path,
    name: &str,
    version: &str,
    keep_build: bool,
) -> Result<CleanupOutcome, InstallError> {
    let mut out = CleanupOutcome::default();
    if !keep_build {
        let build_path = db_dir.join("build").join(format!("{name}-{version}"));
        if build_path.is_dir() {
            io_err(&build_path, std::fs::remove_dir_all(&build_path))?;
            out.build_removed = true;
        }
    }
    let destdir_path = destdir(db_dir, name, version);
    if destdir_path.is_dir() {
        io_err(&destdir_path, std::fs::remove_dir_all(&destdir_path))?;
        out.destdir_removed = true;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
