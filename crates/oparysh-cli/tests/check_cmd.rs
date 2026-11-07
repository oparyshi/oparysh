//! Интеграционные тесты `oparysh check` (этап 11.1/11.6 ROADMAP):
//! целостность установленных файлов против БД — `removed | modified | new`,
//! коды возврата 0/1/3, полный аудит без аргументов.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// Копия фикстуры `tests/fixtures/hello-1.0` с настоящим тарболом и sha256.
fn recipe_dir(tmp: &Path) -> PathBuf {
    let dir = tmp.join("pkghello");
    fs::create_dir_all(&dir).unwrap();

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hello-1.0");
    for file in ["hello.c", "Makefile"] {
        fs::copy(fixture.join(file), dir.join(file)).unwrap();
    }
    let tarball = tmp.join("hello-1.0.tar.gz");
    let status = Command::new("tar")
        .arg("czf")
        .arg(&tarball)
        .arg("hello.c")
        .arg("Makefile")
        .current_dir(&dir)
        .status()
        .unwrap();
    assert!(status.success(), "tar не собрал фикс-тарбол");

    let sha = String::from_utf8(
        Command::new("sha256sum")
            .arg(&tarball)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let sha = sha.split_whitespace().next().unwrap().to_string();

    let raw = fs::read_to_string(fixture.join("pkg.toml")).unwrap();
    let recipe = raw
        .replace("file:///fixtures/hello-1.0/hello-1.0.tar.gz", &format!("file://{}", tarball.display()))
        .replace(
            "0000000000000000000000000000000000000000000000000000000000000000",
            &sha,
        );
    fs::write(dir.join("pkg.toml"), recipe).unwrap();
    dir
}

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

fn run(tmp: &Path, args: &[&str]) -> (ExitStatus, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_oparysh"))
        .args(args)
        .env("LC_ALL", "ru_RU.UTF-8")
        .env("OPARYSH_CONFIG", tmp.join("oparysh.conf"))
        .env_remove("OPARYSH_DB_DIR")
        .env_remove("OPARYSH_CACHE_DIR")
        .env_remove("OPARYSH_ROOT")
        .env_remove("OPARYSH_NO_COLORS")
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_check_cmd_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Установить hello из локального рецепта; вернуть (tmp, путь к установленному файлу).
fn install_hello(tag: &str) -> (PathBuf, PathBuf) {
    let tmp = tmp_dir(tag);
    let dir = recipe_dir(&tmp);
    let _ = config_file(&tmp);
    // `build --install`: собирает пакет из локального рецепта без разрешения
    // build-зависимостей из репозиториев (make есть в системе test-runner'а).
    let (status, stdout, stderr) = run(
        &tmp,
        &["build", dir.to_str().unwrap(), "--install"],
    );
    assert!(status.success(), "install failed\nstdout: {stdout}\nstderr: {stderr}");
    let installed = tmp.join("root/usr/local/bin/hello");
    assert!(installed.is_file(), "файл установлен: {stdout}");
    (tmp, installed)
}

#[test]
fn check_ok_after_install() {
    let (tmp, _) = install_hello("ok");
    let (status, stdout, stderr) = run(&tmp, &["check"]);
    assert_eq!(status.code(), Some(0), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("все файлы целы"), "{stdout}");
}

#[test]
fn check_named_package_and_full_audit() {
    let (tmp, _) = install_hello("named");
    let (status, stdout, _) = run(&tmp, &["check", "hello"]);
    assert_eq!(status.code(), Some(0), "{stdout}");
    // Полный аудит без аргументов тоже работает.
    let (status, stdout, _) = run(&tmp, &["check"]);
    assert_eq!(status.code(), Some(0), "{stdout}");
}

#[test]
fn check_detects_modified_file() {
    let (tmp, installed) = install_hello("modified");
    fs::write(&installed, b"#!/bin/sh\necho tampered\n").unwrap();
    let (status, stdout, _) = run(&tmp, &["check"]);
    assert_eq!(status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("modified"), "{stdout}");
    assert!(stdout.contains("hello"), "{stdout}");
}

#[test]
fn check_detects_removed_file() {
    let (tmp, installed) = install_hello("removed");
    fs::remove_file(&installed).unwrap();
    let (status, stdout, _) = run(&tmp, &["check"]);
    assert_eq!(status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("removed"), "{stdout}");
}

#[test]
fn check_detects_new_file_in_package_dir() {
    let (tmp, installed) = install_hello("new");
    let stowaway = installed.parent().unwrap().join("stowaway");
    fs::write(&stowaway, b"unexpected\n").unwrap();
    let (status, stdout, _) = run(&tmp, &["check"]);
    assert_eq!(status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("new"), "{stdout}");
    assert!(stdout.contains("stowaway"), "{stdout}");
}

#[test]
fn check_unknown_package_exits_3() {
    let tmp = tmp_dir("unknown");
    let _ = config_file(&tmp);
    let (status, _, stderr) = run(&tmp, &["check", "no-such-pkg"]);
    assert_eq!(status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("не установлен"), "{stderr}");
}
