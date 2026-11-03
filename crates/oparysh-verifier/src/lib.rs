//! oparysh-verifier: проверка SHA-256 контрольных сумм артефактов (этап 2 ROADMAP, раздел 9 DOCS).
//!
//! Проверяются:
//! - тарбол исходников против `[source].sha256`;
//! - каждый патч против `[[patches]].sha256`;
//! - `recipe_sha` / `src_sha256` из `index.toml`.
//!
//! Несовпадение — ошибка (код возврата **6**), артефакт не используется.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use thiserror::Error;

/// Ошибки верификации.
#[derive(Debug, Error)]
pub enum VerifierError {
    #[error("не удалось прочитать файл {path}: {source}")]
    Io {
        path: String,
        source: io::Error,
    },
    #[error("sha256 mismatch для {path}: ожидалось {expected}, получено {actual}")]
    Mismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("отсутствует обязательный sha256 для {path}")]
    MissingSha { path: String },
}

/// Вычислить SHA-256 файла и вернуть hex-строку в нижнем регистре.
pub fn sha256_file(path: &Path) -> Result<String, VerifierError> {
    let mut file = File::open(path).map_err(|e| VerifierError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|e| VerifierError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Проверить, что sha256 файла совпадает с ожидаемым (сравнение без учёта регистра).
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), VerifierError> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(VerifierError::Mismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

/// Проверить тарбол источника рецепта (`[source].sha256`).
/// Отсутствие или пустая строка sha256 — MissingSha (жирная ошибка по DOCS §9).
pub fn verify_tarball(sha256: Option<&str>, path: &Path) -> Result<(), VerifierError> {
    let expected = match sha256 {
        Some(s) if !s.is_empty() => s,
        _ => return Err(VerifierError::MissingSha {
            path: path.display().to_string(),
        }),
    };
    verify_sha256(path, expected)
}

/// Проверить список патчей против их sha256.
/// `patches` — пары `(имя_файла, ожидаемый_sha256)`, `base_dir` — каталог с патчами.
pub fn verify_patches(
    patches: &[(String, String)],
    base_dir: &Path,
) -> Result<(), VerifierError> {
    for (file, sha) in patches {
        let path = base_dir.join(file);
        verify_sha256(&path, sha)?;
    }
    Ok(())
}

/// Проверить `recipe_sha` — sha256 содержимого файла рецепта `pkg.toml`.
pub fn verify_recipe_sha(content: &[u8], expected: &str) -> Result<(), VerifierError> {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let actual = hex::encode(hasher.finalize());
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(VerifierError::Mismatch {
            path: "recipe".to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

/// Проверить `src_sha256` — sha256 тарбола из `dist/` репозитория (из `index.toml`).
pub fn verify_src_sha256(path: &Path, expected: &str) -> Result<(), VerifierError> {
    verify_sha256(path, expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(content: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let file = dir.join(format!("oparysh_verify_test_{}", std::process::id()));
        let mut f = File::create(&file).unwrap();
        f.write_all(content).unwrap();
        file
    }

    #[test]
    fn sha256_file_known_value() {
        // sha256("hello\n") = 5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03
        let path = write_temp_file(b"hello\n");
        let hash = sha256_file(&path).unwrap();
        assert_eq!(
            hash,
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sha256_file_empty() {
        // sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let path = write_temp_file(b"");
        let hash = sha256_file(&path).unwrap();
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_sha256_match() {
        let path = write_temp_file(b"hello\n");
        assert!(verify_sha256(
            &path,
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        )
        .is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_sha256_mismatch() {
        let path = write_temp_file(b"hello\n");
        let err = verify_sha256(&path, "0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, VerifierError::Mismatch { .. }));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_tarball_missing_sha() {
        let path = write_temp_file(b"hello\n");
        let err = verify_tarball(None, &path).unwrap_err();
        assert!(matches!(err, VerifierError::MissingSha { .. }));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_tarball_empty_sha() {
        let path = write_temp_file(b"hello\n");
        let err = verify_tarball(Some(""), &path).unwrap_err();
        assert!(matches!(err, VerifierError::MissingSha { .. }));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_recipe_sha_match() {
        let content = b"[pkg]\nname = \"hello\"\nversion = \"1.0\"\n";
        // sha256(content)
        let mut hasher = Sha256::new();
        hasher.update(content);
        let expected = hex::encode(hasher.finalize());
        assert!(verify_recipe_sha(content, &expected).is_ok());
    }

    #[test]
    fn verify_recipe_sha_mismatch() {
        let content = b"[pkg]\nname = \"hello\"\nversion = \"1.0\"\n";
        let err = verify_recipe_sha(content, "0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, VerifierError::Mismatch { .. }));
    }

    #[test]
    fn verify_patches_all_ok() {
        let dir = std::env::temp_dir().join(format!("oparysh_verify_patches_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p1 = dir.join("fix.patch");
        let mut f = File::create(&p1).unwrap();
        f.write_all(b"patch content\n").unwrap();
        let sha = sha256_file(&p1).unwrap();

        let patches = vec![("fix.patch".to_string(), sha)];
        assert!(verify_patches(&patches, &dir).is_ok());

        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn verify_patches_missing_file() {
        let dir = std::env::temp_dir().join(format!("oparysh_verify_patches_missing_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let patches = vec![("nonexistent.patch".to_string(), "abcd".to_string())];
        let err = verify_patches(&patches, &dir).unwrap_err();
        assert!(matches!(err, VerifierError::Io { .. }));
        let _ = std::fs::remove_dir(&dir);
    }
}
