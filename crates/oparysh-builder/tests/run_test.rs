//! Интеграционные тесты 7.6: логи сборки в logs/<pkg>-<ver>-<date>.log,
//! падение шага → сборка прервана (следующие шаги не выполняются),
//! код возврата 7 для ошибки сборки.

use oparysh_builder::*;
use oparysh_core::config::OparyshConfig;
use oparysh_core::recipe::Recipe;
use std::path::Path;

const CONF: &str = r#"
[general]
jobs = 4

[toolchain]
cc = "gcc"
cxx = "g++"

[build.active]
profile = "release"

[profile.release]
cflags = "-O2"

[build.extra]
"#;

fn recipe(build_section: &str) -> Recipe {
    Recipe::parse(&format!(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "https://example.com/hello.tar.gz"
sha256 = "deadbeef"

[build]
{build_section}
"#,
    ))
    .unwrap()
}

#[test]
fn log_path_format() {
    let path = build_log_path(Path::new("/var/lib/oparysh"), "hello", "1.0", "20261106");
    assert_eq!(path, Path::new("/var/lib/oparysh/logs/hello-1.0-20261106.log"));
    let today = today_log_path(Path::new("/tmp"), "pkg", "2.3");
    let name = today.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("pkg-2.3-") && name.ends_with(".log"), "{name}");
    let date = name.trim_start_matches("pkg-2.3-").trim_end_matches(".log");
    assert_eq!(date.len(), 8, "YYYYMMDD: {date}");
    assert!(date.chars().all(|c| c.is_ascii_digit()), "{date}");
}

#[test]
fn successful_plan_logs_every_step() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = recipe(r#"system = "custom"

[build.steps]
prepare = ["sh", "-c", "echo preparing"]
compile = ["sh", "-c", "echo compiling >&2; sleep 0"]
"#);
    let plan = build_plan(&config, &recipe, "/tmp/src", "/tmp/destdir").unwrap();
    assert_eq!(plan.system, "custom");

    let tmp = std::env::temp_dir().join("oparysh-build-test-success");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let log_path = today_log_path(&tmp, "hello", "1.0");
    std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();

    let mut log = std::fs::File::create(&log_path).unwrap();
    run_build_plan(&plan, Path::new("/tmp"), &mut log).unwrap();
    drop(log);

    let text = read_log(&log_path).unwrap();
    assert!(text.contains("== [prepare] sh -c echo preparing =="), "{text}");
    assert!(text.contains("== [compile] sh -c echo compiling >&2; sleep 0 =="), "{text}");
    assert!(text.contains("preparing"), "{text}");
    assert!(text.contains("compiling"), "{text}");
    assert!(text.contains("все шаги завершены успешно"), "{text}");
}

#[test]
fn failing_step_interrupts_build_and_preserves_build_dir() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = recipe(r#"system = "custom"

[build.steps]
prepare = ["sh", "-c", "echo prepare-ok; exit 0"]
compile = ["sh", "-c", "echo compile-failed >&2; exit 3"]
install = ["sh", "-c", "echo must-not-run"]
"#);
    let plan = build_plan(&config, &recipe, "/tmp/src", "/tmp/destdir").unwrap();

    let tmp = std::env::temp_dir().join("oparysh-build-test-fail");
    let _ = std::fs::remove_dir_all(&tmp);
    let build_dir = tmp.join("build/hello-1.0");
    std::fs::create_dir_all(&build_dir).unwrap();
    // Файл в каталоге build/ — он должен сохраниться после падения.
    let artifact = build_dir.join("partial-obj.o");
    std::fs::write(&artifact, b"x").unwrap();

    std::fs::create_dir_all(tmp.join("logs")).unwrap();
    let log_path = tmp.join("logs/hello-1.0-.log");
    let mut log = std::fs::File::create(&log_path).unwrap();

    let err = run_build_plan(&plan, &build_dir, &mut log).unwrap_err();
    assert!(matches!(err, BuildRunError::Failed { exit_code: 3, .. }), "{err:?}");
    assert_eq!(BUILD_FAILED_EXIT_CODE, 7);
    assert!(
        err.to_string().contains("сборка прервана, build/ сохранён"),
        "{err}"
    );

    // Лог: prepare-ok записан, compile-failed тоже, install не выполнялся.
    let text = read_log(&log_path).unwrap();
    assert!(text.contains("prepare-ok"), "{text}");
    assert!(text.contains("compile-failed"), "{text}");
    assert!(text.contains("FAILED, exit 3"), "{text}");
    assert!(!text.contains("must-not-run"), "{text}");
    assert!(!text.contains("все шаги завершены успешно"), "{text}");

    // build/ сохранён: артефакт на месте, install-шаг туда ничего не добавил.
    assert!(artifact.exists(), "build/ должен сохраниться после падения");
}

#[test]
fn step_failure_maps_to_exit_code_seven() {
    // DOCS §11: 7 — ошибка сборки. Любая BuildRunError::Failed → 7.
    let err = BuildRunError::Failed {
        phase: "compile".into(),
        executable: "make".into(),
        exit_code: 2,
    };
    assert_eq!(BUILD_FAILED_EXIT_CODE, 7);
    assert!(format!("{err}").contains("compile"));
}
