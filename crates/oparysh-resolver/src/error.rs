//! Ошибки oparysh-resolver (этап 5 ROADMAP).
//!
//! Код **4** по DOCS §11 — «неудовлетворённые зависимости»; ошибки
//! разрешения (нет кандидата, конфликт версий) маппятся в него.
//! Ошибки репозитория/сети сохраняют свои коды (маппинг — на стороне CLI).

use thiserror::Error;

/// Ошибки разрешения зависимостей.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("ошибка репозитория: {0}")]
    Repo(#[from] oparysh_repo::RepoError),

    #[error("ошибка версии: {0}")]
    Version(#[from] oparysh_core::version::VersionError),

    #[error("не удалось разобрать зависимость «{raw}» (в {context}): {source}")]
    InvalidDep {
        raw: String,
        context: String,
        source: oparysh_core::version::VersionError,
    },

    #[error("не удалось разобрать рецепт {name} {version}: {message}")]
    RecipeParse {
        name: String,
        version: String,
        message: String,
    },

    /// Не найдено кандидатов, удовлетворяющих требованию — код **4**.
    #[error("неудовлетворённая зависимость {req}: {reason}")]
    Unresolved { req: String, reason: String },

    /// Ранее выбранная версия не удовлетворяет новому требованию — код **4**.
    #[error("конфликт версий {name}: выбрано {selected}, требуется {required}")]
    VersionConflict {
        name: String,
        selected: String,
        required: String,
    },

    /// Конфликт с `conflicts` другого пакета — код **4**.
    #[error("конфликт пакетов: {pkg} конфликтует с {conflicting} ({req})")]
    Conflict {
        pkg: String,
        conflicting: String,
        req: String,
    },

    /// Цикл по `deps.build` — код **4** (runtime-цикл разрешён).
    #[error("цикл build-зависимостей: {path}")]
    BuildCycle { path: String },

    /// Несколько пакетов предоставляют виртуальное имя — нужен выбор.
    #[error("неоднозначный поставщик виртуального пакета {virtual_name}: {}", providers.join(", "))]
    AmbiguousProvider {
        virtual_name: String,
        providers: Vec<String>,
    },
}

/// Специализированный Result для oparysh-resolver.
pub type ResolveResult<T> = Result<T, ResolveError>;
