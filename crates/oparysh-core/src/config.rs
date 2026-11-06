//! Парсер пользовательской конфигурации `oparysh.conf` (этап 1.5 ROADMAP, раздел 6 DOCS).
//!
//! Пример:
//! ```toml
//! [general]
//! db_dir      = "/var/lib/oparysh"
//! cache_dir   = "/usr/local/oparysh/cache"
//! destroot    = "/"
//! jobs        = 8
//! keep_build  = false
//! assume_yes  = false
//!
//! [toolchain]
//! cc     = "gcc"
//! cxx    = "g++"
//! cpp    = "cpp"
//! linker = ""
//!
//! [build.active]
//! profile = "release"
//!
//! [profile.release]
//! cflags   = "-O2 -pipe"
//! cxxflags = "-O2 -pipe"
//! ldflags  = "-Wl,-O1"
//!
//! [profile.debug]
//! cflags   = "-O0 -g3"
//! cxxflags = "-O0 -g3"
//!
//! [build.extra]
//! cflags   = "-march=native"
//! makeopts = ""
//! env      = {}
//!
//! [repositories]
//! [[repositories.entries]]
//! name = "local"
//! path = "/srv/oparysh-repo"
//!
//! [[repositories.entries]]
//! name   = "stable"
//! url    = "https://repo.example.com/stable"
//! key    = "/etc/oparysh/keys/stable.gpg"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Весь конфиг oparysh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OparyshConfig {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub toolchain: Toolchain,
    #[serde(default)]
    pub build: Build,
    /// Именованные профили: ключ — имя профиля (`release`, `debug`, …).
    #[serde(default)]
    pub profile: BTreeMap<String, ProfileFlags>,
    #[serde(default)]
    pub repositories: Repositories,
}

/// Секция `[general]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub db_dir: String,
    pub cache_dir: String,
    pub destroot: String,
    pub jobs: u32,
    pub keep_build: bool,
    pub assume_yes: bool,
}

impl Default for General {
    fn default() -> Self {
        General {
            db_dir: "/var/lib/oparysh".to_string(),
            cache_dir: "/usr/local/oparysh/cache".to_string(),
            destroot: "/".to_string(),
            jobs: 8,
            keep_build: false,
            assume_yes: false,
        }
    }
}

/// Секция `[toolchain]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Toolchain {
    pub cc: String,
    pub cxx: String,
    pub cpp: String,
    pub linker: String,
}

impl Default for Toolchain {
    fn default() -> Self {
        Toolchain {
            cc: "gcc".to_string(),
            cxx: "g++".to_string(),
            cpp: "cpp".to_string(),
            linker: String::new(),
        }
    }
}

/// Секция `[build]` — содержит `active` (выбранный профиль) и `extra` (доп. флаги).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    pub active: BuildActive,
    pub extra: BuildExtra,
}

impl Default for Build {
    fn default() -> Self {
        Build {
            active: BuildActive::default(),
            extra: BuildExtra::default(),
        }
    }
}

/// Секция `[build.active]` — выбранный активный профиль.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildActive {
    pub profile: String,
}

impl Default for BuildActive {
    fn default() -> Self {
        BuildActive {
            profile: "release".to_string(),
        }
    }
}

/// Флаги одного профиля (`[profile.<имя>]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileFlags {
    pub cflags: String,
    pub cxxflags: String,
    pub ldflags: String,
}

impl Default for ProfileFlags {
    fn default() -> Self {
        ProfileFlags {
            cflags: String::new(),
            cxxflags: String::new(),
            ldflags: String::new(),
        }
    }
}

/// Секция `[build.extra]` — дополнительные флаги, склеиваемые с профилем.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildExtra {
    pub cflags: String,
    pub cxxflags: String,
    pub ldflags: String,
    pub makeopts: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Default for BuildExtra {
    fn default() -> Self {
        BuildExtra {
            cflags: String::new(),
            cxxflags: String::new(),
            ldflags: String::new(),
            makeopts: String::new(),
            env: BTreeMap::new(),
        }
    }
}

/// Секция `[repositories]` со списком записей.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Repositories {
    pub entries: Vec<RepoEntry>,
}

impl Default for Repositories {
    fn default() -> Self {
        Repositories {
            entries: Vec::new(),
        }
    }
}

/// Одна запись `[[repositories.entries]]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoEntry {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
}

/// Ошибка применения переменных окружения `OPARYSH_*` (DOCS §12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvOverrideError {
    /// Имя переменной, вызвавшей ошибку.
    pub var: &'static str,
    /// Описание проблемы.
    pub message: String,
}

impl std::fmt::Display for EnvOverrideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.var, self.message)
    }
}

impl std::error::Error for EnvOverrideError {}

/// Развернуть ведущую `~` в пути: `"~"` → `$HOME`, `"~/..."` → `$HOME/...`.
/// Остальные значения (включая `~user`) возвращаются без изменений.
fn expand_tilde(path: &str, home: &str) -> String {
    if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", home.trim_end_matches('/'), rest)
    } else {
        path.to_string()
    }
}

/// Склеить фрагмент флагов с существующим значением через один пробел.
fn append_flag(base: &str, extra: &str) -> String {
    let extra = extra.trim();
    if extra.is_empty() {
        return base.trim().to_string();
    }
    let base = base.trim();
    if base.is_empty() {
        extra.to_string()
    } else {
        format!("{base} {extra}")
    }
}

impl OparyshConfig {
    /// Разобрать `oparysh.conf` из строки TOML.
    pub fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Применить перекрытия из переменных окружения `OPARYSH_*` (DOCS §12).
    ///
    /// Переменные перекрывают значения конфига; `OPARYSH_CFLAGS`/`OPARYSH_CXXFLAGS`/
    /// `OPARYSH_MAKEOPTS` дописываются **после** `[build.extra]` (как и оно —
    /// поверх профиля). Пустые строки трактуются как «не задано».
    ///
    /// Не задаёт `OPARYSH_NO_COLORS`/`NO_COLOR` (это про оформление вывода,
    /// см. `oparysh_cli::style`) и `OPARYSH_CONFIG` (путь к самому конфигу).
    pub fn apply_env_overrides(&mut self) -> Result<(), EnvOverrideError> {
        self.apply_env_overrides_with(|key| std::env::var(key).ok())
    }

    /// Вариант [`OparyshConfig::apply_env_overrides`] с внедряемым источником
    /// значений — для тестов без модификации реального окружения.
    pub fn apply_env_overrides_with(
        &mut self,
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<(), EnvOverrideError> {
        let non_empty = |key: &str| get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());

        if let Some(v) = non_empty("OPARYSH_DB_DIR") {
            self.general.db_dir = v;
        }
        if let Some(v) = non_empty("OPARYSH_CACHE_DIR") {
            self.general.cache_dir = v;
        }
        if let Some(v) = non_empty("OPARYSH_ROOT") {
            self.general.destroot = v;
        }
        if let Some(v) = non_empty("OPARYSH_JOBS") {
            self.general.jobs = v.parse::<u32>().map_err(|_| EnvOverrideError {
                var: "OPARYSH_JOBS",
                message: format!("ожидалось положительное число, получено «{v}»"),
            })?;
            if self.general.jobs == 0 {
                return Err(EnvOverrideError {
                    var: "OPARYSH_JOBS",
                    message: "значение должно быть >= 1".to_string(),
                });
            }
        }
        if let Some(v) = non_empty("OPARYSH_CC") {
            self.toolchain.cc = v;
        }
        if let Some(v) = non_empty("OPARYSH_CXX") {
            self.toolchain.cxx = v;
        }
        if let Some(v) = non_empty("OPARYSH_CFLAGS") {
            self.build.extra.cflags = append_flag(&self.build.extra.cflags, &v);
        }
        if let Some(v) = non_empty("OPARYSH_CXXFLAGS") {
            self.build.extra.cxxflags = append_flag(&self.build.extra.cxxflags, &v);
        }
        if let Some(v) = non_empty("OPARYSH_MAKEOPTS") {
            self.build.extra.makeopts = append_flag(&self.build.extra.makeopts, &v);
        }
        if let Some(v) = non_empty("OPARYSH_PROFILE") {
            self.build.active.profile = v;
        }
        Ok(())
    }

    /// Развёртка `~` в путях конфига (DOCS §6: «везде работает развёртка `~`»).
    ///
    /// Затрагивает `general.db_dir`, `general.cache_dir`, `general.destroot`,
    /// а также `path` и `key` каждой записи `[[repositories.entries]]`.
    pub fn expand_tildes(&mut self) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        self.expand_tildes_with(&home);
    }

    /// Вариант [`OparyshConfig::expand_tildes`] с явным домашним каталогом —
    /// для тестов.
    pub fn expand_tildes_with(&mut self, home: &str) {
        self.general.db_dir = expand_tilde(&self.general.db_dir, home);
        self.general.cache_dir = expand_tilde(&self.general.cache_dir, home);
        self.general.destroot = expand_tilde(&self.general.destroot, home);
        for entry in &mut self.repositories.entries {
            if let Some(p) = &mut entry.path {
                *p = expand_tilde(p, home);
            }
            if let Some(k) = &mut entry.key {
                *k = expand_tilde(k, home);
            }
        }
    }

    /// Вернуть имя активного профиля.
    pub fn active_profile(&self) -> &str {
        &self.build.active.profile
    }

    /// Найти флаги профиля по имени.
    pub fn find_profile(&self, name: &str) -> Option<&ProfileFlags> {
        self.profile.get(name)
    }
}
