//! oparysh-resolver: построение графа зависимостей, топологическая
//! сортировка, выбор версий (этап 5 ROADMAP, DOCS §7).
//!
//! Пока реализовано:
//! - [`collect::collect_requirements`] — сбор требований: рекурсивное
//!   раскрытие `deps.build` и `deps.run` начиная с целей (этап 5.1);
//! - [`repo_source::RepoPackageSource`] — источник пакетов поверх
//!   `[repositories]` конфига (локальные репозитории и кэш индексов).
//!
//! Учёт установленных (5.2), стратегия выбора версии (5.3), виртуальные
//! пакеты (5.4), конфликты и циклы (5.5), топологическая сортировка (5.6) —
//! следующие этапы.

pub mod collect;
pub mod error;
pub mod repo_source;

#[cfg(test)]
mod tests;

pub use collect::{
    collect_requirements, Candidate, CollectedNode, DepEdge, DepKind, PackageSource,
    RequirementSet,
};
pub use error::{ResolveError, ResolveResult};
pub use repo_source::RepoPackageSource;
