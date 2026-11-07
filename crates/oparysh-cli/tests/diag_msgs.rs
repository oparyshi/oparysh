//! Диагностика ошибок команд `deps`/`downgrade` (этапы 5.8/10.2):
//! молчаливый `exit(3)` считается багом UX — CLI обязан печатать причину.
//! Смотри `run_deps()` в `crates/oparysh-cli/src/main.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn config_file(tmp: &Path, repo_path: Option<&Path>) -> PathBuf {
    let path = tmp.join("oparysh.conf");
    let repo_block = match repo_path {
        Some(p) => format!(
            "[[repositories.entries]]\nname = \"test\"\npath = {:?}\n",
            p.display()
        ),
        None => String::new(),
    };
    fs::write(
        &path,
        format!(
            "[general]\njobs = 2\ndb_dir = {:?}\ncache_dir = {:?}\ndestroot = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n{}",
            tmp.join("db").display(),
            tmp.join("cache").display(),
            tmp.join("root").display(),
            repo_block,
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
        .output()
        .unwrap();
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_diag_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Зарегистрированный, но сломанный репозиторий: отсутствии index.toml.
#[test]
fn deps_on_broken_repo_prints_error() {
    let tmp = tmp_dir("broken");
    let missing = tmp.join("doesnotexist");
    config_file(&tmp, Some(&missing));

    let (status, stdout, stderr) = run(&tmp, &["deps", "hello"]);
    assert!(
        !status.success(),
        "deps должен ошибиться: {stdout}{stderr}"
    );
    assert_eq!(status.code(), Some(3), "{stdout}{stderr}");
    assert!(
        stderr.contains("ERROR"),
        "`deps` обязан печатать причину в stderr:\n{stderr}"
    );
}

/// Пакет не найден ни в репозиториях, ни в БД — тоже печатаем причину.
#[test]
fn deps_unknown_package_prints_error() {
    let tmp = tmp_dir("nosuch");
    // Вместо репозитория — кривой путь, чтобы resolve гарантированно упал.
    let missing = tmp.join("doesnotexist");
    config_file(&tmp, Some(&missing));

    let (status, stdout, stderr) = run(&tmp, &["deps", "no-such-pkg"]);
    assert_eq!(status.code(), Some(3), "{stdout}{stderr}");
    assert!(
        stderr.contains("ERROR"),
        "`deps` обязан печатать причину в stderr:\n{stderr}"
    );
}

/// `downgrade` на несуществующую версию: ровно одно сообщение ERROR (код 3).
///
/// Пакет должен быть установлен (`build ... --install` из локального рецепта),
/// т.к. проверяется ветка «нет такой версии ни в репо, ни в кэше».
#[test]
fn downgrade_unknown_version_prints_single_error() {
    let tmp = tmp_dir("downgrade");

    // Локальный рецепт по образцу fixtures/hello-1.0 (make присутствует в системе).
    let dir = tmp.join("pkgstub");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("Makefile"), "all:\ninstall:\n\tinstall -d $(DESTDIR)/usr/local/bin\n\tprintf '#!/bin/sh\\ntrue\\n' > $(DESTDIR)/usr/local/bin/stub\n\n.PHONY: all install\n").unwrap();

    let tarball = tmp.join("stub-1.0.tar.gz");
    let status = Command::new("tar")
        .arg("czf")
        .arg(&tarball)
        .arg("Makefile")
        .current_dir(&dir)
        .status()
        .unwrap();
    assert!(status.success(), "tar не собрал тарбол");

    let out = Command::new("sha256sum").arg(&tarball).output().unwrap();
    let sha = String::from_utf8(out.stdout).unwrap();
    let sha = sha.split_whitespace().next().unwrap().to_string();

    fs::write(
        dir.join("pkg.toml"),
        format!(
            "[pkg]\nname     = \"stub\"\nversion  = \"1.0\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"stub\"\nlicense  = \"MIT\"\n\n\
             [source]\nurl      = \"file://{}\"\nsha256   = \"{sha}\"\n\n\
             [deps]\nbuild = []\nrun   = []\n\n\
             [build]\nsystem   = \"make-only\"\n\n\
             [build.install]\nprefix = \"/usr/local\"\n\n\
             [files]\nunknown_warn = true\n",
            tarball.display(),
        ),
    )
    .unwrap();

    config_file(&tmp, None);
    let (status, stdout, stderr) = run(&tmp, &["build", dir.to_str().unwrap(), "--install"]);
    assert!(status.success(), "установка не удалась: {stdout}{stderr}");

    let (status, stdout, stderr) = run(&tmp, &["downgrade", "stub", "9.9.9", "--yes"]);
    assert_eq!(status.code(), Some(3), "{stdout}{stderr}");
    let errors = stderr.lines().filter(|l| l.contains("ERROR:")).count();
    assert_eq!(errors, 1, "должно быть ровно одно сообщение ERROR:\n{stderr}");
}
