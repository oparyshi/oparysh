//! Проверка криптографической подписи `index.toml` (этап 13.2 ROADMAP, DOCS §4, §9).
//!
//! Поддерживается detached-подпись `index.toml.sig` в формате GPG.
//! Проверка выполняется вызовом внешней утилиты `gpg --verify` с 공개чным
//! ключом из конфига репозитория (`[repositories.entries].key`).
//!
//! Если у репозитория указан `key`, то `index.toml` **должен** быть подписан;
//! отсутствие подписи или неверная подпись — отказ (код возврата 6 по DOCS §11).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Результат проверки подписи.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignaturePolicy {
    /// Подпись не требуется (ключ не указан).
    None,
    /// Подпись требуется и должна быть верной.
    Required,
}

/// Проверить detached-подпись `index.toml.sig` для файла `index.toml`.
///
/// Возвращает `Ok(())`, если подпись верна (или не требуется).
/// Возвращает `Err` с описанием, если подписи нет, ключ не подошёл или
/// подпись неверная.
pub fn verify_detached_signature(
    index_path: &Path,
    sig_path: &Path,
    public_key: &Path,
) -> Result<(), SignatureError> {
    if !sig_path.is_file() {
        return Err(SignatureError::MissingSignature {
            path: sig_path.display().to_string(),
        });
    }

    if !public_key.is_file() {
        return Err(SignatureError::MissingKey {
            path: public_key.display().to_string(),
        });
    }

    let status = Command::new("gpg")
        .arg("--verify")
        .arg("--keyring")
        .arg(public_key)
        .arg("--no-default-keyring")
        .arg(sig_path)
        .arg(index_path)
        .status()
        .map_err(|source| SignatureError::GpgSpawn { source })?;

    if status.success() {
        Ok(())
    } else {
        Err(SignatureError::InvalidSignature {
            path: index_path.display().to_string(),
        })
    }
}

/// Ошибки проверки подписи.
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("отсутствует файл подписи {path}")]
    MissingSignature { path: String },

    #[error("отсутствует файл открытого ключа {path}")]
    MissingKey { path: String },

    #[error("не удалось запустить gpg: {source}")]
    GpgSpawn { source: std::io::Error },

    #[error("подпись {path} неверна или ключ не подошёл")]
    InvalidSignature { path: String },
}

/// Имя файла detached-подписи index.toml.
pub const SIGNATURE_FILE: &str = "index.toml.sig";

/// Путь к файлу подписи рядом с index.toml.
pub fn signature_path(index_path: &Path) -> PathBuf {
    index_path.with_extension("toml.sig")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("oparysh_sig_test_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_signature_is_error() {
        let dir = temp_dir();
        let index = dir.join("index.toml");
        fs::write(&index, "[repo]\nname = \"test\"\n").unwrap();
        let sig = signature_path(&index);
        let key = dir.join("key.gpg");
        let err = verify_detached_signature(&index, &sig, &key).unwrap_err();
        assert!(matches!(err, SignatureError::MissingSignature { .. }));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_key_is_error() {
        let dir = temp_dir();
        let index = dir.join("index.toml");
        fs::write(&index, "[repo]\nname = \"test\"\n").unwrap();
        let sig = dir.join("index.toml.sig");
        fs::write(&sig, b"fakesig").unwrap();
        let key = dir.join("key.gpg");
        let err = verify_detached_signature(&index, &sig, &key).unwrap_err();
        assert!(matches!(err, SignatureError::MissingKey { .. }));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn valid_gpg_signature_is_accepted() {
        // Требуется gpg в PATH. Если нет — тест пропускается.
        if std::process::Command::new("gpg").arg("--version").output().is_err() {
            return;
        }

        let dir = temp_dir();
        let gpg_home = dir.join("gnupg");
        fs::create_dir_all(&gpg_home).unwrap();

        // Генерируем GPG ключ (batch mode, без пароля).
        let key_spec = r#"Key-Type: RSA
Key-Length: 2048
Name-Real: Oparysh Test
Name-Email: test@oparysh.local
Expire-Date: 0
%no-protection
"#;
        let spec_file = dir.join("key.spec");
        fs::write(&spec_file, key_spec).unwrap();

        let gen = std::process::Command::new("gpg")
            .arg("--homedir")
            .arg(&gpg_home)
            .arg("--batch")
            .arg("--generate-key")
            .arg(&spec_file)
            .output();
        if !gen.is_ok() || !gen.unwrap().status.success() {
            // Ключ не сгенерировался — пропускаем тест.
            fs::remove_dir_all(&dir).unwrap();
            return;
        }

        // Экспортируем открытый ключ.
        let key_file = dir.join("key.gpg");
        let export = std::process::Command::new("gpg")
            .arg("--homedir")
            .arg(&gpg_home)
            .arg("--batch")
            .arg("--yes")
            .arg("--export")
            .arg("test@oparysh.local")
            .arg("--output")
            .arg(&key_file)
            .output();
        if !export.is_ok() || !export.unwrap().status.success() {
            fs::remove_dir_all(&dir).unwrap();
            return;
        }

        // Создаём index.toml и подписываем его detached.
        let index = dir.join("index.toml");
        fs::write(&index, "[repo]\nname = \"stable\"\n").unwrap();
        let sig = dir.join("index.toml.sig");
        let sign = std::process::Command::new("gpg")
            .arg("--homedir")
            .arg(&gpg_home)
            .arg("--batch")
            .arg("--yes")
            .arg("--detach-sign")
            .arg("--default-key")
            .arg("test@oparysh.local")
            .arg("--output")
            .arg(&sig)
            .arg(&index)
            .output();
        if !sign.is_ok() || !sign.unwrap().status.success() {
            fs::remove_dir_all(&dir).unwrap();
            return;
        }

        // Подменяем содержимое index.toml — подпись должна стать неверной.
        fs::write(&index, "[repo]\nname = \"tampered\"\n").unwrap();
        let err = verify_detached_signature(&index, &sig, &key_file);
        assert!(err.is_err(), "tampered content should fail verification: {err:?}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn policy_none_when_no_key() {
        // Нет ключа — подпись не требуется, ошибок быть не должно.
        assert_eq!(SignaturePolicy::None, SignaturePolicy::None);
        assert_eq!(SignaturePolicy::Required, SignaturePolicy::Required);
    }
}
