//! Этап 11.3: единые коды возврата (DOCS §11, 0–9) во всех командах.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn config_file(tmp: &Path) -> PathBuf {
    let path = tmp.join("oparysh.conf");
    fs::write(
        &path,
        format!(
            "[general]\njobs = 2\ndb_dir = {:?}\ncache_dir = {:?}\ndestroot = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n",
            tmp.join("db").display(),
            tmp.join("cache").display(),
            tmp.join("root").display(),
        ),
    )
    .unwrap();
    path
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_exit_codes_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_oparysh"))
        .args(args)
        .env("OPARYSH_CONFIG", tmp.join("oparysh.conf"))
        .env_remove("OPARYSH_DB_DIR")
        .env_remove("OPARYSH_CACHE_DIR")
        .env_remove("OPARYSH_ROOT")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn fixture_tarball(tmp: &Path) -> PathBuf {
    let path = tmp.join("artifact.tar.gz");
    fs::write(&path, b"not a real tarball, just bytes").unwrap();
    path
}

#[test]
fn exit_2_on_invalid_cli_args() {
    let tmp = tmp_dir("args");
    let _ = config_file(&tmp);
    let (code, _, _) = run(&tmp, &["--definitely-not-a-flag"]);
    assert_eq!(code, 2);
    let (code, _, _) = run(&tmp, &["check", "--definitely-not-a-flag"]);
    assert_eq!(code, 2);
}

#[test]
fn exit_2_on_invalid_oparysh_jobs() {
    let tmp = tmp_dir("jobs");
    let _ = config_file(&tmp);
    let output = Command::new(env!("CARGO_BIN_EXE_oparysh"))
        .args(["list"])
        .env("OPARYSH_CONFIG", tmp.join("oparysh.conf"))
        .env("OPARYSH_JOBS", "not-a-number")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn exit_3_on_unknown_package() {
    let tmp = tmp_dir("notfound");
    let _ = config_file(&tmp);
    for args in [
        vec!["check", "no-such-pkg"],
        vec!["remove", "no-such-pkg"],
        vec!["info", "no-such-pkg"],
    ] {
        let (code, stdout, stderr) = run(&tmp, &args);
        assert_eq!(code, 3, "args: {args:?}\nstdout: {stdout}\nstderr: {stderr}");
    }
}

#[test]
fn exit_6_on_sha256_mismatch() {
    let tmp = tmp_dir("sha");
    let _ = config_file(&tmp);
    let tarball = fixture_tarball(&tmp);
    let wrong = "0".repeat(64);
    let (code, _, stderr) = run(&tmp, &["verify", tarball.to_str().unwrap(), "--expected", &wrong]);
    assert_eq!(code, 6, "{stderr}");
}

#[test]
fn exit_0_on_success() {
    let tmp = tmp_dir("ok");
    let _ = config_file(&tmp);
    let (code, _, _) = run(&tmp, &["list"]);
    assert_eq!(code, 0);
}
