//! Сценарий-тест этапа 10.5 ROADMAP (приложение A DOCS.md):
//! install → upgrade → autoremove в изолированном корне.
//!
//! Сценарий: `install hello-tools` (тянутся libgreet, hello, make;
//! hello-tools — explicit, остальные — dependency) → добавление hello 1.1.0
//! в репозиторий → `upgrade` (hello обновлён, hello-tools пересобран) →
//! `remove hello-tools` → `autoremove` (сироты hello и libgreet удалены,
//! явно установленные не затронуты).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use oparysh_repo::{generate_index, render_index, GenIndexOptions, INDEX_FILE};

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

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

/// Пересобрать тарбол пакета с добавленным Makefile и обновить sha256 в рецепте.
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
    if !recipe_path.is_file() {
        return;
    }
    let raw = fs::read_to_string(&recipe_path).unwrap();
    let updated = raw
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("sha256") {
                format!("sha256 = \"{sha}\"")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&recipe_path, updated).unwrap();
}

fn add_make_package(repo: &Path) {
    let version = "4.4.1";
    let name = "make";
    repack_with_makefile(
        repo,
        name,
        version,
        "PREFIX = /usr/local\n\nall:\n\tprintf '#!/bin/sh\\nexit 0\\n' > make\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/bin\n\tinstall -m 0755 make $(DESTDIR)$(PREFIX)/bin/oparysh-make-stub\n\n.PHONY: all install\n",
    );

    let tarball = repo.join(format!("dist/{name}/{version}/{name}-{version}.tar.gz"));
    let sha = sha256_file(&tarball);
    let recipe_dir = repo.join(format!("recipes/{name}/{version}"));
    fs::create_dir_all(&recipe_dir).unwrap();
    fs::write(
        recipe_dir.join("pkg.toml"),
        format!(
            "[pkg]\nname     = \"{name}\"\nversion  = \"{version}\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"Stub make for the mini-repo test\"\nlicense  = \"MIT\"\nhomepage = \"https://example.org/make\"\n\n[source]\nfile   = \"{name}-{version}.tar.gz\"\nsha256 = \"{sha}\"\n\n[deps]\nbuild = []\nrun   = []\n\n[build]\nsystem = \"make-only\"\n\n[build.install]\nprefix = \"/usr/local\"\n\n[files]\nunknown_warn = true\n"
        ),
    )
    .unwrap();
}

/// Добавить hello 1.1.0 в репозиторий и перегенерировать индекс.
fn add_hello_1_1_0(repo: &Path) {
    let version = "1.1.0";
    repack_with_makefile(
        repo,
        "hello",
        version,
        "PREFIX = /usr/local\n\nall:\n\tprintf '#!/bin/sh\\necho Hello 1.1.0 from mini-repo\\n' > hello\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/bin\n\tinstall -m 0755 hello $(DESTDIR)$(PREFIX)/bin/hello\n\n.PHONY: all install\n",
    );
    // 1.1.0 ещё нет в recipes — пишем рецепт сами, после пересборки тарбола.
    let tarball = repo.join(format!("dist/hello/{version}/hello-{version}.tar.gz"));
    let sha = sha256_file(&tarball);
    let recipe_dir = repo.join(format!("recipes/hello/{version}"));
    fs::create_dir_all(&recipe_dir).unwrap();
    fs::write(
        recipe_dir.join("pkg.toml"),
        format!(
            "[pkg]\nname     = \"hello\"\nversion  = \"{version}\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"Hello world program from the mini repository fixture\"\nlicense  = \"MIT\"\nhomepage = \"https://example.org/hello\"\n\n[source]\nfile   = \"hello-{version}.tar.gz\"\nsha256 = \"{sha}\"\n\n[deps]\nbuild = [\"make>=4.0\"]\nrun   = [\"libgreet>=1.0\"]\n\n[build]\nsystem = \"make-only\"\n\n[build.install]\nprefix = \"/usr/local\"\n\n[files]\nunknown_warn = true\n"
        ),
    )
    .unwrap();
    regen_index(repo);
}

fn regen_index(repo: &Path) {
    let mut options = GenIndexOptions::new("mini");
    options.origin = Some("https://example.org/mini".to_string());
    options.updated = Some(1_704_067_200);
    let generated = generate_index(repo, &options).unwrap();
    fs::write(repo.join(INDEX_FILE), render_index(&generated).unwrap()).unwrap();
}

/// Подготовить копию мини-репозитория, пригодную для сборки.
fn prepared_repo(tmp: &Path) -> PathBuf {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mini-repo");
    let repo = tmp.join("repo");
    copy_dir(&fixture, &repo);

    repack_with_makefile(
        &repo,
        "libgreet",
        "1.0.0",
        "PREFIX = /usr/local\n\nall:\n\tprintf 'int greet(void){return 0;}\\n' > greet.c\n\t$(CC) -O2 -c greet.c\n\tar rcs libgreet.a greet.o\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/lib $(DESTDIR)$(PREFIX)/include\n\tinstall -m 0644 libgreet.a $(DESTDIR)$(PREFIX)/lib/libgreet.a\n\tprintf 'int greet(void);\\n' > $(DESTDIR)$(PREFIX)/include/greet.h\n\n.PHONY: all install\n",
    );
    repack_with_makefile(
        &repo,
        "hello",
        "1.0.0",
        "PREFIX = /usr/local\n\nall:\n\tprintf '#!/bin/sh\\necho Hello from mini-repo\\n' > hello\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/bin\n\tinstall -m 0755 hello $(DESTDIR)$(PREFIX)/bin/hello\n\n.PHONY: all install\n",
    );
    repack_with_makefile(
        &repo,
        "hello-tools",
        "1.0.0",
        "PREFIX = /usr/local\n\nall:\n\tprintf '#!/bin/sh\\necho hello-tools from mini-repo\\n' > hello-tools\n\ninstall:\n\tinstall -d $(DESTDIR)$(PREFIX)/bin\n\tinstall -m 0755 hello-tools $(DESTDIR)$(PREFIX)/bin/hello-tools\n\n.PHONY: all install\n",
    );
    add_make_package(&repo);
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
             [[repositories.entries]]\nname = \"mini\"\npath = {:?}\n",
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
    let dir = std::env::temp_dir().join(format!("oparysh_scenario_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn install_upgrade_autoremove_scenario() {
    let tmp = tmp_dir("scenario");
    let repo = prepared_repo(&tmp);
    let _ = config_file(&tmp, &repo);
    let root = tmp.join("root");

    // 1. Установка цепочки (как в приложении A: install с тулчейном/флагами из конфига).
    let (status, stdout, stderr) = run(&tmp, &["install", "hello-tools", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(root.join("usr/local/bin/hello-tools").is_file());
    assert!(root.join("usr/local/bin/hello").is_file());
    assert!(root.join("usr/local/lib/libgreet.a").is_file());

    // Причины установки: hello-tools — explicit, остальные — dependency.
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    let reason = |name: &str| db.get_package(name).unwrap().unwrap().reason.clone();
    assert_eq!(reason("hello-tools"), "explicit");
    assert_eq!(reason("hello"), "dependency");
    assert_eq!(reason("libgreet"), "dependency");
    drop(db);

    // 2. Обновление: добавляем hello 1.1.0 в репозиторий, sync не требуется (path-репо).
    add_hello_1_1_0(&repo);
    let (status, stdout, stderr) = run(&tmp, &["upgrade", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    assert_eq!(db.get_package("hello").unwrap().unwrap().version, "1.1.0");
    assert_eq!(db.get_package("hello-tools").unwrap().unwrap().build_rev, 2);
    drop(db);
    let out = Command::new(root.join("usr/local/bin/hello")).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("1.1.0"));

    // 3. Удаляем явно установленный hello-tools; сироты остаются.
    let (status, stdout, stderr) = run(&tmp, &["remove", "hello-tools", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(!root.join("usr/local/bin/hello-tools").exists());
    // Сироты пока на месте (autoremove ещё не вызван).
    assert!(root.join("usr/local/bin/hello").is_file());

    // 4. autoremove: удаляет сирот hello и libgreet, не трогает явные/нужные.
    let (status, stdout, stderr) = run(&tmp, &["autoremove", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");

    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    assert!(db.get_package("hello").unwrap().is_none(), "hello должен быть удалён autoremove");
    assert!(db.get_package("libgreet").unwrap().is_none(), "libgreet должен быть удалён autoremove");
    assert!(db.get_package("hello-tools").unwrap().is_none(), "hello-tools удалён remove");
    drop(db);
    assert!(!root.join("usr/local/bin/hello").exists());
    assert!(!root.join("usr/local/lib/libgreet.a").exists());

    // 5. history отражает install → upgrade → remove → autoremove.
    let (status, stdout, stderr) = run(&tmp, &["history"]);
    assert!(status.success(), "stderr: {stderr}");
    for action in ["install", "upgrade", "remove", "autoremove"] {
        assert!(stdout.contains(action), "history не содержит {action}:\n{stdout}");
    }

    // Повторный autoremove — идемпотентен (нечего удалять).
    let (status, stdout, stderr) = run(&tmp, &["autoremove", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");

    fs::remove_dir_all(&tmp).unwrap();
}
