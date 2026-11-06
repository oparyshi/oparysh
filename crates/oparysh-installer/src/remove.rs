//! Удаление установленных пакетов (этап 9 ROADMAP, DOCS §9).
//!
//!   9.1 Удаление по манифесту `files` — только собственные файлы пакета,
//!       путь должен быть абсолютным (каноническим на момент установки);
//!   9.2 Проверка обратных зависимостей: отказ без `force` с кодом **9**
//!       и списком пострадавших;
//!   9.3 `remove --force`, `autoremove` (пакеты, поставленные как зависимости,
//!       от которых никто из установленных не зависит);
//!   9.4 Очистка: каталог кэша рецепта `recipes/<name>/<ver>/` + каскадное
//!       удаление записей БД (`packages` → `files`/`deps`/`build_info`).

use oparysh_db::{Database, DbError};
use std::path::{Path, PathBuf};

/// Код возврата при отказе из-за обратных зависимостей (DOCS §11).
pub const REMOVE_BLOCKED_EXIT_CODE: i32 = 9;
/// Код возврата «пакет не найден» (DOCS §11).
pub const REMOVE_NOT_FOUND_EXIT_CODE: i32 = 3;
/// Ошибка БД (DOCS §11).
pub const REMOVE_DB_EXIT_CODE: i32 = 8;

/// Причина установки, записанная в `packages.reason`.
pub const REASON_EXPLICIT: &str = "explicit";
pub const REASON_DEPENDENCY: &str = "dependency";

/// Ошибка удаления.
#[derive(Debug)]
pub enum RemoveError {
    /// Пакет не установлен.
    NotInstalled { name: String },
    /// Есть установленные обратные зависимости; нужен `--force` (код 9).
    Blocked { name: String, rdeps: Vec<String> },
    /// Ошибка БД (код 8).
    Db(DbError),
    /// Ошибка файловой операции.
    Io { path: PathBuf, source: std::io::Error },
}

impl std::fmt::Display for RemoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoveError::NotInstalled { name } => {
                write!(f, "пакет `{name}` не установлен")
            }
            RemoveError::Blocked { name, rdeps } => write!(
                f,
                "отказ: пакет `{name}` нужен установленным пакетам: {}; используйте --force",
                rdeps.join(", ")
            ),
            RemoveError::Db(e) => write!(f, "БД: {e}"),
            RemoveError::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for RemoveError {}

impl From<DbError> for RemoveError {
    fn from(e: DbError) -> Self {
        RemoveError::Db(e)
    }
}

impl RemoveError {
    /// Код возврата по DOCS §11.
    pub fn exit_code(&self) -> i32 {
        match self {
            RemoveError::NotInstalled { .. } => REMOVE_NOT_FOUND_EXIT_CODE,
            RemoveError::Blocked { .. } => REMOVE_BLOCKED_EXIT_CODE,
            RemoveError::Db(_) => REMOVE_DB_EXIT_CODE,
            RemoveError::Io { .. } => 1,
        }
    }
}

fn io_err<T>(path: &Path, r: std::io::Result<T>) -> Result<T, RemoveError> {
    r.map_err(|source| RemoveError::Io { path: path.to_path_buf(), source })
}

/// Итог удаления одного пакета.
#[derive(Debug)]
pub struct RemoveOutcome {
    pub name: String,
    pub version: String,
    /// Сколько файлов удалено.
    pub files_removed: usize,
    /// Файлы из манифеста, которых уже не было на диске.
    pub missing: Vec<String>,
    /// Обратные зависимости, оставшиеся в системе (при `force`).
    pub forced_rdeps: Vec<String>,
    /// Удалён ли каталог кэша рецепта `recipes/<name>/<ver>/`.
    pub recipe_cache_removed: bool,
}

/// Удалить пакет по манифесту `files` (9.1, 9.2, 9.4).
///
/// Без `force` при наличии установленных обратных зависимостей возвращает
/// [`RemoveError::Blocked`] (код 9) и ничего не меняет.
/// Файлы, которых нет на диске, пропускаются (заносятся в `missing`).
/// Пустые родительские каталоги удаляются вплоть до `destroot` (не включая его).
pub fn remove_package(
    db: &mut Database,
    db_dir: &Path,
    destroot: &Path,
    name: &str,
    force: bool,
) -> Result<RemoveOutcome, RemoveError> {
    let pkg = db
        .get_package(name)?
        .ok_or_else(|| RemoveError::NotInstalled { name: name.to_string() })?;

    let rdeps = db.reverse_deps(name)?;
    if !rdeps.is_empty() && !force {
        return Err(RemoveError::Blocked { name: name.to_string(), rdeps });
    }

    let files = db.get_files(name)?;
    let mut files_removed = 0usize;
    let mut missing = Vec::new();
    for entry in &files {
        let path = Path::new(&entry.path);
        // 9.1: только абсолютные канонические пути из манифеста.
        if !path.is_absolute() {
            eprintln!("WARN: пропуск неабсолютного пути из манифеста: {}", entry.path);
            continue;
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) => {
                if meta.is_dir() {
                    // Манифест хранит только файлы; каталог с таким именем — чужой.
                    eprintln!("WARN: пропуск каталога из манифеста: {}", path.display());
                    continue;
                }
                io_err(path, std::fs::remove_file(path))?;
                files_removed += 1;
                prune_empty_dirs(path.parent(), destroot);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing.push(entry.path.clone());
            }
            Err(e) => {
                return Err(RemoveError::Io { path: path.to_path_buf(), source: e });
            }
        }
    }

    // 9.4: каскадное удаление записей БД (files/deps/build_info).
    db.delete_package(name)?;

    // 9.4: кэш рецепта `recipes/<name>/<ver>/` и пустой `recipes/<name>/`.
    let recipe_dir = db_dir.join("recipes").join(name).join(&pkg.version);
    let recipe_cache_removed = if recipe_dir.is_dir() {
        io_err(&recipe_dir, std::fs::remove_dir_all(&recipe_dir))?;
        true
    } else {
        false
    };
    let parent = db_dir.join("recipes").join(name);
    if parent.is_dir() {
        let _ = std::fs::remove_dir(&parent); // только если пуст
    }

    Ok(RemoveOutcome {
        name: pkg.name,
        version: pkg.version,
        files_removed,
        missing,
        forced_rdeps: if force { rdeps } else { Vec::new() },
        recipe_cache_removed,
    })
}

/// Удалить пустые родительские каталоги вплоть до `destroot` (не включая его).
fn prune_empty_dirs(start: Option<&Path>, destroot: &Path) {
    let destroot = destroot.canonicalize().unwrap_or_else(|_| destroot.to_path_buf());
    let mut dir = match start {
        Some(d) => d.to_path_buf(),
        None => return,
    };
    loop {
        let canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if canon == destroot || !canon.starts_with(&destroot) {
            return;
        }
        match std::fs::remove_dir(&dir) {
            Ok(()) => match dir.parent() {
                Some(p) => dir = p.to_path_buf(),
                None => return,
            },
            Err(_) => return, // не пуст или нет прав — останавливаемся
        }
    }
}

/// Кандидаты в «сироты» для autoremove: установленные пакеты с причиной
/// `dependency`, от которых не зависит ни один установленный пакет.
pub fn autoremove_candidates(db: &Database) -> Result<Vec<String>, RemoveError> {
    let pkgs = db.list_packages()?;
    let mut needed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in &pkgs {
        for dep in db.get_deps(&p.name)? {
            needed.insert(dep.dep_name);
        }
    }
    Ok(pkgs
        .into_iter()
        .filter(|p| p.reason == REASON_DEPENDENCY && !needed.contains(&p.name))
        .map(|p| p.name)
        .collect())
}

/// `autoremove` (9.3): удалить всех «сирот» до фиксированной точки
/// (удаление одного сироты может осиротить другой). Возвращает имена удалённых.
pub fn autoremove(
    db: &mut Database,
    db_dir: &Path,
    destroot: &Path,
) -> Result<Vec<RemoveOutcome>, RemoveError> {
    let mut removed = Vec::new();
    loop {
        let candidates = autoremove_candidates(db)?;
        if candidates.is_empty() {
            break;
        }
        for name in candidates {
            // У сироты по построению нет установленных обратных зависимостей.
            removed.push(remove_package(db, db_dir, destroot, &name, false)?);
        }
    }
    Ok(removed)
}
