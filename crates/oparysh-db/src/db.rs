//! Локальная база installed-пакетов (этап 3 ROADMAP, DOCS §3).
//!
//! Схема (DOCS §3.1):
//!   packages, files, deps, build_info + индекс idx_files_pkg
//! Миграции (DOCS §3.2): таблица _migrations, нумерованные SQL-миграции
//! DAO (DOCS §3.3): insert/get/list/delete package, batch insert files/deps

use crate::error::{DbError, DbResult};
use crate::lock::AdvisoryLock;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Путь по умолчанию к файлу advisory-lock.
const DEFAULT_LOCK_NAME: &str = "lock";

/// Счётчик для уникальных имён тестовых директорий (чтобы тесты не конфликтовали на advisory-lock).
#[cfg(test)]
static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Текущая схема БД (одна миграция — создание всех таблиц).
const MIGRATION_1: &str = r#"
-- Установленные пакеты
CREATE TABLE IF NOT EXISTS packages (
    name        TEXT PRIMARY KEY,
    version     TEXT NOT NULL,
    build_rev   INTEGER NOT NULL,
    arch        TEXT NOT NULL,
    pkg_hash    TEXT NOT NULL,
    install_ts  INTEGER NOT NULL,
    size_bytes  INTEGER NOT NULL
);

-- Список файлов пакета (для корректного удаления)
CREATE TABLE IF NOT EXISTS files (
    pkg         TEXT NOT NULL REFERENCES packages(name) ON DELETE CASCADE,
    path        TEXT NOT NULL,       -- абсолютный, канонический
    sha256      TEXT NOT NULL        -- хэш содержимого на момент установки
);
CREATE INDEX IF NOT EXISTS idx_files_pkg ON files(pkg);

-- Рёбра графа зависимостей (снимок на момент установки)
CREATE TABLE IF NOT EXISTS deps (
    pkg         TEXT NOT NULL REFERENCES packages(name) ON DELETE CASCADE,
    dep_name    TEXT NOT NULL,
    req         TEXT NOT NULL        -- исходное требование: ">=3.2, <4"
);

-- Флаги сборки, зафиксированные при установке
CREATE TABLE IF NOT EXISTS build_info (
    pkg         TEXT PRIMARY KEY REFERENCES packages(name) ON DELETE CASCADE,
    cc          TEXT NOT NULL,
    cflags      TEXT NOT NULL,
    makeopts    TEXT NOT NULL,
    profile     TEXT NOT NULL,       -- имя профиля из oparysh.conf
    cmd         TEXT NOT NULL        -- точная команда сборки
);

-- Таблица версий миграций
CREATE TABLE IF NOT EXISTS _migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL
);
"#;

/// Миграция 2 (этап 9.3): причина установки пакета — `explicit` (установлен
/// явно) или `dependency` (поставлен как зависимость). Нужна для `autoremove`.
const MIGRATION_2: &str = r#"
ALTER TABLE packages ADD COLUMN reason TEXT NOT NULL DEFAULT 'explicit';
"#;

/// Миграция 3 (этап 10.3): журнал событий — установки/обновления/понижения/удаления.
const MIGRATION_3: &str = r#"
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    action      TEXT NOT NULL,       -- install | upgrade | downgrade | remove | autoremove
    pkg         TEXT NOT NULL,
    version     TEXT NOT NULL,
    build_rev   INTEGER NOT NULL DEFAULT 0,
    detail      TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_events_pkg ON events(pkg);
"#;

/// Установленный пакет (таблица `packages`).
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub build_rev: u32,
    pub arch: String,
    pub pkg_hash: String,
    pub install_ts: i64,
    pub size_bytes: i64,
    /// Причина установки: `explicit` или `dependency` (этап 9.3, autoremove).
    pub reason: String,
}

/// Файл пакета (таблица `files`).
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub pkg: String,
    pub path: String,
    pub sha256: String,
}

/// Ребро зависимости (таблица `deps`).
#[derive(Debug, Clone)]
pub struct Dep {
    pub pkg: String,
    pub dep_name: String,
    pub req: String,
}

/// Запись журнала событий (таблица `events`, этап 10.3).
#[derive(Debug, Clone)]
pub struct Event {
    pub id: i64,
    pub ts: i64,
    /// install | upgrade | downgrade | remove | autoremove
    pub action: String,
    pub pkg: String,
    pub version: String,
    pub build_rev: u32,
    pub detail: String,
}

/// Информация о сборке (таблица `build_info`).
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub pkg: String,
    pub cc: String,
    pub cflags: String,
    pub makeopts: String,
    pub profile: String,
    pub cmd: String,
}

/// Локальная база oparysh (обёртка над rusqlite::Connection + advisory-lock).
pub struct Database {
    conn: Connection,
    _lock: AdvisoryLock,
    db_dir: PathBuf,
}

impl Database {
    /// Открыть (или создать) базу в `db_dir` с advisory-lock.
    ///
    /// `db_dir` — корень базы (по умолчанию `/var/lib/oparysh`).
    /// Файл `db.sqlite` и `lock` создаются внутри `db_dir`.
    pub fn open(db_dir: &Path) -> DbResult<Self> {
        std::fs::create_dir_all(db_dir).map_err(|e| DbError::Io(e))?;

        let db_path = db_dir.join("db.sqlite");
        let lock_path = db_dir.join(DEFAULT_LOCK_NAME);

        let _lock = AdvisoryLock::acquire(&lock_path)?;

        let conn = Connection::open(&db_path).map_err(DbError::Sqlite)?;

        // Включаем foreign keys для ON DELETE CASCADE
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(DbError::Sqlite)?;

        let mut db = Database {
            conn,
            _lock,
            db_dir: db_dir.to_path_buf(),
        };

        db.run_migrations()?;

        Ok(db)
    }

    /// Вернуть путь к директории базы.
    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    /// Запустить все pending миграции.
    fn run_migrations(&mut self) -> DbResult<()> {
        // Все таблицы используют IF NOT EXISTS, поэтому повторный вызов безопасен.
        self.conn.execute_batch(MIGRATION_1).map_err(DbError::Sqlite)?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO _migrations (version, applied_at) VALUES (1, ?1)",
                params![now_ts()],
            )
            .map_err(DbError::Sqlite)?;

        // Миграция 2: колонка packages.reason (этап 9.3).
        let applied2: Option<i64> = self
            .conn
            .query_row("SELECT version FROM _migrations WHERE version = 2", [], |r| r.get(0))
            .optional()
            .map_err(DbError::Sqlite)?;
        if applied2.is_none() {
            let has_reason: bool = self
                .conn
                .prepare("SELECT reason FROM packages LIMIT 0")
                .map(|_| true)
                .unwrap_or(false);
            if !has_reason {
                self.conn.execute_batch(MIGRATION_2).map_err(DbError::Sqlite)?;
            }
            self.conn
                .execute(
                    "INSERT INTO _migrations (version, applied_at) VALUES (2, ?1)",
                    params![now_ts()],
                )
                .map_err(DbError::Sqlite)?;
        }

        // Миграция 3: таблица events (этап 10.3).
        let applied3: Option<i64> = self
            .conn
            .query_row("SELECT version FROM _migrations WHERE version = 3", [], |r| r.get(0))
            .optional()
            .map_err(DbError::Sqlite)?;
        if applied3.is_none() {
            self.conn.execute_batch(MIGRATION_3).map_err(DbError::Sqlite)?;
            self.conn
                .execute(
                    "INSERT INTO _migrations (version, applied_at) VALUES (3, ?1)",
                    params![now_ts()],
                )
                .map_err(DbError::Sqlite)?;
        }

        Ok(())
    }

    // ━━━━━━━━━━━━━━━ DAO: packages ━━━━━━━━━━━━━━━

    /// Вставить новый пакет.
    pub fn insert_package(&mut self, pkg: &Package) -> DbResult<()> {
        self.conn.execute(
            "INSERT INTO packages (name, version, build_rev, arch, pkg_hash, install_ts, size_bytes, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pkg.name,
                pkg.version,
                pkg.build_rev as i64,
                pkg.arch,
                pkg.pkg_hash,
                pkg.install_ts,
                pkg.size_bytes,
                pkg.reason,
            ],
        )?;
        Ok(())
    }

    /// Получить пакет по имени.
    pub fn get_package(&self, name: &str) -> DbResult<Option<Package>> {
        let row = self
            .conn
            .query_row(
                "SELECT name, version, build_rev, arch, pkg_hash, install_ts, size_bytes, reason
                 FROM packages WHERE name = ?1",
                params![name],
                |row| {
                    Ok(Package {
                        name: row.get(0)?,
                        version: row.get(1)?,
                        build_rev: row.get::<_, i64>(2)? as u32,
                        arch: row.get(3)?,
                        pkg_hash: row.get(4)?,
                        install_ts: row.get(5)?,
                        size_bytes: row.get(6)?,
                        reason: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::Sqlite)?;
        Ok(row)
    }

    /// Список всех установленных пакетов.
    pub fn list_packages(&self) -> DbResult<Vec<Package>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name, version, build_rev, arch, pkg_hash, install_ts, size_bytes, reason
                 FROM packages ORDER BY name",
            )
            .map_err(DbError::Sqlite)?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Package {
                    name: row.get(0)?,
                    version: row.get(1)?,
                    build_rev: row.get::<_, i64>(2)? as u32,
                    arch: row.get(3)?,
                    pkg_hash: row.get(4)?,
                    install_ts: row.get(5)?,
                    size_bytes: row.get(6)?,
                    reason: row.get(7)?,
                })
            })
            .map_err(DbError::Sqlite)?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(DbError::Sqlite)?);
        }
        Ok(out)
    }

    /// Удалить пакет (каскадно удаляются files, deps, build_info через ON DELETE CASCADE).
    pub fn delete_package(&mut self, name: &str) -> DbResult<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM packages WHERE name = ?1", params![name])
            .map_err(DbError::Sqlite)?;
        Ok(changed > 0)
    }

    /// Записать установленный пакет целиком ОДНОЙ транзакцией (этап 8.3):
    /// `packages` + `files` + `deps` + `build_info`. Если пакет с тем же
    /// именем уже был установлен, его строки удаляются в той же транзакции
    /// (reinstall через каскад ON DELETE).
    pub fn commit_package(
        &mut self,
        pkg: &Package,
        files: &[FileEntry],
        deps: &[Dep],
        info: Option<&BuildInfo>,
    ) -> DbResult<()> {
        let tx = self.conn.transaction().map_err(DbError::Sqlite)?;
        tx.execute("DELETE FROM packages WHERE name = ?1", params![pkg.name])
            .map_err(DbError::Sqlite)?;
        tx.execute(
            "INSERT INTO packages (name, version, build_rev, arch, pkg_hash, install_ts, size_bytes, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pkg.name,
                pkg.version,
                pkg.build_rev as i64,
                pkg.arch,
                pkg.pkg_hash,
                pkg.install_ts,
                pkg.size_bytes,
                pkg.reason,
            ],
        )
        .map_err(DbError::Sqlite)?;
        for f in files {
            tx.execute(
                "INSERT INTO files (pkg, path, sha256) VALUES (?1, ?2, ?3)",
                params![f.pkg, f.path, f.sha256],
            )
            .map_err(DbError::Sqlite)?;
        }
        for d in deps {
            tx.execute(
                "INSERT INTO deps (pkg, dep_name, req) VALUES (?1, ?2, ?3)",
                params![d.pkg, d.dep_name, d.req],
            )
            .map_err(DbError::Sqlite)?;
        }
        if let Some(info) = info {
            tx.execute(
                "INSERT INTO build_info (pkg, cc, cflags, makeopts, profile, cmd)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![info.pkg, info.cc, info.cflags, info.makeopts, info.profile, info.cmd],
            )
            .map_err(DbError::Sqlite)?;
        }
        tx.commit().map_err(DbError::Sqlite)?;
        Ok(())
    }

    // ━━━━━━━━━━━━━━━ DAO: files ━━━━━━━━━━━━━━━

    /// Вставить список файлов пакета одним批量.
    pub fn insert_files(&mut self, _pkg: &str, files: &[FileEntry]) -> DbResult<()> {
        let tx = self.conn.transaction().map_err(DbError::Sqlite)?;
        for f in files {
            tx.execute(
                "INSERT INTO files (pkg, path, sha256) VALUES (?1, ?2, ?3)",
                params![f.pkg, f.path, f.sha256],
            )
            .map_err(DbError::Sqlite)?;
        }
        tx.commit().map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Получить все файлы пакета.
    pub fn get_files(&self, pkg: &str) -> DbResult<Vec<FileEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT pkg, path, sha256 FROM files WHERE pkg = ?1 ORDER BY path")
            .map_err(DbError::Sqlite)?;
        let rows = stmt
            .query_map(params![pkg], |row| {
                Ok(FileEntry {
                    pkg: row.get(0)?,
                    path: row.get(1)?,
                    sha256: row.get(2)?,
                })
            })
            .map_err(DbError::Sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(DbError::Sqlite)?);
        }
        Ok(out)
    }

    // ━━━━━━━━━━━━━━━ DAO: deps ━━━━━━━━━━━━━━━

    /// Вставить рёбра зависимости пакета скопом.
    pub fn insert_deps(&mut self, _pkg: &str, deps: &[Dep]) -> DbResult<()> {
        let tx = self.conn.transaction().map_err(DbError::Sqlite)?;
        for d in deps {
            tx.execute(
                "INSERT INTO deps (pkg, dep_name, req) VALUES (?1, ?2, ?3)",
                params![d.pkg, d.dep_name, d.req],
            )
            .map_err(DbError::Sqlite)?;
        }
        tx.commit().map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Получить зависимости пакета.
    pub fn get_deps(&self, pkg: &str) -> DbResult<Vec<Dep>> {
        let mut stmt = self
            .conn
            .prepare("SELECT pkg, dep_name, req FROM deps WHERE pkg = ?1 ORDER BY dep_name")
            .map_err(DbError::Sqlite)?;
        let rows = stmt
            .query_map(params![pkg], |row| {
                Ok(Dep {
                    pkg: row.get(0)?,
                    dep_name: row.get(1)?,
                    req: row.get(2)?,
                })
            })
            .map_err(DbError::Sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(DbError::Sqlite)?);
        }
        Ok(out)
    }

    // ━━━━━━━━━━━━━━━ DAO: rdeps ━━━━━━━━━━━━━━━

    /// Имена установленных пакетов, которые объявляют зависимость от `name`.
    pub fn reverse_deps(&self, name: &str) -> DbResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT pkg FROM deps WHERE dep_name = ?1 ORDER BY pkg")
            .map_err(DbError::Sqlite)?;
        let rows = stmt
            .query_map(params![name], |row| row.get::<_, String>(0))
            .map_err(DbError::Sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(DbError::Sqlite)?);
        }
        Ok(out)
    }

    // ━━━━━━━━━━━━━━━ DAO: events (этап 10.3) ━━━━━━━━━━━━━━━

    /// Записать событие в журнал (`events`).
    pub fn record_event(
        &self,
        action: &str,
        pkg: &str,
        version: &str,
        build_rev: u32,
        detail: &str,
    ) -> DbResult<()> {
        self.conn.execute(
            "INSERT INTO events (ts, action, pkg, version, build_rev, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![now_ts(), action, pkg, version, build_rev as i64, detail],
        )?;
        Ok(())
    }

    /// Прочитать журнал событий: все (`pkg = None`) или по одному пакету,
    /// от новых к старым.
    pub fn list_events(&self, pkg: Option<&str>) -> DbResult<Vec<Event>> {
        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<Event> {
            Ok(Event {
                id: row.get(0)?,
                ts: row.get(1)?,
                action: row.get(2)?,
                pkg: row.get(3)?,
                version: row.get(4)?,
                build_rev: row.get::<_, i64>(5)? as u32,
                detail: row.get(6)?,
            })
        };
        let mut out = Vec::new();
        match pkg {
            Some(name) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, ts, action, pkg, version, build_rev, detail
                     FROM events WHERE pkg = ?1 ORDER BY id DESC",
                ).map_err(DbError::Sqlite)?;
                let rows = stmt.query_map(params![name], map_row).map_err(DbError::Sqlite)?;
                for row in rows { out.push(row.map_err(DbError::Sqlite)?); }
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, ts, action, pkg, version, build_rev, detail
                     FROM events ORDER BY id DESC",
                ).map_err(DbError::Sqlite)?;
                let rows = stmt.query_map([], map_row).map_err(DbError::Sqlite)?;
                for row in rows { out.push(row.map_err(DbError::Sqlite)?); }
            }
        }
        Ok(out)
    }

    // ━━━━━━━━━━━━━━━ DAO: build_info ━━━━━━━━━━━━━━━

    /// Вставить информацию о сборке пакета.
    pub fn insert_build_info(&mut self, info: &BuildInfo) -> DbResult<()> {
        self.conn.execute(
            "INSERT INTO build_info (pkg, cc, cflags, makeopts, profile, cmd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![info.pkg, info.cc, info.cflags, info.makeopts, info.profile, info.cmd],
        )?;
        Ok(())
    }

    /// Получить информацию о сборке пакета.
    pub fn get_build_info(&self, pkg: &str) -> DbResult<Option<BuildInfo>> {
        let row = self
            .conn
            .query_row(
                "SELECT pkg, cc, cflags, makeopts, profile, cmd FROM build_info WHERE pkg = ?1",
                params![pkg],
                |row| {
                    Ok(BuildInfo {
                        pkg: row.get(0)?,
                        cc: row.get(1)?,
                        cflags: row.get(2)?,
                        makeopts: row.get(3)?,
                        profile: row.get(4)?,
                        cmd: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::Sqlite)?;
        Ok(row)
    }
}

/// Создать резервную копию `db.sqlite` → `db.sqlite.bak` (этап 3.6).
pub fn backup_db(db_dir: &Path) -> DbResult<PathBuf> {
    let src = db_dir.join("db.sqlite");
    let dst = db_dir.join("db.sqlite.bak");

    let src_conn = Connection::open(&src).map_err(DbError::Sqlite)?;
    let mut dst_conn = Connection::open(&dst).map_err(DbError::Sqlite)?;
    {
        let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
            .map_err(|e| DbError::Backup(format!("backup init: {e}")))?;
        loop {
            let result = backup
                .step(-1)
                .map_err(|e| DbError::Backup(format!("backup step: {e}")))?;
            if matches!(result, rusqlite::backup::StepResult::Done) {
                break;
            }
        }
    } // backup освобождает borrow dst_conn перед close()
    dst_conn.close().map_err(|(_, e)| DbError::Backup(format!("close dst: {e}")))?;

    Ok(dst)
}

/// Восстановить базу из `db.sqlite.bak` (команда `rebuild-db`, этап 3.6).
pub fn rebuild_db_from_backup(db_dir: &Path) -> DbResult<PathBuf> {
    let bak = db_dir.join("db.sqlite.bak");
    if !bak.exists() {
        return Err(DbError::Backup("db.sqlite.bak не найден".into()));
    }

    let db_file = db_dir.join("db.sqlite");
    std::fs::copy(&bak, &db_file).map_err(|e| DbError::Backup(format!("copy backup: {e}")))?;

    Ok(bak)
}

/// Текущее время в Unix timestamp (секунды).
fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn temp_db_dir() -> PathBuf {
        let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("oparysh_db_test_{}_{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn open_creates_schema_and_migrations() {
        let dir = temp_db_dir();
        let mut db = Database::open(&dir).unwrap();
        assert!(db.get_package("nonexistent").unwrap().is_none());

        // Миграция применена — таблица _migrations содержит версию 1
        let version: Option<u32> = db
            .conn
            .query_row("SELECT version FROM _migrations WHERE version = 1", [], |row| row.get(0))
            .optional()
            .unwrap();
        assert_eq!(version, Some(1));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn insert_and_get_package() {
        let dir = temp_db_dir();
        let mut db = Database::open(&dir).unwrap();

        let pkg = Package {
            name: "hello".into(),
            version: "1.0".into(),
            build_rev: 1,
            arch: "x86_64".into(),
            pkg_hash: "abcd".into(),
            install_ts: now_ts(),
            size_bytes: 12345,
            reason: "explicit".into(),
        };
        db.insert_package(&pkg).unwrap();

        let got = db.get_package("hello").unwrap().unwrap();
        assert_eq!(got.name, "hello");
        assert_eq!(got.version, "1.0");
        assert_eq!(got.build_rev, 1);

        // Несуществующий
        assert!(db.get_package("nope").unwrap().is_none());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn list_packages() {
        let dir = temp_db_dir();
        let mut db = Database::open(&dir).unwrap();

        db.insert_package(&Package {
            name: "zlib".into(),
            version: "1.3".into(),
            build_rev: 0,
            arch: "x86_64".into(),
            pkg_hash: "a".into(),
            install_ts: now_ts(),
            size_bytes: 1,
            reason: "dependency".into(),
        })
        .unwrap();
        db.insert_package(&Package {
            name: "gcc".into(),
            version: "14".into(),
            build_rev: 2,
            arch: "x86_64".into(),
            pkg_hash: "b".into(),
            install_ts: now_ts(),
            size_bytes: 2,
            reason: "explicit".into(),
        })
        .unwrap();

        let list = db.list_packages().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "gcc");
        assert_eq!(list[1].name, "zlib");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn delete_package_cascades() {
        let dir = temp_db_dir();
        let mut db = Database::open(&dir).unwrap();

        db.insert_package(&Package {
            name: "hello".into(),
            version: "1.0".into(),
            build_rev: 1,
            arch: "x86_64".into(),
            pkg_hash: "h".into(),
            install_ts: now_ts(),
            size_bytes: 10,
            reason: "explicit".into(),
        })
        .unwrap();

        db.insert_files(
            "hello",
            &[FileEntry {
                pkg: "hello".into(),
                path: "/usr/bin/hello".into(),
                sha256: "f1".into(),
            }],
        )
        .unwrap();
        db.insert_deps(
            "hello",
            &[Dep {
                pkg: "hello".into(),
                dep_name: "libc".into(),
                req: ">=2.0".into(),
            }],
        )
        .unwrap();
        db.insert_build_info(&BuildInfo {
            pkg: "hello".into(),
            cc: "gcc".into(),
            cflags: "-O2".into(),
            makeopts: "-j4".into(),
            profile: "release".into(),
            cmd: "make".into(),
        })
        .unwrap();

        // Проверяем, что всё на месте перед удалением
        assert_eq!(db.get_files("hello").unwrap().len(), 1);
        assert_eq!(db.get_deps("hello").unwrap().len(), 1);
        assert!(db.get_build_info("hello").unwrap().is_some());

        // Удаляем — каскадно должны удалиться files, deps, build_info
        assert!(db.delete_package("hello").unwrap());
        assert!(db.get_package("hello").unwrap().is_none());
        assert_eq!(db.get_files("hello").unwrap().len(), 0);
        assert_eq!(db.get_deps("hello").unwrap().len(), 0);
        assert!(db.get_build_info("hello").unwrap().is_none());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn backup_and_rebuild() {
        let dir = temp_db_dir();
        let mut db = Database::open(&dir).unwrap();

        db.insert_package(&Package {
            name: "hello".into(),
            version: "1.0".into(),
            build_rev: 1,
            arch: "x86_64".into(),
            pkg_hash: "h".into(),
            install_ts: now_ts(),
            size_bytes: 10,
            reason: "explicit".into(),
        })
        .unwrap();

        let bak = backup_db(&dir).unwrap();
        assert!(bak.exists());

        // Проверяем содержимое бэкапа
        let bak_conn = Connection::open(&bak).unwrap();
        let count: u32 = bak_conn
            .query_row("SELECT COUNT(*) FROM packages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // rebuild_db_from_backup
        let restored = rebuild_db_from_backup(&dir).unwrap();
        assert_eq!(restored, dir.join("db.sqlite.bak"));

        fs::remove_dir_all(&dir).unwrap();
    }
}
