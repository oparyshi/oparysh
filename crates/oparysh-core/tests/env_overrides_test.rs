//! Этап 11.4: перекрытие `oparysh.conf` переменными окружения `OPARYSH_*` (DOCS §12).

use oparysh_core::config::OparyshConfig;
use std::collections::HashMap;

fn base_config() -> OparyshConfig {
    OparyshConfig::parse(
        r#"
[general]
db_dir = "/base/db"
cache_dir = "/base/cache"
destroot = "/base/root"
jobs = 4

[toolchain]
cc = "gcc"
cxx = "g++"

[build.active]
profile = "release"

[build.extra]
cflags = "-march=native"
cxxflags = "-march=native"
makeopts = "V=1"

[profile.release]
cflags = "-O2"
[profile.debug]
cflags = "-O0 -g3"
"#,
    )
    .expect("валидный конфиг")
}

fn with_env(pairs: &[(&str, &str)]) -> OparyshConfig {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut cfg = base_config();
    cfg.apply_env_overrides_with(|key| map.get(key).cloned())
        .expect("применение перекрытий");
    cfg
}

#[test]
fn db_and_cache_dir_overridden() {
    let cfg = with_env(&[("OPARYSH_DB_DIR", "/mnt/chroot/db"), ("OPARYSH_CACHE_DIR", "/tmp/cache")]);
    assert_eq!(cfg.general.db_dir, "/mnt/chroot/db");
    assert_eq!(cfg.general.cache_dir, "/tmp/cache");
}

#[test]
fn root_overrides_destroot() {
    let cfg = with_env(&[("OPARYSH_ROOT", "/mnt/chroot2")]);
    assert_eq!(cfg.general.destroot, "/mnt/chroot2");
}

#[test]
fn jobs_overridden_and_validated() {
    let cfg = with_env(&[("OPARYSH_JOBS", "16")]);
    assert_eq!(cfg.general.jobs, 16);

    for bad in ["abc", "0", "-3", "1.5"] {
        let map: HashMap<String, String> =
            [("OPARYSH_JOBS".to_string(), bad.to_string())].into_iter().collect();
        let mut cfg = base_config();
        let err = cfg.apply_env_overrides_with(|key| map.get(key).cloned());
        assert!(err.is_err(), "OPARYSH_JOBS={bad:?} должно быть ошибкой");
    }
}

#[test]
fn compilers_overridden() {
    let cfg = with_env(&[("OPARYSH_CC", "clang"), ("OPARYSH_CXX", "clang++")]);
    assert_eq!(cfg.toolchain.cc, "clang");
    assert_eq!(cfg.toolchain.cxx, "clang++");
}

#[test]
fn flags_appended_after_build_extra() {
    let cfg = with_env(&[
        ("OPARYSH_CFLAGS", "-flto"),
        ("OPARYSH_CXXFLAGS", "-flto"),
        ("OPARYSH_MAKEOPTS", "V=2"),
    ]);
    assert_eq!(cfg.build.extra.cflags, "-march=native -flto");
    assert_eq!(cfg.build.extra.cxxflags, "-march=native -flto");
    assert_eq!(cfg.build.extra.makeopts, "V=1 V=2");
}

#[test]
fn profile_overridden() {
    let cfg = with_env(&[("OPARYSH_PROFILE", "debug")]);
    assert_eq!(cfg.active_profile(), "debug");
}

#[test]
fn empty_values_are_ignored() {
    let cfg = with_env(&[
        ("OPARYSH_DB_DIR", ""),
        ("OPARYSH_JOBS", "   "),
        ("OPARYSH_CFLAGS", ""),
    ]);
    assert_eq!(cfg.general.db_dir, "/base/db");
    assert_eq!(cfg.general.jobs, 4);
    assert_eq!(cfg.build.extra.cflags, "-march=native");
}

#[test]
fn unset_env_keeps_config() {
    let cfg = with_env(&[]);
    assert_eq!(cfg.general.db_dir, "/base/db");
    assert_eq!(cfg.general.cache_dir, "/base/cache");
    assert_eq!(cfg.general.destroot, "/base/root");
    assert_eq!(cfg.general.jobs, 4);
    assert_eq!(cfg.toolchain.cc, "gcc");
    assert_eq!(cfg.active_profile(), "release");
}
