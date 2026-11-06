//! oparysh-db: локальная база installed-пакетов на SQLite (этап 3 ROADMAP, DOCS §3).
//!
//! Схема: `packages`, `files`, `deps`, `build_info` + индекс `idx_files_pkg`.
//! Advisory-lock через `flock` на `/var/lib/oparysh/lock` (код возврата 8).
//! DAO: insert/get/list/delete пакетов, batch insert files/deps/build_info.
//! Резервное копирование `.bak` и `rebuild-db`.

pub mod db;
pub mod error;
pub mod lock;

pub use db::{BuildInfo, Database, Dep, Event, FileEntry, Package};
pub use error::{DbError, DbResult};
pub use lock::AdvisoryLock;
pub use db::rebuild_db_from_backup;
