//! Интеграционный тест команды `oparysh build` (этапы 7.7–7.8 ROADMAP):
//! сборка фикстурного `hello` без установки, перекрытия `--set`/`--jobs`
//! действуют только на эту сборку, коды возврата 2/3.

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

/// Конфиг с изолированными db/cache в tmp.
fn config_file(tmp: &Path) -> PathBuf {
    let path = tmp.join("oparysh.conf");
    fs::write(
        &path,
        format!(
            "[general]\njobs = 4\ndb_dir = {:?}\ncache_dir = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n",
            tmp.join("db").display(),
            tmp.join("cache").display(),
        ),
    )
    .unwrap();
    path
}

/// Запустить бинарь с изолированной конфигурацией.
fn run(tmp: &Path, args: &[&str]) -> (ExitStatus, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_oparysh"))
        .args(args)
        .env("OPARYSH_CONFIG", tmp.join("oparysh.conf"))
        .env_remove("OPARYSH_DB_DIR")
        .env_remove("OPARYSH_CACHE_DIR")
        .output()
        .unwrap();
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oparysh_build_cmd_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn build_recipe_dir_without_install_applies_overrides() {
    let tmp = tmp_dir("ok");
    let dir = recipe_dir(&tmp);
    let _ = config_file(&tmp);

    let (status, stdout, stderr) = run(
        &tmp,
        &[
            "build",
            dir.to_str().unwrap(),
            "--set",
            "cflags=-O0 -DOPARYSH_TEST",
            "--set",
            "cc=cc",
            "--jobs",
            "2",
        ],
    );
    assert!(
        status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("Сборка hello 1.0"), "{stdout}");
    assert!(stdout.contains("cflags = -O0 -DOPARYSH_TEST"), "{stdout}");
    assert!(stdout.contains("jobs = 2"), "{stdout}");
    // Установка не выполнялась: файлы только в tmp-DESTDIR.
    assert!(stdout.contains("установка не выполнялась"), "{stdout}");
    let built = tmp.join("db/tmp/hello-1.0-destdir/usr/local/bin/hello");
    assert!(built.is_file(), "DESTDIR-артефакт на месте: {stdout}");
    let running = Command::new(&built).output().unwrap();
    assert!(running.status.success());
    assert!(
        String::from_utf8_lossy(&running.stdout).contains("Hello"),
        "собранный бинарник работает"
    );

    // Лог сборки в logs/<pkg>-<ver>-<date>.log (7.6, формат DOCS §8).
    let logs: Vec<PathBuf> = fs::read_dir(tmp.join("db/logs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(logs.len(), 1, "ровно один лог: {logs:?}");
    let name = logs[0].file_name().unwrap().to_str().unwrap().to_string();
    assert!(name.starts_with("hello-1.0-") && name.ends_with(".log"), "{name}");

    // Перекрытия — только на эту сборку: повторная сборка без --set
    // собирается с флагами профиля (-O2), а не с перекрытием.
    let (status, stdout, stderr) = run(tmp.as_path(), &["build", dir.to_str().unwrap()]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    // Перекрытие — только на эту сборку: повторная сборка без --set
    // собирается с флагами профиля (-O2), а не с перекрытием. Лог —
    // общий файл (дописывается), поэтому проверяем количество вхождений.
    let log = fs::read_to_string(&logs[0]).unwrap();
    let mentions = log.matches("OPARYSH_TEST").count();
    assert_eq!(
        mentions, 1,
        "перекрытие не должно переживать вторую сборку: {log}"
    );
}

#[test]
fn build_install_installs_to_destroot_and_records_db() {
    let tmp = tmp_dir("install");
    let dir = recipe_dir(&tmp);
    let _ = config_file(&tmp);
    // destroot изолируем в tmp (никогда не пишите в реальный /).
    let root = tmp.join("root");
    fs::write(
        tmp.join("oparysh.conf"),
        format!(
            "[general]\njobs = 4\ndb_dir = {:?}\ncache_dir = {:?}\ndestroot = {:?}\n\n\
             [build.active]\nprofile = \"release\"\n\n\
             [profile.release]\ncflags = \"-O2\"\n",
            tmp.join("db").to_string_lossy(),
            tmp.join("cache").to_string_lossy(),
            root.to_string_lossy(),
        ),
    )
    .unwrap();

    let (status, stdout, stderr) = run(
        &tmp,
        &["build", dir.to_str().unwrap(), "--install"],
    );
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("Установлено: hello 1.0"), "{stdout}");
    // Файл в destroot.
    assert!(root.join("usr/local/bin/hello").is_file(), "{stdout}");
    // DESTDIR и build-каталог очищены (8.4).
    assert!(!tmp.join("db/tmp/hello-1.0-destdir").exists(), "{stdout}");
    assert!(!tmp.join("db/build/hello-1.0").exists(), "{stdout}");
    // Кэш рецепта записан.
    assert!(tmp.join("db/recipes/hello/1.0/pkg.toml").is_file());
    // БД: packages/files/deps/build_info заполнены (8.3).
    let db = oparysh_db::Database::open(&tmp.join("db")).unwrap();
    let pkg = db.get_package("hello").unwrap().unwrap();
    assert_eq!(pkg.name, "hello");
    assert_eq!(pkg.version, "1.0");
    assert!(pkg.size_bytes > 0);
    let files = db.get_files("hello").unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("usr/local/bin/hello"));
    assert!({
        let expected = oparysh_verifier::sha256_file(&root.join("usr/local/bin/hello")).unwrap();
        files[0].sha256 == expected
    });
    assert!(db.get_build_info("hello").unwrap().is_some());
}

#[test]
fn build_unknown_package_is_exit_three() {
    let tmp = tmp_dir("notfound");
    let _ = config_file(&tmp);
    let dir = recipe_dir(&tmp);
    // Рецептный каталог отсутствует → пакетная форма → поиск в (пустых)
    // репозиториях → код 3.
    let (_status, _stdout, stderr) = run(tmp.as_path(), &["build", "no-such-pkg"]);
    assert_eq!(_status.code(), Some(3), "stderr: {stderr}");
    assert!(stderr.contains("не найден"), "stderr: {stderr}");
    let _ = dir;
}

#[test]
fn build_bad_set_key_is_exit_two() {
    let tmp = tmp_dir("badset");
    let dir = recipe_dir(&tmp);
    let _ = config_file(&tmp);
    let (_status, _stdout, stderr) = run(
        tmp.as_path(),
        &["build", dir.to_str().unwrap(), "--set", "tuples=1"],
    );
    assert_eq!(_status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains("неизвестный флаг"), "stderr: {stderr}");
}
