//! oparysh-resolver: построение графа зависимостей, топологическая
//! сортировка, выбор версий (этап 5 ROADMAP, DOCS §7).
//!
//! Реализовано:
//! - [`collect::collect_requirements`] / [`collect::resolve`] — сбор
//!   требований: рекурсивное раскрытие `deps.build` и `deps.run` начиная с
//!   целей (5.1);
//! - [`installed::InstalledSource`] — учёт установленных пакетов:
//!   удовлетворённые требования → узлы «уже выполнено» (5.2);
//! - выбор версии `latest-satisfying` по приоритету репозиториев (5.3);
//! - виртуальные пакеты (`provides.virtual`): выбор поставщика
//!   (установленный > `--provider` > единственный), иначе
//!   [`ResolveError::AmbiguousProvider`] для интерактивного выбора (5.4);
//! - проверка `conflicts` и обнаружение build-циклов; runtime-цикл
//!   разрешён (5.5);
//! - [`plan::build_order`] / [`plan::pending_build_order`] — топологическая
//!   сортировка (алгоритм Кана), порядок сборки (5.6);
//! - [`plan::render_plan`] — план перед работами и подтверждение через CLI
//!   (5.7);
//! - [`repo_source::RepoPackageSource`] — источник пакетов поверх
//!   `[repositories]` конфига (локальные репозитории и кэш индексов).

pub mod collect;
pub mod error;
pub mod installed;
pub mod plan;
pub mod repo_source;

#[cfg(test)]
mod tests;

pub use collect::{
    collect_requirements, resolve, select_latest_satisfying, Candidate, CollectedNode, DepEdge,
    DepKind, PackageSource, RequirementSet, ResolveOptions,
};
pub use error::{ResolveError, ResolveResult};
pub use installed::{InstalledPkg, InstalledSource};
pub use plan::{build_order, format_bytes, pending_build_order, render_plan};
pub use repo_source::RepoPackageSource;
