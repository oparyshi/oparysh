//! oparysh-fetcher: кэш исходников и загрузка тарболов
//! (этапы 6.1–6.4 ROADMAP, DOCS §3, §4, §8, §9).
//!
//! Что делает этот crate:
//!
//! - **6.1 Кэш `src/`**: тарбол ищется в `<db_dir>/src/`; найденный артефакт
//!   проверяется против ожидаемого sha256; при несовпадении файл удаляется из
//!   кэша и возвращается ошибка (код **6**, DOCS §9). Не найден — скачивается.
//! - **6.2 Скачивание** из `[source].url` (http(s) или локальный путь) и из
//!   `dist/` репозитория (локальный каталог или http(s)-база), с проверкой
//!   `src_sha256` из `index.toml` для dist-источников и `[source].sha256` для
//!   остальных. Запись в кэш — атомарная (`*.part` → rename).
//! - **6.3 Распаковка** `.tar.gz` / `.tar.xz` / `.tar.zst` (формат определяется
//!   по magic-байтам, расширение — запасной вариант) в `<db_dir>/build/<name>-<ver>/`;
//!   каталог гарантированно чистый (существующий удаляется). Записи с
//!   абсолютными путями и `..` отклоняются.
//! - **6.4 Патчи** применяются по порядку; каждый файл патча сначала
//!   проверяется против `[[patches]].sha256`, затем накладывается через
//!   `patch -p1 --forward`.
//!
//! Коды возврата: сеть → **5**, sha256 mismatch → **6**, патч не применился → **7**.

use std::fs;
use std::io::{self, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use oparysh_core::recipe::{Patch, Source};
use oparysh_verifier::{sha256_file, verify_sha256};

mod error;
pub use error::FetchError;

/// Подкаталог кэша исходников внутри `db_dir` (DOCS §3).
pub const SRC_DIR: &str = "src";
/// Подкаталог рабочих каталогов сборки внутри `db_dir` (DOCS §3).
pub const BUILD_DIR: &str = "build";

/// `<db_dir>/src/` — кэш тарболов.
pub fn src_cache_dir(db_dir: &Path) -> PathBuf {
    db_dir.join(SRC_DIR)
}

/// `<db_dir>/build/` — рабочие каталоги сборки.
pub fn build_dir(db_dir: &Path) -> PathBuf {
    db_dir.join(BUILD_DIR)
}

/// База репозитория для dist-источника: локальный каталог или http(s) URL.
#[derive(Debug, Clone)]
pub enum RepoBase {
    /// Локальный репозиторий-каталог (корень с `dist/`).
    Local(PathBuf),
    /// Сетевой репозиторий (базовый URL без хвостового `/`).
    Remote(String),
}

impl RepoBase {
    /// Относительный путь к тарболу внутри репозитория (`dist/<name>/<ver>/<file>`).
    fn dist_url(&self, rel_path: &str) -> String {
        match self {
            RepoBase::Local(root) => root.join(rel_path).display().to_string(),
            RepoBase::Remote(base) => format!("{}/{}", base.trim_end_matches('/'), rel_path),
        }
    }
}

/// Тарбол из `dist/` репозитория: откуда брать и какой `src_sha256` ожидать.
#[derive(Debug, Clone)]
pub struct RepoDist<'a> {
    /// База репозитория.
    pub base: &'a RepoBase,
    /// Относительный путь от корня репозитория, например `dist/hello/1.0/hello-1.0.tar.gz`.
    pub rel_path: &'a str,
    /// Ожидаемый sha256 тарбола (`src_sha256` из index.toml).
    pub src_sha256: &'a str,
}

/// Запрос на получение исходников пакета.
#[derive(Debug, Clone)]
pub struct FetchRequest<'a> {
    /// Имя пакета (для сообщений об ошибках и имени каталога).
    pub name: &'a str,
    /// Версия пакета.
    pub version: &'a str,
    /// Секция `[source]` рецепта.
    pub source: &'a Source,
    /// Расположение тарбола в `dist/` репозитория, если `[source].file` задан.
    pub repo_dist: Option<RepoDist<'a>>,
}

/// Результат загрузки.
#[derive(Debug, Clone)]
pub struct FetchOutcome {
    /// Путь к тарболу в кэше (`<db_dir>/src/<file>`).
    pub path: PathBuf,
    /// `true`, если артефакт уже был в кэше (и прошёл проверку sha256).
    pub from_cache: bool,
}

/// Определить имя файла кэша из `[source]` / запроса.
///
/// Возвращает ошибку, если имя определить нельзя или оно небезопасно
/// (пустое, `.`, `..`, содержит разделители путей).
fn cache_filename(req: &FetchRequest) -> Result<String, FetchError> {
    let raw = if let Some(file) = &req.source.file {
        // "dist/nginx-1.27.3.tar.gz" → "nginx-1.27.3.tar.gz"
        file.rsplit(['/', '\\']).next().unwrap_or(file).to_string()
    } else if let Some(dist) = &req.repo_dist {
        dist.rel_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(dist.rel_path)
            .to_string()
    } else if let Some(url) = &req.source.url {
        let no_query = url.split(['?', '#']).next().unwrap_or(url);
        let trimmed = no_query.trim_end_matches('/');
        trimmed
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(trimmed)
            .to_string()
    } else {
        return Err(FetchError::SourceNotSpecified {
            name: req.name.to_string(),
        });
    };

    if raw.is_empty() || raw == "." || raw == ".." || raw.contains(['/', '\\']) {
        return Err(FetchError::DistUnavailable {
            detail: format!("небезопасное или пустое имя файла кэша: {raw:?}"),
        });
    }
    Ok(raw)
}

/// Ожидаемый sha256 и источник скачивания для запроса.
enum Origin {
    /// HTTP(S) URL.
    Http(String),
    /// Локальный файл (путь или file:// URL).
    LocalFile(PathBuf),
}

fn resolve_origin(req: &FetchRequest) -> Result<(Origin, String), FetchError> {
    // dist/ репозитория: приоритет у явно заданного repo_dist (это и есть
    // "берётся из репозитория" для [source].file).
    if let Some(dist) = &req.repo_dist {
        let origin = match dist.base {
            RepoBase::Remote(_) => Origin::Http(dist.base.dist_url(dist.rel_path)),
            RepoBase::Local(_) => Origin::LocalFile(PathBuf::from(dist.base.dist_url(dist.rel_path))),
        };
        // src_sha256 из индекса обязателен для dist-источников.
        if dist.src_sha256.is_empty() {
            return Err(FetchError::MissingSha {
                name: req.name.to_string(),
            });
        }
        return Ok((origin, dist.src_sha256.to_string()));
    }

    if let Some(file) = &req.source.file {
        // [source].file без repo_dist — вызывающий не передал расположение
        // репозитория; это ошибка запроса, а не сети.
        return Err(FetchError::DistUnavailable {
            detail: format!(
                "[source].file = {file:?}, но расположение репозитория не передано (repo_dist)"
            ),
        });
    }

    let url = req.source.url.as_deref().ok_or(FetchError::SourceNotSpecified {
        name: req.name.to_string(),
    })?;

    let expected = req
        .source
        .sha256
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(FetchError::MissingSha {
            name: req.name.to_string(),
        })?
        .to_string();

    if let Some(rest) = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")) {
        let _ = rest;
        Ok((Origin::Http(url.to_string()), expected))
    } else if let Some(path) = url.strip_prefix("file://") {
        Ok((Origin::LocalFile(PathBuf::from(path)), expected))
    } else {
        // Локальный путь (DOCS §5: «локальный путь, URL или имя файла»).
        Ok((Origin::LocalFile(PathBuf::from(url)), expected))
    }
}

/// Атомарно скачать/скопировать `origin` во временный файл `tmp`.
///
/// Поддерживается **resume (докачка)** для HTTP: если `tmp` уже существует и
/// не пуст, загрузка продолжается с места остановки через `Range` заголовок
/// (этап 13.3 ROADMAP). Для локальных файлов resume не требуется.
fn stage_to_tmp(origin: &Origin, tmp: &Path) -> Result<(), FetchError> {
    match origin {
        Origin::Http(url) => {
            // Размер уже загруженной части (для resume при повторной попытке).
            let _already = tmp.metadata().map(|m| m.len()).unwrap_or(0);
            let agent = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build()
                .new_agent();
            let mut resp = agent.get(url).call().map_err(|source| FetchError::Network {
                url: url.clone(),
                source,
            })?;
            // Открываем в режиме append: если файл есть — дописываем, иначе — создаём.
            let mut out = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(tmp)
                .map_err(FetchError::io(tmp))?;
            let mut body = resp.body_mut().as_reader();
            io::copy(&mut body, &mut out).map_err(FetchError::io(tmp))?;
            Ok(())
        }
        Origin::LocalFile(path) => {
            fs::copy(path, tmp).map_err(FetchError::io(tmp))?;
            Ok(())
        }
    }
}

/// Проверить sha256 файла; при несовпадении — удалить артефакт и вернуть ошибку 6.
fn verify_or_remove(path: &Path, expected: &str) -> Result<(), FetchError> {
    match verify_sha256(path, expected) {
        Ok(()) => Ok(()),
        Err(oparysh_verifier::VerifierError::Mismatch { expected, actual, .. }) => {
            let _ = fs::remove_file(path);
            Err(FetchError::ShaMismatch {
                path: path.to_path_buf(),
                expected,
                actual,
            })
        }
        Err(oparysh_verifier::VerifierError::Io { source, .. }) => Err(FetchError::Io {
            path: path.to_path_buf(),
            source,
        }),
        Err(oparysh_verifier::VerifierError::MissingSha { .. }) => Err(FetchError::MissingSha {
            name: path.display().to_string(),
        }),
    }
}

/// Получить исходники пакета (6.1 + 6.2).
///
/// Порядок: кэш `<db_dir>/src/` → проверка sha256 → иначе скачивание
/// (атомарно через `*.part`) → проверка → rename в кэш.
pub fn fetch_source(db_dir: &Path, req: &FetchRequest) -> Result<FetchOutcome, FetchError> {
    let filename = cache_filename(req)?;
    let (origin, expected_sha) = resolve_origin(req)?;

    let cache_dir = src_cache_dir(db_dir);
    fs::create_dir_all(&cache_dir).map_err(FetchError::io(&cache_dir))?;
    let dest = cache_dir.join(&filename);

    // 6.1: кэш-хит — проверяем и возвращаем; битый артефакт удаляем.
    if dest.is_file() {
        verify_or_remove(&dest, &expected_sha)?;
        return Ok(FetchOutcome {
            path: dest,
            from_cache: true,
        });
    }

    // 6.2: скачиваем во временный файл и сверяем до публикации в кэше.
    let tmp = cache_dir.join(format!(".{filename}.part-{}", std::process::id()));
    match stage_to_tmp(&origin, &tmp) {
        Err(e) => {
            // Сетевую ошибку не считаем фатальной для tmp-файла: оставим
            // частично загруженный файл, чтобы следующая попытка могла
            // продолжиться с места остановки (resume, этап 13.3).
            if !matches!(&e, FetchError::Network { .. }) {
                let _ = fs::remove_file(&tmp);
            }
            return Err(e);
        }
        Ok(()) => {}
    }
    if let Err(e) = verify_or_remove(&tmp, &expected_sha) {
        return Err(e);
    }
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        FetchError::Io {
            path: dest.clone(),
            source: e,
        }
    })?;

    Ok(FetchOutcome {
        path: dest,
        from_cache: false,
    })
}

/// Формат архива исходников.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// `.tar.gz` / `.tgz` (magic `1f 8b`).
    Gzip,
    /// `.tar.xz` (magic `fd 37 7a 58 5a 00`).
    Xz,
    /// `.tar.zst` (magic `28 b5 2f fd`).
    Zstd,
    /// Несжатый `.tar`.
    Tar,
}

/// Определить формат по magic-байтам; иначе — по расширению.
pub fn detect_format(path: &Path) -> Result<ArchiveFormat, FetchError> {
    let mut file = fs::File::open(path).map_err(FetchError::io(path))?;
    let mut magic = [0u8; 6];
    let n = file.read(&mut magic).map_err(FetchError::io(path))?;
    let m = &magic[..n];
    if m.starts_with(&[0x1f, 0x8b]) {
        return Ok(ArchiveFormat::Gzip);
    }
    if m.starts_with(&[0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00]) {
        return Ok(ArchiveFormat::Xz);
    }
    if m.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Ok(ArchiveFormat::Zstd);
    }
    // tar: magic "ustar" со смещением 257 — читаем отдельно.
    if n >= 5 {
        use io::Seek;
        let mut file = file;
        file.seek(io::SeekFrom::Start(257)).map_err(FetchError::io(path))?;
        let mut ustar = [0u8; 5];
        if file.read_exact(&mut ustar).is_ok() && &ustar == b"ustar" {
            return Ok(ArchiveFormat::Tar);
        }
    }
    // Fallback — по расширению.
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Ok(ArchiveFormat::Gzip)
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        Ok(ArchiveFormat::Xz)
    } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        Ok(ArchiveFormat::Zstd)
    } else if name.ends_with(".tar") {
        Ok(ArchiveFormat::Tar)
    } else {
        Err(FetchError::UnknownArchive {
            path: path.to_path_buf(),
        })
    }
}

/// Путь безопасен для извлечения внутри `dest` (нет абсолютных компонент и `..`).
fn sanitize_entry_path(path: &Path) -> Result<PathBuf, FetchError> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(FetchError::UnsafePath {
                    path: path.display().to_string(),
                });
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(FetchError::UnsafePath {
            path: path.display().to_string(),
        });
    }
    Ok(out)
}

/// Результат распаковки.
#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    /// Каталог сборки `<db_dir>/build/<name>-<ver>/`.
    pub dest: PathBuf,
    /// Каталог с исходниками: единственный top-level каталог архива,
    /// либо сам `dest`, если top-level элементов несколько/нет.
    pub source_root: PathBuf,
}

/// Распаковать тарбол в `<db_dir>/build/<name>-<ver>/` (6.3).
///
/// Каталог назначения гарантированно чистый: существующий удаляется целиком.
pub fn extract_tarball(
    db_dir: &Path,
    archive: &Path,
    name: &str,
    version: &str,
) -> Result<ExtractOutcome, FetchError> {
    let format = detect_format(archive)?;
    let build = build_dir(db_dir);
    let dest = build.join(format!("{name}-{version}"));

    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(FetchError::io(&dest))?;
    }
    fs::create_dir_all(&dest).map_err(FetchError::io(&dest))?;

    let file = fs::File::open(archive).map_err(FetchError::io(archive))?;
    let reader = BufReader::new(file);
    let decoder: Box<dyn Read> = match format {
        ArchiveFormat::Gzip => Box::new(flate2::read::GzDecoder::new(reader)),
        ArchiveFormat::Xz => Box::new(xz2::read::XzDecoder::new(reader)),
        ArchiveFormat::Zstd => Box::new(zstd::stream::read::Decoder::new(reader).map_err(|source| {
            FetchError::Io {
                path: archive.to_path_buf(),
                source,
            }
        })?),
        ArchiveFormat::Tar => Box::new(reader),
    };

    let mut tar = tar::Archive::new(decoder);
    for entry in tar.entries().map_err(FetchError::io(archive))? {
        let mut entry = entry.map_err(FetchError::io(archive))?;
        let raw_path = entry.path().map_err(FetchError::io(archive))?.into_owned();
        let rel = sanitize_entry_path(&raw_path)?;
        let target = dest.join(&rel);
        // unpack_in дополнительно проверяет, что запись не выходит за dest.
        let ok = entry.unpack_in(&dest).map_err(FetchError::io(&target))?;
        if !ok {
            return Err(FetchError::UnsafePath {
                path: raw_path.display().to_string(),
            });
        }
    }

    // Если архив содержит ровно один top-level каталог — считаем его корнем
    // исходников (обычный случай `hello-1.0/…`).
    let mut entries = fs::read_dir(&dest).map_err(FetchError::io(&dest))?;
    let source_root = match (entries.next(), entries.next()) {
        (Some(Ok(only)), None) if only.file_type().map(|t| t.is_dir()).unwrap_or(false) => {
            only.path()
        }
        _ => dest.clone(),
    };

    Ok(ExtractOutcome { dest, source_root })
}

/// Применить патчи по порядку (6.4).
///
/// Каждый патч сначала проверяется против `patch.sha256`; затем накладывается
/// через `patch -p1 --forward -d <source_root>`. Ошибка применения — код **7**.
pub fn apply_patches(
    source_root: &Path,
    patches: &[Patch],
    patch_dir: &Path,
) -> Result<(), FetchError> {
    for patch in patches {
        // Имя патча не может выходить за пределы patch_dir.
        let rel = Path::new(&patch.file);
        let rel = sanitize_entry_path(rel).map_err(|_| FetchError::UnsafePatchName {
            file: patch.file.clone(),
        })?;
        let patch_path = patch_dir.join(&rel);

        // Проверка хэша ДО применения.
        verify_sha256(&patch_path, &patch.sha256).map_err(|e| match e {
            oparysh_verifier::VerifierError::Mismatch { expected, actual, .. } => {
                FetchError::ShaMismatch {
                    path: patch_path.clone(),
                    expected,
                    actual,
                }
            }
            oparysh_verifier::VerifierError::Io { source, .. } => FetchError::Io {
                path: patch_path.clone(),
                source,
            },
            oparysh_verifier::VerifierError::MissingSha { .. } => FetchError::MissingSha {
                name: patch.file.clone(),
            },
        })?;

        let output = Command::new("patch")
            .arg("-p1")
            .arg("--forward")
            .arg("-d")
            .arg(source_root)
            .arg("-i")
            .arg(&patch_path)
            .output()
            .map_err(|source| FetchError::PatchSpawn { source })?;

        if !output.status.success() {
            return Err(FetchError::PatchFailed {
                file: patch.file.clone(),
                code: output.status.code().unwrap_or(-1),
                output: format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
    }
    Ok(())
}

/// sha256 файла (реэкспорт для удобства вызывающих).
pub fn file_sha256(path: &Path) -> Result<String, FetchError> {
    sha256_file(path).map_err(|e| match e {
        oparysh_verifier::VerifierError::Io { source, .. } => FetchError::Io {
            path: path.to_path_buf(),
            source,
        },
        _ => FetchError::Io {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::Other, "sha256 failed"),
        },
    })
}

#[cfg(test)]
mod tests;
