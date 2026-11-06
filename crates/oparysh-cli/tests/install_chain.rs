//! Интеграционный тест `oparysh install` цепочки из нескольких пакетов
//! в изолированном корне (этап 8.8 ROADMAP).
//!
//! Фикстура `tests/fixtures/mini-repo` — локальный репозиторий из трёх
//! пакетов (`libgreet` → `hello` → `hello-tools`). Тарболы фикстуры —
//! заглушки (только README, без Makefile), поэтому здесь репозиторий
//! копируется во временную директорию, тарболы пересобираются с настоящими
//! Makefile, добавляется пакет `make` (build-зависимость фикстуры), и
//! индекс перегенерируется. Исходная фикстура не изменяется.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use oparysh_repo::{generate_index, render_index, GenIndexOptions, INDEX_FILE};

/// Рекурсивное копирование каталога (фикстура маленькая).
fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
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

/// Пересобрать тарбол пакета с добавленным Makefile и вернуть sha256.
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

    // Обновить sha256 источника в рецепте (если рецепт уже существует;
    // для нового пакета `make` рецепт пишется вызывающей стороной).
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

/// Добавить пакет `make` (build-зависимость фикстуры: `make>=4.0`).
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
            "[pkg]\nname     = \"{name}\"\nversion  = \"{version}\"\nbuild    = 1\narch     = [\"x86_64\"]\ndesc     = \"Stub make for the mini-repo install test\"\nlicense  = \"MIT\"\nhomepage = \"https://example.org/make\"\n\n[source]\nfile   = \"{name}-{version}.tar.gz\"\nsha256 = \"{sha}\"\n\n[deps]\nbuild = []\nrun   = []\n\n[build]\nsystem = \"make-only\"\n\n[build.install]\nprefix = \"/usr/local\"\n\n[files]\nunknown_warn = true\n"
        ),
    )
    .unwrap();
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

    // Перегенерировать index.toml по содержимому копии.
    let mut options = GenIndexOptions::new("mini");
    options.origin = Some("https://example.org/mini".to_string());
    options.updated = Some(1_704_067_200);
    let generated = generate_index(&repo, &options).unwrap();
    fs::write(repo.join(INDEX_FILE), render_index(&generated).unwrap()).unwrap();

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
    let dir = std::env::temp_dir().join(format!("oparysh_install_chain_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn install_chain_libgreet_hello_hello_tools() {
    let tmp = tmp_dir("chain");
    let repo = prepared_repo(&tmp);
    let _ = config_file(&tmp, &repo);
    let root = tmp.join("root");

    let (status, stdout, stderr) = run(&tmp, &["install", "hello-tools", "--yes"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");

    // Топологический порядок: libgreet → hello → hello-tools (make — build-dep).
    let pos = |needle: &str| stdout.find(needle).unwrap_or_else(|| panic!("{needle} не найден в выводе:\n{stdout}"));
    assert!(pos("] libgreet ") < pos("] hello "), "{stdout}");
    assert!(pos("] hello ") < pos("] hello-tools "), "{stdout}");

    // Файлы в изолированном destroot.
    assert!(root.join("usr/local/lib/libgreet.a").is_file(), "{stdout}");
    assert!(root.join("usr/local/include/greet.h").is_file(), "{stdout}");
    assert!(root.join("usr/local/bin/hello").is_file(), "{stdout}");
    assert!(root.join("usr/local/bin/hello-tools").is_file(), "{stdout}");

    // БД: все четыре пакета (включая build-dep make) записаны.
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    for name in ["make", "libgreet", "hello", "hello-tools"] {
        let pkg = db.get_package(name).unwrap().unwrap_or_else(|| panic!("{name} не в БД"));
        assert_eq!(pkg.name, name);
    }
    // Рантайм-зависимости зафиксированы: hello → libgreet, hello-tools → hello.
    let hello_deps = db.get_deps("hello").unwrap();
    assert!(hello_deps.iter().any(|d| d.dep_name == "libgreet"), "{hello_deps:?}");
    let tools_deps = db.get_deps("hello-tools").unwrap();
    assert!(tools_deps.iter().any(|d| d.dep_name == "hello"), "{tools_deps:?}");

    // Установленный скрипт работает из изолированного корня.
    let out = Command::new(root.join("usr/local/bin/hello")).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Hello from mini-repo"));
}
