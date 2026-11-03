//! Advisory-lock на файл базы (этап 3.4 ROADMAP, DOCS §3.2).
//!
//! Используется `flock(LOCK_EX | LOCK_NB)` на файле `/var/lib/oparysh/lock`.
//! Неудача — ошибка с кодом возврата **8** (DOCS §11).

use crate::error::{DbError, DbResult};
use libc;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;

/// Exclusive advisory lock, автоматически снимается при drop.
#[derive(Debug)]
pub struct AdvisoryLock {
    _file: File,
    path: String,
}

impl AdvisoryLock {
    /// Захватить эксклюзивную блокировку на файле `path` (создаёт файл, если нет).
    pub fn acquire(path: &Path) -> DbResult<Self> {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| DbError::Lock {
                path: path.display().to_string(),
                source: e,
            })?;

        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return Err(DbError::Locked);
        }

        Ok(AdvisoryLock {
            _file: file,
            path: path.display().to_string(),
        })
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self._file.as_raw_fd(), libc::LOCK_UN) };
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn acquire_and_release_lock() {
        let dir = env::temp_dir().join(format!("oparysh_db_lock_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join("lock");

        {
            let _lock = AdvisoryLock::acquire(&lock_path).unwrap();
            // Второй попытка должна вернуть Locked
            let err = AdvisoryLock::acquire(&lock_path).unwrap_err();
            assert!(matches!(err, DbError::Locked));
        }

        // После drop блокировка снимается, можно захватить снова
        let _lock2 = AdvisoryLock::acquire(&lock_path).unwrap();

        fs::remove_dir_all(&dir).unwrap();
    }
}
