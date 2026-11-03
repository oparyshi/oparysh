//! Парсер `index.toml` удалённого репозитория (этап 1.4 ROADMAP, раздел 4 DOCS).
//!
//! Формат:
//! ```toml
//! [repo]
//! name    = "stable"
//! .origin = "https://repo.example.com/stable"
//! updated = 1770000000
//!
//! [[pkg]]
//! name      = "gcc"
//! version   = "14.2.0"
//! build     = 2
//! arch      = ["x86_64", "aarch64"]
//! desc      = "GNU Compiler Collection"
//! tags      = ["devel", "toolchain"]
//! deps      = ["gmp>=6.2", "mpfr>=4.1", "zlib"]
//! provides  = ["c-compiler"]
//! conflicts = ["clang<16"]
//! recipe_sha = "9f2c…"
//! src_sha256 = "b04a…"
//! ```

use serde::{Deserialize, Serialize};

use crate::version::DepReq;

/// Весь индекс репозитория.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoIndex {
    pub repo: RepoMeta,
    pub pkg: Vec<RepoPkg>,
}

/// Секция `[repo]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoMeta {
    pub name: String,
    #[serde(rename = ".origin")]
    pub origin: Option<String>,
    #[serde(default)]
    pub updated: u64,
}

/// Одна запись `[[pkg]]` в индексе.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoPkg {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub build: u32,
    #[serde(default)]
    pub arch: Vec<String>,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Список требований зависимостей в сыром виде (`"gmp>=6.2"`).
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// SHA-256 файла рецепта `pkg.toml`.
    pub recipe_sha: String,
    /// SHA-256 тарбола исходников в `dist/`.
    pub src_sha256: String,
}

impl RepoIndex {
    /// Разобрать `index.toml` из строки.
    pub fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

impl RepoPkg {
    /// Распарсить список сырой строки зависимостей в `DepReq` (через `version::DepReq::parse`).
    pub fn parse_deps(&self) -> Result<Vec<DepReq>, crate::version::VersionError> {
        self.deps
            .iter()
            .map(|d| DepReq::parse(d))
            .collect()
    }
}
