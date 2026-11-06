//! Этап 11.5: глобальные опции `--config` / `--db-dir` / `--cache-dir` /
//! `--root` (эквивалентны переменным `OPARYSH_*` из DOCS §12).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn config_file(tmp: &Path, tag: &str) -> PathBuf {
    let path = tmp.join(format!("oparysh-{tag}.conf"));
    fs::write(
        &path,
        format!(
            "[general]\njobs = 2\ndb_dir = {:?}\ncache_dir = {:?}\ndestroot = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n",
            tmp.join(format!("db-{tag}")).display(),
            tmp.join(format!("cache-{tag}")).display(),
            tmp.join(format!("root-{tag}")).display(),
        ),
    )
    .unwrap();
    path
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_global_opts_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn base_cmd(_tmp: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oparysh"));
    cmd.env_remove("OPARYSH_CONFIG")
        .env_remove("OPARYSH_DB_DIR")
        .env_remove("OPARYSH_CACHE_DIR")
        .env_remove("OPARYSH_ROOT")
        .env_remove("OPARYSH_JOBS");
    cmd
}

#[test]
fn global_db_dir_flag_overrides_config() {
    let tmp = tmp_dir("dbflag");
    let _ = config_file(&tmp, "a");
    let override_db = tmp.join("override-db");
    let output = base_cmd(&tmp)
        .args(["--db-dir"])
        .arg(&override_db)
        .args(["--config"])
        .arg(tmp.join("oparysh-a.conf"))
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(override_db.join("db.sqlite").is_file(), "БД создана в --db-dir");
    assert!(!tmp.join("db-a").join("db.sqlite").exists(), "конфигный db_dir не трогали");
}

#[test]
fn global_cache_dir_flag_overrides_config() {
    let tmp = tmp_dir("cacheflag");
    let _ = config_file(&tmp, "a");
    let override_cache = tmp.join("override-cache");
    let output = base_cmd(&tmp)
        .args(["--cache-dir"])
        .arg(&override_cache)
        .args(["--config"])
        .arg(tmp.join("oparysh-a.conf"))
        .arg("repo")
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn global_config_flag_beats_oparysh_config_env() {
    let tmp = tmp_dir("cfgflag");
    let good = config_file(&tmp, "good");
    // OPARYSH_CONFIG указывает на несуществующий файл с «плохим» конфигом —
    // если --config работает, команда успешна.
    let bad = tmp.join("broken.conf");
    fs::write(&bad, "this is [ not toml").unwrap();
    let output = base_cmd(&tmp)
        .args(["--config"])
        .arg(&good)
        .arg("list")
        .env("OPARYSH_CONFIG", &bad)
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn oparysh_db_dir_env_is_honored() {
    let tmp = tmp_dir("dbenv");
    let _ = config_file(&tmp, "a");
    let override_db = tmp.join("env-db");
    let output = base_cmd(&tmp)
        .args(["--config"])
        .arg(tmp.join("oparysh-a.conf"))
        .arg("list")
        .env("OPARYSH_DB_DIR", &override_db)
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(override_db.join("db.sqlite").is_file(), "БД создана в OPARYSH_DB_DIR");
}

#[test]
fn verbose_flag_prints_diagnostics() {
    let tmp = tmp_dir("verbose");
    let _ = config_file(&tmp, "a");
    let output = base_cmd(&tmp)
        .args(["--verbose", "--config"])
        .arg(tmp.join("oparysh-a.conf"))
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("verbose: config="), "{stderr}");
    assert!(stderr.contains("db_dir="), "{stderr}");
}
