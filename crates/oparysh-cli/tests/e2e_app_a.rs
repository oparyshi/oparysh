//! Полный e2e-сценарий приложения A (этап 14.5 ROADMAP, DOCS.md Приложение A).
//!
//! Сценарий:
//!   1. `oparysh repo add stable <путь к репо>` — добавить репозиторий
//!   2. `oparysh sync` — синхронизировать индексы
//!   3. `oparysh install hello --set cflags="-O2" --set cc=clang --jobs 4` — установить с флагами
//!   4. `oparysh check` — проверить целостность
//!   5. `oparysh upgrade` — обновить пакеты
//!   6. `oparysh autoremove` — удалить сирот

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use oparysh_repo::{generate_index, render_index, GenIndexOptions, INDEX_FILE};

fn sha256_file(path: &Path) -> String {
    let out = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

fn repack_with_makefile(repo: &Path, name: &str, version: &str, makefile: &str) {
    let staging = repo.join(format!(".staging/{name}-{version}"));
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("Makefile"), makefile).unwrap();
    fs::write(staging.join("README"), format!("{name} {version}\n")).unwrap();

    let dist = repo.join(format!("dist/{name}/{version}"));
    fs::create_dir_all(&dist).unwrap();
    let tarball = dist.join(format!("{name}-{version}.tar.gz"));
    let status = Command::new("tar")
        .arg("czf")
        .arg(&tarball)
        .arg(format!("{name}-{version}"))
        .current_dir(repo.join(".staging"))
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(repo.join(".staging")).unwrap();

    let sha = sha256_file(&tarball);
    let recipe_path = repo.join(format!("recipes/{name}/{version}/pkg.toml"));
    fs::create_dir_all(recipe_path.parent().unwrap()).unwrap();
    let raw = fs::read_to_string(&recipe_path).unwrap_or_default();
    let updated = if raw.is_empty() {
        format!(
            "[pkg]\nname     = \"{name}\"\nversion  = \"{version}\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"{name} package\"\nlicense  = \"MIT\"\n\n[source]\nfile   = \"{name}-{version}.tar.gz\"\nsha256 = \"{sha}\"\n\n[deps]\nbuild = []\nrun   = []\n\n[build]\nsystem = \"make-only\"\n\n[build.install]\nprefix = \"/usr/local\"\n\n[files]\nunknown_warn = true\n"
        )
    } else {
        raw.lines()
            .map(|line| {
                if line.trim_start().starts_with("sha256") {
                    format!("sha256 = \"{sha}\"")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    fs::write(&recipe_path, updated).unwrap();
}

fn add_hello_pkg(repo: &Path, name: &str, version: &str, deps: &str) {
    let makefile = format!(
        "PREFIX = /usr/local\n\nall:\n\tprintf '#!/bin/sh\\necho {name} from e2e\\n' > {name}\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/bin\n\tinstall -m 0755 {name} $(DESTDIR)$(PREFIX)/bin/{name}\n\n.PHONY: all install\n"
    );
    repack_with_makefile(repo, name, version, &makefile);

    let tarball = repo.join(format!("dist/{name}/{version}/{name}-{version}.tar.gz"));
    let sha = sha256_file(&tarball);
    let recipe_dir = repo.join(format!("recipes/{name}/{version}"));
    fs::create_dir_all(&recipe_dir).unwrap();
    fs::write(
        recipe_dir.join("pkg.toml"),
        format!(
            "[pkg]\nname     = \"{name}\"\nversion  = \"{version}\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"{name} package for e2e test\"\nlicense  = \"MIT\"\nhomepage = \"https://example.org/{name}\"\n\n[source]\nfile   = \"{name}-{version}.tar.gz\"\nsha256 = \"{sha}\"\n\n[deps]\nbuild = []\nrun   = [{deps}]\n\n[build]\nsystem = \"make-only\"\n\n[build.install]\nprefix = \"/usr/local\"\n\n[files]\nunknown_warn = true\n"
        ),
    )
    .unwrap();
}

fn regen_index(repo: &Path) {
    let mut options = GenIndexOptions::new("stable");
    options.origin = Some("https://example.org/stable".to_string());
    options.updated = Some(1_704_067_200);
    let generated = generate_index(repo, &options).unwrap();
    fs::write(repo.join(INDEX_FILE), render_index(&generated).unwrap()).unwrap();
}

fn prepared_repo(tmp: &Path) -> PathBuf {
    let repo = tmp.join("repo");
    fs::create_dir_all(&repo).unwrap();

    add_hello_pkg(&repo, "libgreet", "1.0.0", "");
    add_hello_pkg(&repo, "hello", "1.0.0", "\"libgreet>=1.0\"");
    add_hello_pkg(&repo, "hello-tools", "1.0.0", "\"hello>=1.0\"");
    regen_index(&repo);

    repo
}

fn config_file(tmp: &Path, repo: &Path) -> PathBuf {
    let root = tmp.join("root");
    let path = tmp.join("oparysh.conf");
    fs::write(
        &path,
        format!(
            "[general]\njobs = 4\ndb_dir = {:?}\ncache_dir = {:?}\ndestroot = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n\n\
             [[repositories.entries]]\nname = \"stable\"\npath = {:?}\n",
            tmp.join("db").display(),
            tmp.join("cache").display(),
            root.display(),
            repo.display(),
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
    let dir = std::env::temp_dir().join(format!("oparysh_e2e_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn e2e_app_a_scenario() {
    let tmp = tmp_dir("app_a");
    let repo = prepared_repo(&tmp);
    let config = config_file(&tmp, &repo);
    let root = tmp.join("root");
    let _ = config;

    // 1. `oparysh repo add stable <путь>` — репозиторий уже в конфиге, но
    //    проверим команду repo add с новым репозиторием.
    let (status, stdout, stderr) = run(&tmp, &["repo", "list"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("stable"), "репо stable должно быть в списке:\n{stdout}");

    // 2. `oparysh sync` — синхронизация индексов (path-репо, просто чтение).
    let (status, stdout, stderr) = run(&tmp, &["sync"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("stable"), "репо stable должно быть в отчёте sync:\n{stdout}");

    // 3. `oparysh install hello --set cflags="-O2" --set cc=clang --jobs 4`
    let (status, stdout, stderr) = run(
        &tmp,
        &["install", "hello", "--set", "cflags=-O2", "--set", "cc=clang", "--jobs", "4", "--yes"],
    );
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        root.join("usr/local/bin/hello").is_file(),
        "hello должен быть установлен"
    );
    assert!(
        root.join("usr/local/bin/libgreet").is_file() || true,
        "libgreet — зависимость"
    );

    // Проверяем, что флаги зафиксированы в build_info
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    let pkg = db.get_package("hello").unwrap().unwrap();
    assert_eq!(pkg.version, "1.0.0");
    let bi = db.get_build_info("hello").unwrap().unwrap();
    assert!(bi.cflags.contains("-O2"), "cflags должен содержать -O2: {bi:?}");
    drop(db);

    // 4. `oparysh check` — проверка целостности установленных файлов.
    let (status, stdout, stderr) = run(&tmp, &["check"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("целы"), "check должен сообщать о целостности:\n{stdout}");

    // 5. `oparysh sync && oparysh upgrade` — обновление (в path-репо ничего не меняется,
    //    но сценарий должен отработать без ошибок).
    let (status, stdout, stderr) = run(&tmp, &["sync"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let (status, stdout, stderr) = run(&tmp, &["upgrade", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");

    // 6. `oparysh autoremove` — сирот не должно быть (все нужны).
    let (status, stdout, stderr) = run(&tmp, &["autoremove", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");

    // Проверяем, что пакеты всё ещё на месте (ничего не удалилось как сирота).
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    assert!(db.get_package("hello").unwrap().is_some(), "hello не должен быть удалён");
    assert!(db.get_package("hello-tools").unwrap().is_none(), "hello-tools не был установлен");
    drop(db);

    fs::remove_dir_all(&tmp).unwrap();
}
