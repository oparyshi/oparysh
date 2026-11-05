//! Ошибки fetcher'а (этап 6 ROADMAP, DOCS §8–9, §11).

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Ошибки загрузки / распаковки / наложения патчей.
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("ошибка ввода-вывода ({path}): {source}")]
    Io {
        path: PathBuf,
        source: io::Error,
    },
    #[error("сеть недоступна или загрузка не удалась ({url}): {source}")]
    Network {
        url: String,
        source: ureq::Error,
    },
    #[error("sha256 mismatch для {path}: ожидалось {expected}, получено {actual}")]
    ShaMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("отсутствует обязательный sha256 для источника {name}")]
    MissingSha { name: String },
    #[error("источник не задан в рецепте и не найден в репозитории ({name})")]
    SourceNotSpecified { name: String },
    #[error("тарбол репозитория недоступен ({detail})")]
    DistUnavailable { detail: String },
    #[error("не удалось определить формат архива {path}")]
    UnknownArchive { path: PathBuf },
    #[error("архив содержит небезопасный путь: {path}")]
    UnsafePath { path: String },
    #[error("патч {file} не применился (код {code}):\n{output}")]
    PatchFailed {
        file: String,
        code: i32,
        output: String,
    },
    #[error("не удалось запустить `patch`: {source}")]
    PatchSpawn { source: io::Error },
    #[error("патч {file}: небезопасное имя файла")]
    UnsafePatchName { file: String },
}

impl FetchError {
    /// Код возврата по DOCS §11.
    pub fn exit_code(&self) -> i32 {
        match self {
            FetchError::Network { .. } => 5,
            FetchError::ShaMismatch { .. } | FetchError::MissingSha { .. } => 6,
            FetchError::PatchFailed { .. } | FetchError::PatchSpawn { .. } => 7,
            _ => 1,
        }
    }
}

impl FetchError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> FetchError {
        let path = path.into();
        move |source| FetchError::Io { path, source }
    }
}
