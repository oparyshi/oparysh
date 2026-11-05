//! oparysh-builder — сборка пакетов (этап 7 ROADMAP).
//!
//! Реализовано:
//! - **7.1** — вычисление финальных флагов сборки по разделу 6 DOCS:
//!   `CFLAGS = profile + build.extra + recipe.build.cflags` (конкатенация слева направо),
//!   `MAKEOPTS = -j{jobs} + config makeopts + recipe makeopts`, приоритет `cc`:
//!   рецепт → конфиг (если у рецепта `cc` непуст, он побеждает).
//! - **7.2** — подстановки `${OPARYSH_*}` в шагах сборки.

use oparysh_core::config::OparyshConfig;
use oparysh_core::recipe::Recipe;
use thiserror::Error;

/// Финальные флаги сборки, вычисленные из конфига и рецепта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFlags {
    /// Компилятор C: `recipe.build.cc` если непуст, иначе `toolchain.cc`.
    pub cc: String,
    /// Компилятор C++ из `toolchain.cxx`.
    pub cxx: String,
    /// Препроцессор из `toolchain.cpp`.
    pub cpp: String,
    /// `profile.cflags + build.extra.cflags + recipe.build.cflags`.
    pub cflags: String,
    /// `profile.cxxflags + build.extra.cxxflags + recipe.build.cflags`-аналог (cxxflags рецепта нет — только profile+extra).
    pub cxxflags: String,
    /// `profile.ldflags + build.extra.ldflags + recipe.build.ldflags`.
    pub ldflags: String,
    /// `-j{jobs} + build.extra.makeopts + recipe.build.makeopts`.
    pub makeopts: String,
    /// Число заданий: `recipe.build.parallel` если > 0, иначе `general.jobs`.
    pub jobs: u32,
    /// Префикс установки из рецепта (`[build.install].prefix`).
    pub prefix: String,
}

/// Ошибки вычисления флагов.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlagError {
    /// Активный профиль не найден в конфиге.
    #[error("активный профиль `{0}` не найден в конфиге (ожидается [profile.{0}])")]
    UnknownProfile(String),
}

/// Склеить непустые фрагменты через один пробел.
fn join_flags(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Вычислить финальные флаги сборки (раздел 6 DOCS, шаг 7.1 ROADMAP).
///
/// Приоритет наложения (конкатенация слева направо):
/// `profile.<active>` → `[build.extra]` → `[recipe.build]`.
/// Для `cc`: непустой `recipe.build.cc` побеждает `toolchain.cc`.
/// Для `jobs`: `recipe.build.parallel > 0` побеждает `general.jobs`.
pub fn compute_flags(config: &OparyshConfig, recipe: &Recipe) -> Result<BuildFlags, FlagError> {
    let profile_name = config.active_profile().to_string();
    let profile = config
        .find_profile(&profile_name)
        .ok_or(FlagError::UnknownProfile(profile_name))?;

    let extra = &config.build.extra;
    let rb = &recipe.build;

    let jobs = if rb.parallel > 0 {
        rb.parallel
    } else {
        config.general.jobs
    };

    let cc = if rb.cc.trim().is_empty() {
        config.toolchain.cc.clone()
    } else {
        rb.cc.clone()
    };

    Ok(BuildFlags {
        cc,
        cxx: config.toolchain.cxx.clone(),
        cpp: config.toolchain.cpp.clone(),
        cflags: join_flags(&[&profile.cflags, &extra.cflags, &rb.cflags]),
        cxxflags: join_flags(&[&profile.cxxflags, &extra.cxxflags]),
        ldflags: join_flags(&[&profile.ldflags, &extra.ldflags, &rb.ldflags]),
        makeopts: join_flags(&[&format!("-j{jobs}"), &extra.makeopts, &rb.makeopts]),
        jobs,
        prefix: recipe.build.install.prefix.clone(),
    })
}

/// Набор подстановок `${OPARYSH_*}` для шагов сборки (7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substitutions {
    /// Каталог исходников (`build/<name>-<ver>/`).
    pub src_dir: String,
    /// Префикс установки (`[build.install].prefix`).
    pub prefix: String,
    /// Псевдо-DESTDIR, куда `make install` кладёт файлы.
    pub destdir: String,
    /// Финальные флаги/параметры.
    pub flags: BuildFlags,
}

impl Substitutions {
    /// Собрать подстановки из флагов и путей.
    pub fn new(flags: BuildFlags, src_dir: &str, destdir: &str) -> Self {
        Substitutions {
            src_dir: src_dir.to_string(),
            prefix: flags.prefix.clone(),
            destdir: destdir.to_string(),
            flags,
        }
    }

    /// Значение переменной по имени без префикса `OPARYSH_`; `None` — неизвестная.
    pub fn get(&self, name: &str) -> Option<String> {
        match name {
            "OPARYSH_PREFIX" => Some(self.prefix.clone()),
            "OPARYSH_DESTDIR" => Some(self.destdir.clone()),
            "OPARYSH_MAKEOPTS" => Some(self.flags.makeopts.clone()),
            "OPARYSH_CFLAGS" => Some(self.flags.cflags.clone()),
            "OPARYSH_CXXFLAGS" => Some(self.flags.cxxflags.clone()),
            "OPARYSH_LDFLAGS" => Some(self.flags.ldflags.clone()),
            "OPARYSH_CC" => Some(self.flags.cc.clone()),
            "OPARYSH_CXX" => Some(self.flags.cxx.clone()),
            "OPARYSH_JOBS" => Some(self.flags.jobs.to_string()),
            "OPARYSH_SRC_DIR" => Some(self.src_dir.clone()),
            _ => None,
        }
    }
}

/// Ошибки подстановки переменных.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubstError {
    /// Встречена неизвестная переменная `${...}`.
    #[error("неизвестная переменная подстановки `${{{0}}}`")]
    UnknownVar(String),
    /// Незакрытая `${`.
    #[error("незакрытая подстановка `${{` в шаге сборки")]
    Unterminated,
}

/// Подставить `${OPARYSH_*}` в одну строку шага (7.2).
pub fn substitute(input: &str, subs: &Substitutions) -> Result<String, SubstError> {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or(SubstError::Unterminated)?;
        let name = &after[..end];
        let value = subs
            .get(name)
            .ok_or_else(|| SubstError::UnknownVar(name.to_string()))?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Подставить переменные во все аргументы шага.
pub fn substitute_args(args: &[String], subs: &Substitutions) -> Result<Vec<String>, SubstError> {
    args.iter().map(|a| substitute(a, subs)).collect()
}
