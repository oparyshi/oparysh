//! Интеграционный тест 7.9 (ROADMAP): сборка фиктивного `hello` из
//! `tests/fixtures/hello-1.0` end-to-end в tmp-каталоге.
//!
//! Полная цепочка как в `oparysh build`, но без CLI: fetch → extract →
//! план сборки → выполнение с логом → артефакт в tmp-DESTDIR.
//! Успешная сборка + падение шага → код возврата 7, `build/` сохранён.

use hex::encode;
use oparysh_builder::*;
use oparysh_core::config::OparyshConfig;
use oparysh_core::recipe::Recipe;
use oparysh_fetcher::{extract_tarball, fetch_source, FetchRequest};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const CONF: &str = r#"
[general]
jobs = 4

[toolchain]
cc = "cc"
cxx = "g++"

[build.active]
profile = "release"

[profile.release]
cflags = "-O2"
cxxflags = ""
ldflags = ""
makeopts = ""

[build.extra]
cflags = ""
cxxflags = ""
ldflags = ""
makeopts = ""
"#;

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_e2e_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Фикстура `tests/fixtures/hello-1.0` с настоящим тарболом (`file://`) и
/// реальным sha256. Возвращает разобранный рецепт; при `swap_system = Some(…)`
/// `[build].system` заменяется на `custom` со steps из `steps_override`.
fn fixture_recipe(
    tmp: &Path,
    tag: &str,
    replacement: Option<(&str, &str)>,
) -> (Recipe, PathBuf) {
    let _ = tag;
    let dir = tmp.join("recipe");
    fs::create_dir_all(&dir).unwrap();

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hello-1.0");
    for file in ["hello.c", "Makefile"] {
        fs::copy(fixture.join(file), dir.join(file)).unwrap();
    }
    let tarball = tmp.join("hello-1.0.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("czf")
        .arg(&tarball)
        .arg("hello.c")
        .arg("Makefile")
        .current_dir(&dir)
        .status()
        .unwrap();
    assert!(status.success(), "tar не собрал фикстурный тарбол");

    let mut hasher = Sha256::new();
    hasher.update(fs::read(&tarball).unwrap());
    let sha = encode(hasher.finalize());

    let raw = fs::read_to_string(fixture.join("pkg.toml")).unwrap();
    let mut raw = raw
        .replace(
            "file:///fixtures/hello-1.0/hello-1.0.tar.gz",
            &format!("file://{}", tarball.display()),
        )
        .replace(
            "0000000000000000000000000000000000000000000000000000000000000000",
            &sha,
        );
    if let Some((system, steps)) = replacement {
        raw = raw
            .replace("system   = \"make-only\"", &format!("system   = \"{system}\""))
            .replace("[build.install]", &format!("{steps}\n\n[build.install]"));
    }
    (Recipe::parse(&raw).unwrap(), fixture)
}

#[test]
fn hello_fixture_builds_end_to_end_in_tmp() {
    let tmp = tmp_dir("hello");
    let (recipe, _fixture) = fixture_recipe(&tmp, "ok", None);
    assert_eq!(recipe.build.system, "make-only");

    let config = OparyshConfig::parse(CONF).unwrap();
    let db = tmp.join("db");

    // 1) fetch: кэш-мисс, тарбол попадает в <db>/src/.
    let source = recipe.source.clone();
    let req = FetchRequest {
        name: &recipe.pkg.name,
        version: &recipe.pkg.version,
        source: &source,
        repo_dist: None,
    };
    let fetched = fetch_source(&db, &req).unwrap();
    assert!(!fetched.from_cache);
    assert_eq!(
        fetched.path,
        oparysh_fetcher::src_cache_dir(&db).join("hello-1.0.tar.gz")
    );

    // Повторный fetch обязан взять артефакт из кэша.
    let again = fetch_source(&db, &req).unwrap();
    assert!(again.from_cache);

    // 2) extract: <db>/build/hello-1.0/, Makefile на месте.
    let extracted = extract_tarball(&db, &fetched.path, "hello", "1.0").unwrap();
    assert!(extracted.source_root.join("Makefile").is_file());
    assert!(extracted.source_root.join("hello.c").is_file());

    // 3) план сборки make-only, 4) выполнение с логом.
    let destdir = tmp.join("tmp/hello-1.0-build");
    fs::create_dir_all(&destdir).unwrap();
    let plan = build_plan(
        &config,
        &recipe,
        &extracted.source_root.display().to_string(),
        &destdir.display().to_string(),
    )
    .unwrap();
    assert_eq!(plan.system, "make-only");
    let log_path = today_log_path(&db, "hello", "1.0");
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    let mut log = fs::File::create(&log_path).unwrap();
    run_build_plan(&plan, &extracted.source_root, &mut log).unwrap();
    drop(log);

    // Артефакт в tmp-DESTDIR: <destdir>/usr/local/bin/hello — работает.
    let built = destdir.join("usr/local/bin/hello");
    assert!(built.is_file(), "DESTDIR-артефакт: {built:?}");
    let run = std::process::Command::new(&built).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "Hello, oparysh!");

    // Лог: обе фазы записаны (имя — logs/<pkg>-<ver>-<date>.log, DOCS §8).
    let name = log_path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("hello-1.0-") && name.ends_with(".log"),
        "{name}"
    );
    let text = fs::read_to_string(&log_path).unwrap();
    assert!(text.contains("== [compile] make -j4 =="), "{text}");
    assert!(text.contains("== [install] make install DESTDIR="), "{text}");
    assert!(text.contains("все шаги завершены успешно"), "{text}");

    // Установка не выполнялась вне tmp-DESTDIR (7.8).
    assert!(!db.join("usr").exists());
}

#[test]
fn hello_fixture_failing_step_maps_to_exit_code_seven() {
    let tmp = tmp_dir("fail");
    let (recipe, _fixture) = fixture_recipe(
        &tmp,
        "fail",
        Some((
            "custom",
            "[build.steps]\ncompile = [\"sh\", \"-c\", \"echo hello-compile >&2; exit 1\"]\ninstall = [\"sh\", \"-c\", \"echo must-not-run\"]",
        )),
    );

    let config = OparyshConfig::parse(CONF).unwrap();
    let db = tmp.join("db");
    let destdir = tmp.join("tmp/hello-1.0-build");
    let build_root = tmp.join("build");
    let build_dir = build_root.join("hello-1.0");
    fs::create_dir_all(&build_dir).unwrap();
    // Файл в build/ — после падения каталог обязан сохраниться (7.6).
    fs::write(build_dir.join("partial.txt"), b"partial").unwrap();

    let plan = build_plan(
        &config,
        &recipe,
        &build_dir.display().to_string(),
        &destdir.display().to_string(),
    )
    .unwrap();

    let log_path = today_log_path(&db, "hello", "1.0");
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    let mut log = fs::File::create(&log_path).unwrap();

    let err = run_build_plan(&plan, &build_root.join("hello-1.0"), &mut log).unwrap_err();
    drop(log);

    // Код возврата 7 (DOCS §11: ошибка сборки).
    assert!(matches!(err, BuildRunError::Failed { exit_code: 1, .. }), "{err:?}");
    assert_eq!(BUILD_FAILED_EXIT_CODE, 7);

    // install-шаг не выполнялся, DESTDIR остался пустым, build/ сохранён.
    let text = fs::read_to_string(log_path).unwrap();
    assert!(text.contains("hello-compile"), "{text}");
    assert!(text.contains("FAILED, exit 1"), "{text}");
    assert!(!text.contains("must-not-run"), "{text}");
    assert!(!text.contains("все шаги завершены успешно"), "{text}");
    assert!(!destdir.join("usr/local/bin/hello").exists(), "{text}");
    assert!(build_dir.join("partial.txt").exists(), "build/ сохранён");
}
