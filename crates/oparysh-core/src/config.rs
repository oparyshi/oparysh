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

impl OparyshConfig {
    /// Разобрать `oparysh.conf` из строки TOML.
    pub fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
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
