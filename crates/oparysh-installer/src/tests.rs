//! Юнит-тесты этапов 8.1–8.4: DESTDIR, manifest scan, commit, cleanup.

use super::*;
use std::fs;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "oparysh_installer_test_{}_{}_{}",
        std::process::id(),
        n,
        tag
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_file(root: &Path, rel: &str, content: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const RECIPE: &str = r#"
[pkg]
name = "hello"
version = "1.2.3"
build = 7
arch = ["x86_64"]
desc = "test pkg"
license = "MIT"
homepage = "https://example.org"

[source]
url = "https://example.org/hello-1.2.3.tar.gz"
sha256 = "deadbeef"

[deps]
run = ["libc>=2.0", "zlib"]
build = ["pkg-config"]

[build]
system = "make-only"

[build.install]
prefix = "/usr/local"
"#;

fn parse_recipe() -> Recipe {
    Recipe::parse(RECIPE).unwrap()
}

fn dep_snapshot() -> DepSnapshot {
    let r = parse_recipe();
    DepSnapshot {
        run: r.deps.run.clone(),
        build: r.deps.build.clone(),
    }
}

fn cleanup_dirs(dirs: &[&PathBuf]) {
    for d in dirs {
        let _ = fs::remove_dir_all(d);
    }
}

#[test]
fn destdir_path_and_prepare() {
    let db = temp_dir("destdir");
    let d = destdir(&db, "hello", "1.2.3");
    assert_eq!(d, db.join("tmp").join("hello-1.2.3-destdir"));
    prepare_destdir(&d).unwrap();
    assert!(d.is_dir());
    make_file(&d, "file.txt", b"x");
    // Повторная подготовка очищает каталог.
    prepare_destdir(&d).unwrap();
    assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
    cleanup_dirs(&[&db]);
}

#[test]
fn scan_and_commit_to_temp_root() {
    let db = temp_dir("scan_db");
    let destroot = temp_dir("scan_root");
    let ddir = destdir(&db, "hello", "1.2.3");
    prepare_destdir(&ddir).unwrap();
    make_file(&ddir, "usr/local/bin/hello", b"#!/bin/sh\necho hi\n");
    make_file(&ddir, "usr/local/share/doc/hello/README", b"docs");
    make_file(&ddir, "etc/stray.conf", b"stray");

    let recipe = parse_recipe();
    // default policy: unknown_warn=false → warn не печатается
    let manifest = scan_manifest(
        &ddir,
        &destroot,
        &recipe.build.install.prefix,
        ScanPolicy::default(),
    )
    .unwrap();
    assert!(!manifest.is_empty);
    assert_eq!(manifest.files.len(), 3);
    assert_eq!(manifest.unknown, vec!["etc/stray.conf".to_string()]);
    // sha256 совпадает с посчитанным напрямую.
    let want = oparysh_verifier::sha256_file(&ddir.join("usr/local/bin/hello")).unwrap();
    let entry = manifest.files.iter().find(|f| f.rel == "usr/local/bin/hello").unwrap();
    assert_eq!(entry.sha256, want);

    // --strict → ошибка про незаявленный файл.
    let err = scan_manifest(
        &ddir,
        &destroot,
        &recipe.build.install.prefix,
        ScanPolicy { unknown_warn: true, strict: true },
    )
    .unwrap_err();
    match err {
        InstallError::UnknownFiles { unknown } => {
            assert_eq!(unknown, vec!["etc/stray.conf".to_string()]);
        }
        other => panic!("ожидали UnknownFiles, получили {other}"),
    }

    // Переносим в destroot и пишем в БД единой транзакцией.
    let moved = move_to_destroot(&manifest).unwrap();
    assert_eq!(moved, manifest.size_bytes());
    assert!(destroot.join("usr/local/bin/hello").is_file());
    assert!(destroot.join("etc/stray.conf").is_file());
    assert!(!ddir.join("usr/local/bin/hello").exists());

    let mut database = Database::open(&db).unwrap();
    let deps = dep_snapshot();
    let build = BuildRecord {
        cc: "gcc".into(),
        cflags: "-O2".into(),
        makeopts: "-j8".into(),
        profile: "release".into(),
        cmd: "make && make install".into(),
    };
    // pkg_hash рецепта — sha256 сырца рецепта.
    let pkg_hash = hex_sha(RECIPE.as_bytes());
    let out = commit_package(&mut database, &recipe, &manifest, &deps, &build, &pkg_hash).unwrap();
    assert_eq!(out.name, "hello");
    assert_eq!(out.files, 3);

    // Проверяем, что все четыре таблицы заполнены в БД.
    let pkg = database.get_package("hello").unwrap().unwrap();
    assert_eq!(pkg.version, "1.2.3");
    assert_eq!(pkg.build_rev, 7);
    assert_eq!(pkg.arch, "x86_64");
    assert_eq!(pkg.pkg_hash, pkg_hash);
    assert_eq!(pkg.size_bytes, manifest.size_bytes());

    let files = database.get_files("hello").unwrap();
    assert_eq!(files.len(), 3);
    let bin = files.iter().find(|f| f.path.ends_with("usr/local/bin/hello")).unwrap();
    assert_eq!(bin.sha256, want);
    // Путь в БД — реальный (destroot + rel), не DESTDIR.
    assert!(bin.path.starts_with(destroot.to_string_lossy().as_ref()));

    let db_deps = database.get_deps("hello").unwrap();
    assert_eq!(db_deps.len(), 2); // только deps.run
    assert!(db_deps.iter().any(|d| d.dep_name == "zlib"));
    assert!(db_deps.iter().any(|d| d.dep_name == "libc" && d.req == ">=2.0.0"));

    let info = database.get_build_info("hello").unwrap().unwrap();
    assert_eq!(info.cc, "gcc");
    assert_eq!(info.profile, "release");

    // Перефикс/переустановка перезаписывает строки без дублирования.
    move_to_destroot(&Manifest::default()).unwrap(); // пустой манифест — не паника
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn commit_package_reinstall_replaces() {
    let db = temp_dir("reinst_db");
    let destroot = temp_dir("reinst_root");
    let ddir = destdir(&db, "hello", "1.2.3");
    prepare_destdir(&ddir).unwrap();
    make_file(&ddir, "usr/local/bin/hello", b"one");
    let recipe = parse_recipe();
    let manifest = scan_manifest(&ddir, &destroot, "/usr/local", ScanPolicy::default()).unwrap();
    move_to_destroot(&manifest).unwrap();
    let mut database = Database::open(&db).unwrap();
    let deps = dep_snapshot();
    let build = BuildRecord::default();
    commit_package(&mut database, &recipe, &manifest, &deps, &build, "h1").unwrap();
    // «Переустановка»: тот же пакет, другие данные — строки должны
    // перезаписаться одной транзакцией, без дублей.
    let out = commit_package(&mut database, &recipe, &manifest, &deps, &build, "h2").unwrap();
    assert_eq!(out.files, manifest.files.len());
    let files = database.get_files("hello").unwrap();
    assert_eq!(files.len(), manifest.files.len()); // нет дублей
    assert_eq!(database.get_package("hello").unwrap().unwrap().pkg_hash, "h2");
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn cache_recipe_and_cleanup() {
    let db = temp_dir("cache_db");
    let build_dir = db.join("build").join("mini-1.0");
    fs::create_dir_all(&build_dir).unwrap();
    fs::write(build_dir.join("obj.o"), b"obj").unwrap();
    let ddir = destdir(&db, "mini", "1.0");
    prepare_destdir(&ddir).unwrap();
    make_file(&ddir, "usr/local/bin/mini", b"bin");

    let path = cache_recipe(&db, "mini", "1.0", "# recipe\n").unwrap();
    assert_eq!(path, db.join("recipes").join("mini").join("1.0").join("pkg.toml"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# recipe\n");

    // keep_build: build/ сохраняется, destdir удаляется.
    let out = cleanup(&db, "mini", "1.0", true).unwrap();
    assert!(!out.build_removed);
    assert!(out.destdir_removed);
    assert!(build_dir.is_dir());

    // Полный cleanup: и build/, и destdir удаляются.
    prepare_destdir(&ddir).unwrap();
    make_file(&ddir, "usr/local/bin/mini", b"bin");
    let out = cleanup(&db, "mini", "1.0", false).unwrap();
    assert!(out.build_removed);
    assert!(out.destdir_removed);
    assert!(!build_dir.exists());
    assert!(!ddir.exists());

    cleanup_dirs(&[&db]);
}
