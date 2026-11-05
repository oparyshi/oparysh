//! Тесты fetcher'а: кэш-хит, кэш-мисс, битый хэш, dist-репозиторий,
//! распаковка архивов и наложение патчей (шаги 6.1–6.4).

use super::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> Self {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("oparysh_fetch_{tag}_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha256_str(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

fn make_tar(files: &[(&str, &[u8])], format: ArchiveFormat, dest: &Path) {
    let file = fs::File::create(dest).unwrap();
    let writer: Box<dyn Write> = match format {
        ArchiveFormat::Gzip => Box::new(flate2::write::GzEncoder::new(file, flate2::Compression::default())),
        ArchiveFormat::Xz => Box::new(xz2::write::XzEncoder::new(file, 6)),
        ArchiveFormat::Zstd => Box::new(zstd::stream::write::Encoder::new(file, 3).unwrap().auto_finish()),
        ArchiveFormat::Tar => Box::new(file),
    };
    let mut builder = tar::Builder::new(writer);
    for (name, content) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *content).unwrap();
    }
    builder.into_inner().unwrap();
}

fn source_with(url: Option<&str>, file: Option<&str>, sha256: Option<&str>) -> Source {
    Source {
        url: url.map(str::to_string),
        file: file.map(str::to_string),
        sha256: sha256.map(str::to_string),
    }
}

// ---------- 6.1 / 6.2: кэш и скачивание ----------

#[test]
fn cache_miss_then_hit() {
    let tmp = TestDir::new("cache_hit");
    let src_file = tmp.path().join("hello-1.0.tar.gz");
    fs::write(&src_file, b"fake-tarball").unwrap();
    let sha = sha256_str(b"fake-tarball");

    let db = tmp.path().join("db");
    let source = source_with(Some(src_file.to_str().unwrap()), None, Some(&sha));
    let req = FetchRequest {
        name: "hello",
        version: "1.0",
        source: &source,
        repo_dist: None,
    };

    let first = fetch_source(&db, &req).unwrap();
    assert!(!first.from_cache);
    assert!(first.path.is_file());
    assert_eq!(first.path, src_cache_dir(&db).join("hello-1.0.tar.gz"));

    // Исходный файл удаляем — второй вызов обязан взять из кэша.
    fs::remove_file(&src_file).unwrap();
    let second = fetch_source(&db, &req).unwrap();
    assert!(second.from_cache);
    assert_eq!(fs::read(&second.path).unwrap(), b"fake-tarball");
}

#[test]
fn corrupt_cache_is_deleted() {
    let tmp = TestDir::new("corrupt");
    let db = tmp.path().join("db");
    let cache = src_cache_dir(&db);
    fs::create_dir_all(&cache).unwrap();
    let cached = cache.join("hello-1.0.tar.gz");
    fs::write(&cached, b"corrupted").unwrap();
    let good_sha = sha256_str(b"good");

    let source = source_with(Some("http://127.0.0.1:1/hello-1.0.tar.gz"), None, Some(&good_sha));
    let req = FetchRequest {
        name: "hello",
        version: "1.0",
        source: &source,
        repo_dist: None,
    };
    let err = fetch_source(&db, &req).unwrap_err();
    assert!(matches!(err, FetchError::ShaMismatch { .. }));
    assert_eq!(err.exit_code(), 6);
    assert!(!cached.exists(), "битый артефакт должен быть удалён из кэша");
}

#[test]
fn bad_hash_on_download_not_cached() {
    let tmp = TestDir::new("badhash");
    let src_file = tmp.path().join("pkg-2.0.tar.gz");
    fs::write(&src_file, b"real-content").unwrap();
    let wrong_sha = sha256_str(b"other-content");

    let db = tmp.path().join("db");
    let source = source_with(Some(src_file.to_str().unwrap()), None, Some(&wrong_sha));
    let req = FetchRequest {
        name: "pkg",
        version: "2.0",
        source: &source,
        repo_dist: None,
    };
    let err = fetch_source(&db, &req).unwrap_err();
    assert!(matches!(err, FetchError::ShaMismatch { .. }));
    assert!(!src_cache_dir(&db).join("pkg-2.0.tar.gz").exists());
    // Временные .part-файлы тоже не должны остаться.
    let leftovers: Vec<_> = fs::read_dir(src_cache_dir(&db))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".part-"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn missing_sha_is_fatal() {
    let tmp = TestDir::new("missingsha");
    let src_file = tmp.path().join("x.tar.gz");
    fs::write(&src_file, b"data").unwrap();
    let db = tmp.path().join("db");
    let source = source_with(Some(src_file.to_str().unwrap()), None, None);
    let req = FetchRequest {
        name: "x",
        version: "1",
        source: &source,
        repo_dist: None,
    };
    let err = fetch_source(&db, &req).unwrap_err();
    assert!(matches!(err, FetchError::MissingSha { .. }));
}

#[test]
fn repo_dist_local_ok_and_mismatch() {
    let tmp = TestDir::new("dist");
    // Локальный репозиторий с dist/
    let repo = tmp.path().join("repo");
    let dist = repo.join("dist/hello/1.0");
    fs::create_dir_all(&dist).unwrap();
    let tarball = dist.join("hello-1.0.tar.gz");
    fs::write(&tarball, b"dist-content").unwrap();
    let good_sha = sha256_str(b"dist-content");

    let db = tmp.path().join("db");
    let source = source_with(None, Some("dist/hello/1.0/hello-1.0.tar.gz"), None);
    let base = RepoBase::Local(repo.clone());
    let d = RepoDist {
        base: &base,
        rel_path: "dist/hello/1.0/hello-1.0.tar.gz",
        src_sha256: &good_sha,
    };
    let req = FetchRequest {
        name: "hello",
        version: "1.0",
        source: &source,
        repo_dist: Some(d),
    };
    let out = fetch_source(&db, &req).unwrap();
    assert_eq!(fs::read(&out.path).unwrap(), b"dist-content");

    // Теперь с неверным src_sha256 — отдельный db, чтобы кэш не мешал.
    let db2 = tmp.path().join("db2");
    let bad_sha = sha256_str(b"nope");
    let d = RepoDist {
        base: &base,
        rel_path: "dist/hello/1.0/hello-1.0.tar.gz",
        src_sha256: &bad_sha,
    };
    let req = FetchRequest {
        name: "hello",
        version: "1.0",
        source: &source,
        repo_dist: Some(d),
    };
    let err = fetch_source(&db2, &req).unwrap_err();
    assert!(matches!(err, FetchError::ShaMismatch { .. }));
    assert_eq!(err.exit_code(), 6);
}

#[test]
fn file_url_source() {
    let tmp = TestDir::new("fileurl");
    let src_file = tmp.path().join("f.tar.gz");
    fs::write(&src_file, b"via-file-url").unwrap();
    let sha = sha256_str(b"via-file-url");
    let db = tmp.path().join("db");
    let url = format!("file://{}", src_file.display());
    let source = source_with(Some(&url), None, Some(&sha));
    let req = FetchRequest {
        name: "f",
        version: "1",
        source: &source,
        repo_dist: None,
    };
    let out = fetch_source(&db, &req).unwrap();
    assert_eq!(fs::read(&out.path).unwrap(), b"via-file-url");
}

// ---------- 6.3: распаковка ----------

#[test]
fn extract_all_formats() {
    let tmp = TestDir::new("extract");
    for (format, ext) in [
        (ArchiveFormat::Gzip, "tar.gz"),
        (ArchiveFormat::Xz, "tar.xz"),
        (ArchiveFormat::Zstd, "tar.zst"),
    ] {
        let archive = tmp.path().join(format!("hello-1.0.{ext}"));
        make_tar(
            &[("hello-1.0/hello.c", b"int main(void){return 0;}\n"),
              ("hello-1.0/Makefile", b"all:\n\ttrue\n")],
            format,
            &archive,
        );
        let db = tmp.path().join(format!("db_{ext}"));
        let out = extract_tarball(&db, &archive, "hello", "1.0").unwrap();
        assert_eq!(out.dest, build_dir(&db).join("hello-1.0"));
        assert_eq!(out.source_root, out.dest.join("hello-1.0"));
        assert!(out.source_root.join("hello.c").is_file());
        assert_eq!(detect_format(&archive).unwrap(), format);
    }
}

#[test]
fn extract_cleans_existing_dir() {
    let tmp = TestDir::new("clean");
    let db = tmp.path().join("db");
    let dest = build_dir(&db).join("hello-1.0");
    fs::create_dir_all(dest.join("stale")).unwrap();
    fs::write(dest.join("stale/old.txt"), b"old").unwrap();

    let archive = tmp.path().join("hello-1.0.tar.gz");
    make_tar(&[("hello-1.0/new.txt", b"new")], ArchiveFormat::Gzip, &archive);
    let out = extract_tarball(&db, &archive, "hello", "1.0").unwrap();
    assert!(!dest.join("stale").exists(), "каталог сборки должен быть чистым");
    assert!(out.source_root.join("new.txt").is_file());
}

/// Собрать сырой .tar.gz с записью, чей name содержит `..`
/// (tar::Builder такие пути отклоняет, поэтому формируем вручную).
fn make_evil_tar_gz(path: &Path, entry_name: &str, content: &[u8]) {
    let mut header = [0u8; 512];
    let name_bytes = entry_name.as_bytes();
    header[..name_bytes.len()].copy_from_slice(name_bytes);
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{:011o}\0", content.len()).as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    for b in &mut header[148..156] {
        *b = b' '; // chksum считается по заголовку с пробелами на месте суммы
    }
    header[156] = b'0'; // typeflag: regular file
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|&b| b as u32).sum();
    header[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());

    let file = fs::File::create(path).unwrap();
    let mut writer = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    writer.write_all(&header).unwrap();
    writer.write_all(content).unwrap();
    let padding = (512 - content.len() % 512) % 512;
    writer.write_all(&vec![0u8; padding]).unwrap();
    writer.write_all(&[0u8; 1024]).unwrap(); // конец архива
    writer.finish().unwrap();
}

#[test]
fn extract_rejects_traversal() {
    let tmp = TestDir::new("traversal");
    let archive = tmp.path().join("evil.tar.gz");
    make_evil_tar_gz(&archive, "../../escape.txt", b"pwned");

    let db = tmp.path().join("db");
    let err = extract_tarball(&db, &archive, "evil", "1.0").unwrap_err();
    assert!(matches!(err, FetchError::UnsafePath { .. }));
}

// ---------- 6.4: патчи ----------

#[test]
fn patches_applied_in_order() {
    let tmp = TestDir::new("patch");
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("hello.c"), "int main(void) {\n    return 1;\n}\n").unwrap();

    let p1 = "--- a/hello.c\n+++ b/hello.c\n@@ -1,3 +1,3 @@\n int main(void) {\n-    return 1;\n+    return 2;\n }\n";
    let p2 = "--- a/hello.c\n+++ b/hello.c\n@@ -1,3 +1,3 @@\n int main(void) {\n-    return 2;\n+    return 0;\n }\n";
    let patch_dir = tmp.path().join("patches");
    fs::create_dir_all(&patch_dir).unwrap();
    fs::write(patch_dir.join("01-a.patch"), p1).unwrap();
    fs::write(patch_dir.join("02-b.patch"), p2).unwrap();

    let patches = vec![
        Patch { file: "01-a.patch".into(), sha256: sha256_str(p1.as_bytes()) },
        Patch { file: "02-b.patch".into(), sha256: sha256_str(p2.as_bytes()) },
    ];
    apply_patches(&src, &patches, &patch_dir).unwrap();
    let content = fs::read_to_string(src.join("hello.c")).unwrap();
    assert!(content.contains("return 0;"));
}

#[test]
fn patch_bad_sha_rejected_before_apply() {
    let tmp = TestDir::new("patchsha");
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("f.txt"), b"original\n").unwrap();

    let p = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-original\n+changed\n";
    let patch_dir = tmp.path().join("patches");
    fs::create_dir_all(&patch_dir).unwrap();
    fs::write(patch_dir.join("p.patch"), p).unwrap();

    let patches = vec![Patch { file: "p.patch".into(), sha256: sha256_str(b"different") }];
    let err = apply_patches(&src, &patches, &patch_dir).unwrap_err();
    assert!(matches!(err, FetchError::ShaMismatch { .. }));
    assert_eq!(err.exit_code(), 6);
    // Патч не должен быть применён.
    assert_eq!(fs::read(src.join("f.txt")).unwrap(), b"original\n");
}

#[test]
fn patch_failure_is_error_7() {
    let tmp = TestDir::new("patchfail");
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("f.txt"), b"completely different\n").unwrap();

    let p = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-original\n+changed\n";
    let patch_dir = tmp.path().join("patches");
    fs::create_dir_all(&patch_dir).unwrap();
    fs::write(patch_dir.join("p.patch"), p).unwrap();

    let patches = vec![Patch { file: "p.patch".into(), sha256: sha256_str(p.as_bytes()) }];
    let err = apply_patches(&src, &patches, &patch_dir).unwrap_err();
    assert!(matches!(err, FetchError::PatchFailed { .. }));
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn patch_name_traversal_rejected() {
    let tmp = TestDir::new("patchtrav");
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    let patch_dir = tmp.path().join("patches");
    fs::create_dir_all(&patch_dir).unwrap();
    let patches = vec![Patch { file: "../escape.patch".into(), sha256: "00".into() }];
    let err = apply_patches(&src, &patches, &patch_dir).unwrap_err();
    assert!(matches!(err, FetchError::UnsafePatchName { .. }));
}
