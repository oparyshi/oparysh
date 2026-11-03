//! Ошибки уровня базы данных (этап 3 ROADMAP).
//!
//! Код возврата **8** (DOCS §11) используется при блокировке advisory-lock.

use thiserror::Error;

/// Все ошибки работы с локальной БД oparysh.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("ошибка SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("не удалось захватить advisory-lock на {path}: {source}")]
    Lock {
        path: String,
        source: std::io::Error,
    },

    /// База уже заблокирована другим процессом oparysh — код возврата **8**.
    #[error("база данных заблокирована другим процессом oparysh (код возврата 8)")]
    Locked,

    #[error("миграция #{version} не удалась: {message}")]
    Migration {
        version: u32,
        message: String,
    },

    #[error("пакет не найден: {name}")]
    NotFound { name: String },

    #[error("ошибка создания резервной копии: {0}")]
    Backup(String),

    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
}

/// Специализированный Result для oparysh-db.
pub type DbResult<T> = Result<T, DbError>;
