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
    let out = commit_package(&mut database, &recipe, &manifest, &deps, &build, &pkg_hash, "explicit").unwrap();
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
    commit_package(&mut database, &recipe, &manifest, &deps, &build, "h1", "explicit").unwrap();
    // «Переустановка»: тот же пакет, другие данные — строки должны
    // перезаписаться одной транзакцией, без дублей.
    let out = commit_package(&mut database, &recipe, &manifest, &deps, &build, "h2", "explicit").unwrap();
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

// ━━━━━━━━━━━━━━━━━━ 9.5: тесты удаления ━━━━━━━━━━━━━━━━━━

fn install_pkg(
    database: &mut Database,
    db_dir: &Path,
    destroot: &Path,
    name: &str,
    version: &str,
    deps_run: &[&str],
    reason: &str,
) {
    let rel = format!("usr/local/bin/{name}");
    let dest = destroot.join(&rel);
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    fs::write(&dest, format!("binary {name}").as_bytes()).unwrap();

    let manifest = Manifest {
        files: vec![ManifestEntry {
            rel: rel.clone(),
            src: dest.clone(),
            dest: dest.clone(),
            sha256: hex_sha(format!("binary {name}").as_bytes()),
            size: format!("binary {name}").len() as u64,
        }],
        unknown: Vec::new(),
        is_empty: false,
    };
    let recipe_toml = format!(
        r#"
[pkg]
name = "{name}"
version = "{version}"
build = 1
arch = ["x86_64"]
desc = "test pkg"
license = "MIT"
homepage = "https://example.org"

[source]
url = "https://example.org/{name}-{version}.tar.gz"
sha256 = "deadbeef"

[deps]
run = [{deps}]

[build]
system = "make-only"
"#,
        deps = deps_run.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(", ")
    );
    let recipe = Recipe::parse(&recipe_toml).unwrap();
    let deps = DepSnapshot { run: recipe.deps.run.clone(), build: recipe.deps.build.clone() };
    let build = BuildRecord::default();
    commit_package(database, &recipe, &manifest, &deps, &build, "hash", reason).unwrap();
    // Имитируем кэш рецепта, который пишет установка.
    cache_recipe(db_dir, name, version, &recipe_toml).unwrap();
}

#[test]
fn remove_free_package() {
    let db = temp_dir("rm_free_db");
    let destroot = temp_dir("rm_free_root");
    let mut database = Database::open(&db).unwrap();
    install_pkg(&mut database, &db, &destroot, "hello", "1.0", &[], REASON_EXPLICIT);
    let bin = destroot.join("usr/local/bin/hello");
    assert!(bin.is_file());
    assert!(db.join("recipes/hello/1.0/pkg.toml").is_file());

    let out = remove_package(&mut database, &db, &destroot, "hello", false).unwrap();
    assert_eq!(out.files_removed, 1);
    assert!(out.missing.is_empty());
    assert!(out.forced_rdeps.is_empty());
    assert!(out.recipe_cache_removed);
    assert!(!bin.exists());
    assert!(database.get_package("hello").unwrap().is_none());
    assert!(database.get_files("hello").unwrap().is_empty());
    assert!(!db.join("recipes/hello").exists());
    drop(database);

    // Повторное удаление → NotInstalled, код 3.
    let mut database = Database::open(&db).unwrap();
    let err = remove_package(&mut database, &db, &destroot, "hello", false).unwrap_err();
    assert!(matches!(err, RemoveError::NotInstalled { .. }));
    assert_eq!(err.exit_code(), REMOVE_NOT_FOUND_EXIT_CODE);
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn remove_blocked_by_rdeps() {
    let db = temp_dir("rm_block_db");
    let destroot = temp_dir("rm_block_root");
    let mut database = Database::open(&db).unwrap();
    install_pkg(&mut database, &db, &destroot, "libgreet", "1.0", &[], REASON_EXPLICIT);
    install_pkg(&mut database, &db, &destroot, "hello", "1.0", &["libgreet"], REASON_EXPLICIT);

    let err = remove_package(&mut database, &db, &destroot, "libgreet", false).unwrap_err();
    match err {
        RemoveError::Blocked { ref name, ref rdeps } => {
            assert_eq!(name, "libgreet");
            assert_eq!(rdeps.as_slice(), ["hello".to_string()]);
        }
        other => panic!("ожидали Blocked, получили {other}"),
    }
    assert_eq!(err.exit_code(), REMOVE_BLOCKED_EXIT_CODE);
    // Ничего не удалено.
    assert!(destroot.join("usr/local/bin/libgreet").is_file());
    assert!(database.get_package("libgreet").unwrap().is_some());
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn remove_force_ignores_rdeps() {
    let db = temp_dir("rm_force_db");
    let destroot = temp_dir("rm_force_root");
    let mut database = Database::open(&db).unwrap();
    install_pkg(&mut database, &db, &destroot, "libgreet", "1.0", &[], REASON_EXPLICIT);
    install_pkg(&mut database, &db, &destroot, "hello", "1.0", &["libgreet"], REASON_EXPLICIT);

    let out = remove_package(&mut database, &db, &destroot, "libgreet", true).unwrap();
    assert_eq!(out.forced_rdeps, vec!["hello".to_string()]);
    assert!(!destroot.join("usr/local/bin/libgreet").exists());
    assert!(database.get_package("libgreet").unwrap().is_none());
    // Зависимый пакет остаётся установленным.
    assert!(database.get_package("hello").unwrap().is_some());
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn autoremove_removes_orphans_to_fixpoint() {
    let db = temp_dir("rm_auto_db");
    let destroot = temp_dir("rm_auto_root");
    let mut database = Database::open(&db).unwrap();
    // Цепочка app(explicit) → mid(dep) → leaf(dep).
    install_pkg(&mut database, &db, &destroot, "leaf", "1.0", &[], REASON_DEPENDENCY);
    install_pkg(&mut database, &db, &destroot, "mid", "1.0", &["leaf"], REASON_DEPENDENCY);
    install_pkg(&mut database, &db, &destroot, "app", "1.0", &["mid"], REASON_EXPLICIT);

    // Пока app установлен — сирот нет.
    assert!(autoremove_candidates(&database).unwrap().is_empty());

    // Удаляем app — mid становится сиротой, затем leaf.
    remove_package(&mut database, &db, &destroot, "app", false).unwrap();
    let candidates = autoremove_candidates(&database).unwrap();
    assert_eq!(candidates, vec!["mid".to_string()]);

    let removed = autoremove(&mut database, &db, &destroot).unwrap();
    let names: Vec<&str> = removed.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["mid", "leaf"]);
    assert!(database.list_packages().unwrap().is_empty());
    assert!(!destroot.join("usr/local/bin/mid").exists());
    assert!(!destroot.join("usr/local/bin/leaf").exists());
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}

#[test]
fn autoremove_keeps_explicit_and_needed() {
    let db = temp_dir("rm_keep_db");
    let destroot = temp_dir("rm_keep_root");
    let mut database = Database::open(&db).unwrap();
    // libdep нужен app; explicitlib — явно установленный, никем не нужен,
    // но explicit → autoremove не трогает.
    install_pkg(&mut database, &db, &destroot, "libdep", "1.0", &[], REASON_DEPENDENCY);
    install_pkg(&mut database, &db, &destroot, "explicitlib", "1.0", &[], REASON_EXPLICIT);
    install_pkg(&mut database, &db, &destroot, "app", "1.0", &["libdep"], REASON_EXPLICIT);

    let removed = autoremove(&mut database, &db, &destroot).unwrap();
    assert!(removed.is_empty());
    assert!(database.get_package("libdep").unwrap().is_some());
    assert!(database.get_package("explicitlib").unwrap().is_some());
    drop(database);
    cleanup_dirs(&[&db, &destroot]);
}
