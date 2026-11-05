//! Ошибки oparysh-repo: чтение локальных/сетевых индексов репозиториев (этап 4 ROADMAP).
//!
//! Несовпадение `recipe_sha`/`src_sha256` — код возврата **6** (sha256 mismatch, DOCS §11),
//! отсутствие пакета в индексе — код **3** (пакет не найден).

use thiserror::Error;

/// Все ошибки чтения репозитория-каталога.
#[derive(Debug, Error)]
pub enum RepoError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("не удалось разобрать index.toml репозитория {path}: {source}")]
    IndexParse {
        path: String,
        source: toml::de::Error,
    },

    #[error("{path} — не каталог репозитория: {message}")]
    NotARepo { path: String, message: String },

    #[error("пакет не найден в индексе: {name}:{version}")]
    PkgNotFound { name: String, version: String },

    #[error("пакеты не найдены в индексе: {name}")]
    NameNotFound { name: String },

    /// Репозиторий с таким именем не настроен — код **3** (категория
    /// «не найдено», DOCS §11).
    #[error("репозиторий не найден в [repositories]: {name}")]
    RepoNotFound { name: String },

    #[error("нет файла рецепта для {name}:{version} (ожидался {path})")]
    RecipeMissing {
        name: String,
        version: String,
        path: String,
    },

    #[error("в dist/ нет тарбола для {name}:{version} (каталог {path})")]
    DistMissing {
        name: String,
        version: String,
        path: String,
    },

    /// recipe_sha рецепта не совпал с индексом — код возврата **6**.
    #[error("sha256 mismatch для {path}: ожидалось {expected}, получено {actual}")]
    RecipeShaMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    /// src_sha256 тарбола не совпал с индексом — код возврата **6**.
    #[error("sha256 mismatch для {path}: ожидалось {expected}, получено {actual}")]
    SourceShaMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("несколько файлов в dist/ для {name}:{version} (каталог {path})")]
    AmbiguousDist { name: String, version: String, path: String },

    /// Сбой HTTP-запроса (сеть, таймаут, статус не 2xx) — по DOCS §11 это
    /// код возврата **5** (ошибка загрузки).
    #[error("не удалось скачать {url}: {source}")]
    Network { url: String, source: ureq::Error },

    /// Имя репозитория, полученное из index.toml, нельзя использовать
    /// как имя каталога кэша.
    #[error("некорректное имя репозитория: {name}")]
    BadRepoName { name: String },

    /// Запись `[[repositories.entries]]` конфига некорректна (например, нет
    /// ни `path`, ни `url`).
    #[error("некорректная запись репозитория {name} в [repositories]: {message}")]
    EntryInvalid { name: String, message: String },

    /// Некорректный шаблон поиска (битый glob/regex) — код возврата **2**
    /// (неверные аргументы CLI, DOCS §11).
    #[error("некорректный шаблон поиска «{pattern}»: {message}")]
    BadPattern { pattern: String, message: String },
}

/// Специализированный Result для oparysh-repo.
pub type RepoResult<T> = Result<T, RepoError>;
