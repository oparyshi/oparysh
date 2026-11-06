//! oparysh-builder — сборка пакетов (этап 7 ROADMAP).
//!
//! Реализовано:
//! - **7.1** — вычисление финальных флагов сборки по разделу 6 DOCS:
//!   `CFLAGS = profile + build.extra + recipe.build.cflags` (конкатенация слева направо),
//!   `MAKEOPTS = -j{jobs} + config makeopts + recipe makeopts`, приоритет `cc`:
//!   рецепт → конфиг (если у рецепта `cc` непуст, он побеждает).
//! - **7.2** — подстановки `${OPARYSH_*}` в шагах сборки.
//! - **7.3** — планы сборочных систем `autoconf-make`, `make-only`, `custom`.
//! - **7.4** — планы сборочных систем `cmake-ninja`, `cargo`, `meson`.
//! - **7.5** — суженное окружение процесса: фиксированный `PATH`, компиляторы и
//!   флаги (`CC/CXX/CPP/CFLAGS/…`) только из конфига/профиля, плюс явно указанные
//!   переменные из `[build.extra].env` и `[build].env = {}`.
//! - **7.6** — выполнение плана сборки (`run_build_plan`): вывод каждого шага
//!   пишется в лог `logs/<pkg>-<ver>-<date>.log`; первый упавший шаг прерывает
//!   сборку, каталог `build/` сохраняется, `BuildRunError::Failed` → код
//!   возврата **7** (DOCS §11).
//! - **7.7** — перекрытия `--set cflags=… --set cc=…` и `--jobs N`(`FlagOverrides`):
//!   действуют только на одну сборку (`compute_flags_with`/`build_plan_with`),
//!   конфиг/рецепт не изменяются.
//! - **7.8** — команда `oparysh build <pkg|recipe-dir>` (CLI): fetch → extract →
//!   patches → план сборки, БЕЗ установки; результат остаётся в tmp-DESTDIR.

use oparysh_core::config::OparyshConfig;
use oparysh_core::recipe::Recipe;
use std::collections::BTreeMap;
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

/// Фиксированный `PATH` для сборочных процессов (7.5).
///
/// `PATH` пользователя/шелла **не наследуется** — иначе сборки невоспроизводимы.
pub const BUILD_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// Ошибки вычисления флагов.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlagError {
    /// Активный профиль не найден в конфиге.
    #[error("активный профиль `{0}` не найден в конфиге (ожидается [profile.{0}])")]
    UnknownProfile(String),
}

/// Перекрытия флагов на одну сборку (7.7).
///
/// Задаются пользователем: `--set cflags=… --set cc=…` и `--jobs N`.
/// Действуют **только на конкретную сборку** — ни рецепт, ни конфиг не меняются.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlagOverrides {
    /// Значения `k=v` по имени флага; перекрывают финальное значение целиком.
    pub sets: BTreeMap<String, String>,
    /// `--jobs N` — перекрывает число заданий (и `-j` в `MAKEOPTS`).
    pub jobs: Option<u32>,
}

/// Перекрываемые имена флагов `--set`.
pub const OVERRIDABLE_FLAGS: &[&str] = &["cc", "cxx", "cpp", "cflags", "cxxflags", "ldflags", "makeopts"];

impl FlagOverrides {
    /// Разобрать аргументы `k=v`. Неизвестный ключ или отсутствие `=`
    /// — ошибка (код возврата 2).
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut sets = BTreeMap::new();
        for arg in args {
            let (key, value) = arg
                .split_once('=')
                .ok_or_else(|| format!("--set ожидает `k=v`, получено `{arg}`"))?;
            if !OVERRIDABLE_FLAGS.contains(&key) {
                return Err(format!(
                    "--set: неизвестный флаг `{key}` (допустимые: {})",
                    OVERRIDABLE_FLAGS.join(", ")
                ));
            }
            sets.insert(key.to_string(), value.to_string());
        }
        Ok(FlagOverrides { sets, jobs: None })
    }

    /// Заменить лидер `-j{N}` в строке MAKEOPTS на актуальный `jobs`.
    fn relead_makeopts(makeopts: &str, jobs: u32) -> String {
        let head = format!("-j{jobs}");
        match makeopts.split_once(' ') {
            Some((first, rest)) if first.starts_with("-j") => format!("{head} {rest}"),
            Some(_) | None => {
                if makeopts.starts_with("-j") {
                    head
                } else {
                    join_flags(&[&head, makeopts])
                }
            }
        }
    }
}

/// Применить перекрытия к вычисленным флагам (7.7).
///
/// `--set k=v` **перекрывает** финальное значение флага целиком (то, что
/// рассчитано из профиля/конфига/рецепта, отбрасывается). `jobs` также
/// пересобирает `-j{N}` в начале `MAKEOPTS`.
fn apply_overrides(mut flags: BuildFlags, overrides: &FlagOverrides) -> BuildFlags {
    for (key, value) in &overrides.sets {
        match key.as_str() {
            "cc" => flags.cc = value.clone(),
            "cxx" => flags.cxx = value.clone(),
            "cpp" => flags.cpp = value.clone(),
            "cflags" => flags.cflags = value.clone(),
            "cxxflags" => flags.cxxflags = value.clone(),
            "ldflags" => flags.ldflags = value.clone(),
            "makeopts" => flags.makeopts = value.clone(),
            _ => unreachable!("OVERRIDABLE_FLAGS согласован с apply_overrides"),
        }
    }
    if let Some(jobs) = overrides.jobs {
        flags.jobs = jobs.max(1);
        flags.makeopts = FlagOverrides::relead_makeopts(&flags.makeopts, flags.jobs);
    }
    flags
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
    compute_flags_with(config, recipe, &FlagOverrides::default())
}

/// Вычислить финальные флаги с перекрытиями `--set`/`--jobs` (7.7).
///
/// Перекрытия действуют только на этот вызов (одну сборку): конфиг,
/// рецепт и профиль не изменяются. `--set k=v` перекрывает финальное
/// значение флага целиком; `--jobs N` перекрывает число заданий и `-j`
/// в `MAKEOPTS`.
pub fn compute_flags_with(
    config: &OparyshConfig,
    recipe: &Recipe,
    overrides: &FlagOverrides,
) -> Result<BuildFlags, FlagError> {
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

    let mut ldflags = join_flags(&[&profile.ldflags, &extra.ldflags, &rb.ldflags]);
    // linker из toolchain ("mold", "ld.lld", …): включаем через -fuse-ld=
    let linker = config.toolchain.linker.trim();
    if !linker.is_empty() {
        ldflags = join_flags(&[&ldflags, &format!("-fuse-ld={linker}")]);
    }

    let flags = BuildFlags {
        cc,
        cxx: config.toolchain.cxx.clone(),
        cpp: config.toolchain.cpp.clone(),
        cflags: join_flags(&[&profile.cflags, &extra.cflags, &rb.cflags]),
        cxxflags: join_flags(&[&profile.cxxflags, &extra.cxxflags]),
        ldflags,
        makeopts: join_flags(&[&format!("-j{jobs}"), &extra.makeopts, &rb.makeopts]),
        jobs,
        prefix: recipe.build.install.prefix.clone(),
    };

    if overrides.jobs.is_some() || !overrides.sets.is_empty() {
        Ok(apply_overrides(flags, overrides))
    } else {
        Ok(flags)
    }
}

/// Канонические переменные окружения сборки (7.5).
///
/// Всегда вычисляются oparysh из активного профиля/конфига и применяются
/// **последними**, поэтому `env = {}` рецепта не может случайно переопределить
/// компилятор или флаги.
fn canonical_build_env(flags: &BuildFlags, destdir: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("PATH".to_string(), BUILD_PATH.to_string());
    env.insert("CC".to_string(), flags.cc.clone());
    env.insert("CXX".to_string(), flags.cxx.clone());
    env.insert("CPP".to_string(), flags.cpp.clone());
    env.insert("CFLAGS".to_string(), flags.cflags.clone());
    env.insert("CXXFLAGS".to_string(), flags.cxxflags.clone());
    env.insert("LDFLAGS".to_string(), flags.ldflags.clone());
    env.insert("MAKEOPTS".to_string(), flags.makeopts.clone());
    env.insert("DESTDIR".to_string(), destdir.to_string());
    env
}

/// Собрать суженное окружение для запуска шагов сборки (7.5).
///
/// Возвращается полный набор переменных: окружение шелла **не** примешивается.
/// Порядок наложения (слева направо, побеждает правее):
/// `[build.extra].env` → `recipe.build.env` → канонические переменные
/// (`PATH`, `CC`, `CXX`, `CPP`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `MAKEOPTS`,
/// `DESTDIR`). Тем самым пользовательские `CFLAGS`/`PATH` из шелла не проникают,
/// а явно указанные `env = {}` сохраняются.
pub fn narrowed_environment(
    config: &OparyshConfig,
    recipe: &Recipe,
    flags: &BuildFlags,
    destdir: &str,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.extend(config.build.extra.env.clone());
    env.extend(recipe.build.env.clone());
    env.extend(canonical_build_env(flags, destdir));
    env
}

/// Фазы сборки в порядке выполнения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildPhase {
    Prepare,
    Compile,
    Install,
}

impl BuildPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Compile => "compile",
            Self::Install => "install",
        }
    }
}

/// Один шаг сборки без shell-интерполяции.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStep {
    pub phase: BuildPhase,
    pub argv: Vec<String>,
    /// Переменные, добавляемые только на этот шаг; `DESTDIR` находится в плане.
    pub env: BTreeMap<String, String>,
}

/// Полный план сборки: команды по фазам и суженное окружение.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    pub system: String,
    pub steps: Vec<BuildStep>,
    /// Суженное окружение (7.5): фиксированный `PATH`, канонические
    /// `CC/CXX/CPP/CFLAGS/…` из профиля и явные `env = {}` (+ `DESTDIR`).
    pub environment: BTreeMap<String, String>,
}

impl BuildPlan {
    /// Окружение конкретного шага: общее окружение плана + `env` шага.
    ///
    /// Канонические переменные плана применяются последними, поэтому
    /// пошаговый `env` может добавить переменные, но не переопределить
    /// компилятор/флаги.
    pub fn env_for_step(&self, step: &BuildStep) -> BTreeMap<String, String> {
        let mut env = step.env.clone();
        env.extend(self.environment.clone());
        env
    }
}

/// Ошибки построения плана сборки.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BuildPlanError {
    #[error(transparent)]
    Flags(#[from] FlagError),
    #[error(transparent)]
    Substitution(#[from] SubstError),
    #[error("неизвестная сборочная система `{0}`")]
    UnknownSystem(String),
    #[error("`system = \"custom\"` не может быть собран: [build.steps] не содержит ни одного шага")]
    EmptyCustomSteps,
}

fn split_make_opts(value: &str) -> Vec<String> {
    value.split_whitespace().map(ToString::to_string).collect()
}

fn join_install_root(destdir: &str, prefix: &str) -> String {
    format!("{}/{}", destdir.trim_end_matches('/'), prefix.trim_start_matches('/'))
}

fn phase_step(phase: BuildPhase, argv: Vec<String>, env: BTreeMap<String, String>) -> BuildStep {
    BuildStep { phase, argv, env }
}

fn custom_phase(
    phase: BuildPhase,
    args: &Option<Vec<String>>,
    subs: &Substitutions,
) -> Result<Option<BuildStep>, SubstError> {
    args.as_ref()
        .map(|args| substitute_args(args, subs).map(|argv| phase_step(phase, argv, BTreeMap::new())))
        .transpose()
}

/// Построить план сборки по `[build].system` (7.3–7.4).
///
/// Явные `[build.steps]` переопределяют соответствующую фазу выбранной системы;
/// отсутствующие фазы берутся из дефолтов. Для `custom` дефолтов нет.
pub fn build_plan(
    config: &OparyshConfig,
    recipe: &Recipe,
    src_dir: &str,
    destdir: &str,
) -> Result<BuildPlan, BuildPlanError> {
    build_plan_with(config, recipe, src_dir, destdir, &FlagOverrides::default())
}

/// Построить план сборки с перекрытиями `--set`/`--jobs` (7.7).
///
/// Перекрытия влияют на все фазы плана и на суженное окружение, но не
/// сохраняются: следующая сборка снова получит флаги из конфига/рецепта.
pub fn build_plan_with(
    config: &OparyshConfig,
    recipe: &Recipe,
    src_dir: &str,
    destdir: &str,
    overrides: &FlagOverrides,
) -> Result<BuildPlan, BuildPlanError> {
    let flags = compute_flags_with(config, recipe, overrides)?;
    let subs = Substitutions::new(flags.clone(), src_dir, destdir);
    let bs = &recipe.build.steps;

    let environment = narrowed_environment(config, recipe, &flags, destdir);

    let prefix = flags.prefix.clone();
    let install_root = join_install_root(destdir, &prefix);
    let mut defaults: Vec<BuildStep> = match recipe.build.system.as_str() {
        "autoconf-make" => vec![
            phase_step(BuildPhase::Prepare, vec!["./configure".into(), format!("--prefix={prefix}")], BTreeMap::new()),
            phase_step(BuildPhase::Compile, std::iter::once("make".to_string()).chain(split_make_opts(&flags.makeopts)).collect(), BTreeMap::new()),
            phase_step(BuildPhase::Install, vec!["make".into(), "install".into(), format!("DESTDIR={destdir}")], BTreeMap::new()),
        ],
        "make-only" => vec![
            phase_step(BuildPhase::Compile, std::iter::once("make".to_string()).chain(split_make_opts(&flags.makeopts)).collect(), BTreeMap::new()),
            phase_step(BuildPhase::Install, vec!["make".into(), "install".into(), format!("DESTDIR={destdir}")], BTreeMap::new()),
        ],
        "cmake-ninja" => {
            let mut env = BTreeMap::new();
            env.insert("DESTDIR".to_string(), destdir.to_string());
            vec![
                phase_step(BuildPhase::Prepare, vec!["cmake".into(), "-G".into(), "Ninja".into(), format!("-DCMAKE_INSTALL_PREFIX={prefix}"), ".".into()], BTreeMap::new()),
                phase_step(BuildPhase::Compile, vec!["ninja".into()], BTreeMap::new()),
                phase_step(BuildPhase::Install, vec!["ninja".into(), "install".into()], env),
            ]
        }
        "cargo" => vec![
            phase_step(BuildPhase::Compile, vec!["cargo".into(), "build".into(), "--release".into()], BTreeMap::new()),
            phase_step(BuildPhase::Install, vec!["cargo".into(), "install".into(), format!("--root={install_root}")], BTreeMap::new()),
        ],
        "meson" => {
            let mut env = BTreeMap::new();
            env.insert("DESTDIR".to_string(), destdir.to_string());
            vec![
                phase_step(BuildPhase::Prepare, vec!["meson".into(), "setup".into(), format!("--prefix={prefix}"), "build".into()], BTreeMap::new()),
                phase_step(BuildPhase::Compile, vec!["ninja".into(), "-C".into(), "build".into()], BTreeMap::new()),
                phase_step(BuildPhase::Install, vec!["ninja".into(), "-C".into(), "build".into(), "install".into()], env),
            ]
        }
        "custom" => Vec::new(),
        other => return Err(BuildPlanError::UnknownSystem(other.to_string())),
    };

    if recipe.build.system == "custom" {
        defaults.extend(custom_phase(BuildPhase::Prepare, &bs.prepare, &subs)?.into_iter());
        defaults.extend(custom_phase(BuildPhase::Compile, &bs.compile, &subs)?.into_iter());
        defaults.extend(custom_phase(BuildPhase::Install, &bs.install, &subs)?.into_iter());
        if defaults.is_empty() {
            return Err(BuildPlanError::EmptyCustomSteps);
        }
    } else {
        for (phase, override_args) in [
            (BuildPhase::Prepare, &bs.prepare),
            (BuildPhase::Compile, &bs.compile),
            (BuildPhase::Install, &bs.install),
        ] {
            if let Some(step) = custom_phase(phase, override_args, &subs)? {
                defaults.retain(|candidate| candidate.phase != phase);
                defaults.push(step);
            }
        }
        defaults.sort_by_key(|step| step.phase);
    }

    Ok(BuildPlan { system: recipe.build.system.clone(), steps: defaults, environment })
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

// ---------------------------------------------------------------------------
// 7.6 — выполнение плана сборки с логом в logs/<pkg>-<ver>-<date>.log
// ---------------------------------------------------------------------------

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Дата по UTC в формате `YYYYMMDD` для имени лог-файла (без зависимостей).
fn today_stamp() -> String {
    /// Howard Hinnant: григорианская дата по числу дней от эпохи.
    fn civil_from_days(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (if m <= 2 { y + 1 } else { y }, m, d)
    }
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}{m:02}{d:02}")
}

/// Путь лога сборки: `<db_dir>/logs/<pkg>-<ver>-<date>.log` (DOCS §3, §8).
pub fn build_log_path(
    db_dir: &Path,
    pkg: &str,
    version: &str,
    date: &str,
) -> PathBuf {
    db_dir
        .join("logs")
        .join(format!("{pkg}-{version}-{date}.log"))
}

/// Путь лога сборки с текущей датой.
pub fn today_log_path(db_dir: &Path, pkg: &str, version: &str) -> PathBuf {
    build_log_path(db_dir, pkg, version, &today_stamp())
}

/// Ошибки выполнения шагов сборки (7.6).
#[derive(Debug, Error)]
pub enum BuildRunError {
    /// Процесс не удалось запустить (несуществующий executable).
    #[error("шаг `{phase}` ({executable}): не запущен: {source}")]
    Spawn {
        phase: String,
        executable: String,
        #[source]
        source: io::Error,
    },
    /// Шаг вернул ненулевой код — сборка прервана, `build/` сохранён, код возврата 7.
    #[error("шаг `{phase}` ({executable}) завершился с кодом {exit_code} — сборка прервана, build/ сохранён")]
    Failed { phase: String, executable: String, exit_code: i32 },
    /// Ошибка записи в лог-файл.
    #[error("не удалось записать лог сборки: {0}")]
    Log(#[from] io::Error),
}

/// Код возврата DOCS §11 для ошибки сборки.
pub const BUILD_FAILED_EXIT_CODE: i32 = 7;

/// Выполнить один шаг в `work_dir` с окружением `env`, вывод — в лог и на экран.
fn run_step(
    step: &BuildStep,
    work_dir: &Path,
    env: &BTreeMap<String, String>,
    log: &mut File,
) -> Result<(), BuildRunError> {
    let executable = step.argv.first().cloned().unwrap_or_default();
    let exe_display = executable.clone();
    writeln!(
        log,
        "== [{}] {} ==",
        step.phase.as_str(),
        step.argv.join(" ")
    )?;
    log.flush()?;

    let child = std::process::Command::new(&executable)
        .args(&step.argv[1..])
        .current_dir(work_dir)
        .env_clear()
        .envs(env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|source| BuildRunError::Spawn {
            phase: step.phase.as_str().to_string(),
            executable: exe_display.clone(),
            source,
        })?;

    let output = child
        .wait_with_output()
        .map_err(|source| BuildRunError::Spawn {
            phase: step.phase.as_str().to_string(),
            executable: exe_display.clone(),
            source,
        })?;

    log.write_all(&output.stdout)?;
    log.write_all(&output.stderr)?;
    log.flush()?;

    let mut screen = io::stdout();
    let _ = screen.write_all(&output.stdout);
    let _ = screen.flush();
    let mut screen_err = io::stderr();
    let _ = screen_err.write_all(&output.stderr);
    let _ = screen_err.flush();

    match output.status.code() {
        Some(0) => {}
        Some(code) => {
            writeln!(
                log,
                "== [{}] FAILED, exit {}: сборка прервана, build/ сохранён ==",
                step.phase.as_str(),
                code
            )?;
            log.flush()?;
            return Err(BuildRunError::Failed {
                phase: step.phase.as_str().to_string(),
                executable,
                exit_code: code,
            });
        }
        // Убит сигналом: трактуем как падение (код возврата 7).
        None => {
            writeln!(log, "== [{}] FAILED (сигнал): сборка прервана ==", step.phase.as_str())?;
            log.flush()?;
            return Err(BuildRunError::Failed {
                phase: step.phase.as_str().to_string(),
                executable,
                exit_code: -1,
            });
        }
    }
    Ok(())
}

/// Выполнить план сборки (7.6): шаги по порядку, вывод каждого шага
/// дублируется в лог. Первый упавший шаг прерывает сборку; файлы `build/`
/// не трогаются (сохраняются для отладки). По DOCS §8 п.6 лог дописывается
/// и после commit — дописывать остаётся возможностью вызывающего.
pub fn run_build_plan(
    plan: &BuildPlan,
    work_dir: &Path,
    log: &mut File,
) -> Result<(), BuildRunError> {
    for step in &plan.steps {
        let env = plan.env_for_step(step);
        run_step(step, work_dir, &env, log)?;
    }
    writeln!(log, "== все шаги завершены успешно ==")?;
    log.flush()?;
    Ok(())
}

/// Прочитать лог (для тестов и `oparysh info`).
pub fn read_log(path: &Path) -> io::Result<String> {
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s)?;
    Ok(s)
}
