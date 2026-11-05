//! Установленные пакеты как источник данных для резолвера (этап 5.2 ROADMAP,
//! DOCS §7 п.2).
//!
//! Если установленный пакет удовлетворяет требованию, он включается в граф
//! как уже выполненный узел (`installed = true`) и его зависимости повторно
//! не раскрываются. Реализация для локальной БД — [`Database`] из `oparysh-db`.

use oparysh_core::version::Version;

/// Сведения об установленном пакете.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPkg {
    pub name: String,
    pub version: Version,
    pub version_str: String,
    pub build_rev: u32,
}

/// Источник сведений об установленных пакетах.
pub trait InstalledSource {
    /// Установленный пакет с данным именем, если есть.
    fn installed(&self, name: &str) -> Option<InstalledPkg>;
}

impl InstalledSource for oparysh_db::Database {
    fn installed(&self, name: &str) -> Option<InstalledPkg> {
        let pkg = self.get_package(name).ok()??;
        Some(InstalledPkg {
            name: pkg.name,
            version: Version::parse(&pkg.version).ok()?,
            version_str: pkg.version,
            build_rev: pkg.build_rev as u32,
        })
    }
}
