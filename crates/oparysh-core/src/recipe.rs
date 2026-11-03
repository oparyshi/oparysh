//! Типы рецепта пакета (`pkg.toml`) — раздел 5 DOCS.
//!
//! Обязательные секции рецепта — `[pkg]` и `[build]`; обязательное поле
//! источника — `sha256` (проверяется валидатором, см. шаг 1.2).

use serde::{Deserialize, Serialize};

/// Целый рецепт пакета.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Описание пакета.
    pub pkg: Pkg,
    /// Источник (тарболл или файл из репозитория).
    #[serde(default)]
    pub source: Source,
    /// Требования к сборке и рантайму.
    #[serde(default)]
    pub deps: Deps,
    /// Патчи, применяемые по порядку.
    #[serde(default)]
    pub patches: Vec<Patch>,
    /// Предоставляемые виртуальные возможности.
    #[serde(default)]
    pub provides: Provides,
    /// Конфликтующие пакеты.
    #[serde(default)]
    pub conflicts: Conflicts,
    /// Секция сборки.
    pub build: Build,
    /// Действия после установки (необязательно).
    #[serde(default)]
    pub files: Files,
}

/// Секция `[pkg]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pkg {
    /// Имя пакета.
    pub name: String,
    /// Версия апстрима.
    pub version: String,
    /// Номер перебилда при той же апстрим-версии.
    #[serde(default)]
    pub build: u32,
    /// Целевые архитектуры.
    #[serde(default)]
    pub arch: Vec<String>,
    /// Короткое описание.
    #[serde(default)]
    pub desc: String,
    /// Лицензия (SPDX).
    #[serde(default)]
    pub license: String,
    /// Домашняя страница.
    #[serde(default)]
    pub homepage: String,
}

/// Секция `[source]`: локальный путь, URL или имя файла в репозитории.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// URL или локальный путь.
    #[serde(default)]
    pub url: Option<String>,
    /// Имя файла в репозитории (`dist/…`).
    #[serde(default)]
    pub file: Option<String>,
    /// SHA-256 тарболла. Обязателен; отсутствие = отказ сборки.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Требования с операторами `>=, <=, >, <, =, ^, ~`; разбор — шаг 1.3.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deps {
    /// Нужны только при сборке.
    #[serde(default)]
    pub build: Vec<String>,
    /// Нужны в рантайме.
    #[serde(default)]
    pub run: Vec<String>,
    /// Опциональные: `"требование"` → имя фичи.
    #[serde(default)]
    pub optional: std::collections::BTreeMap<String, String>,
}

/// `[[patches]]`: применяется по порядку, каждый проверяется по sha256.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// Имя файла патча.
    pub file: String,
    /// Обязательный SHA-256 патча.
    pub sha256: String,
}

/// Секция `[provides]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    /// Виртуальные предоставляемые возможности (ключ `virtual`).
    #[serde(default, rename = "virtual")]
    pub r#virtual: Vec<String>,
}

/// Секция `[conflicts]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflicts {
    /// Требования конфликтующих пакетов (с операторами версий).
    #[serde(default)]
    pub pkgs: Vec<String>,
}

/// Единица окружения сборки: переменная окружения или шаг сборки.
pub type ShellArgs = Vec<String>;

/// Секция `[build]`: всё это юзер может перекрыть через `oparysh.conf`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// Сборочная система: `autoconf-make`, `cmake-ninja`, `cargo`, `meson`,
    /// `make-only`, `custom`.
    #[serde(default = "default_build_system")]
    pub system: String,
    /// Компилятор C; пусто = берётся из конфига/профиля.
    #[serde(default)]
    pub cc: String,
    /// Доп. флаги ПОВЕРХ профиля.
    #[serde(default)]
    pub cflags: String,
    /// Доп. флаги линковщика.
    #[serde(default)]
    pub ldflags: String,
    /// Доп. аргументы make.
    #[serde(default)]
    pub makeopts: String,
    /// Число заданий; 0 = из config (jobs).
    #[serde(default)]
    pub parallel: u32,
    /// Произвольные переменные окружения.
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// Секция установки.
    #[serde(default)]
    pub install: BuildInstall,
    /// Полное переопределение команд (обязательно для `system = "custom"`).
    #[serde(default)]
    pub steps: BuildSteps,
}

fn default_build_system() -> String {
    "autoconf-make".to_string()
}

/// Секция `[build.install]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInstall {
    /// Префикс установки; DESTDIR применяется всегда.
    pub prefix: String,
}

impl Default for BuildInstall {
    fn default() -> Self {
        BuildInstall {
            prefix: "/usr/local".to_string(),
        }
    }
}

/// Секция `[build.steps]`: необязательное переопределение шагов сборки.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSteps {
    /// Подготовка (например `./configure …`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<ShellArgs>,
    /// Компиляция (например `make`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile: Option<ShellArgs>,
    /// Установка.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install: Option<ShellArgs>,
}

/// Секция `[files]`: необязательные действия после установки.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Files {
    /// Предупреждать, если make установил незаявленные файлы.
    #[serde(default)]
    pub unknown_warn: bool,
}

impl Default for Files {
    fn default() -> Self {
        Files {
            unknown_warn: false,
        }
    }
}

impl Recipe {
    /// Разобрать рецепт из repr. TOML-строки.
    pub fn parse(toml_str: &str) -> Result<Self, String> {
        toml::from_str(toml_str).map_err(|e| e.to_string())
    }
}
