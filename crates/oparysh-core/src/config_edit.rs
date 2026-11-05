//! Текстовое редактирование секции `[repositories]` в `oparysh.conf`
//! (этап 4.5 ROADMAP, DOCS §6/§10).
//!
//! Файл конфига переписывается **минимально**: все секции, кроме
//! `[repositories]` (включая комментарии и порядок), сохраняются как есть.
//! Сама секция `[repositories]` рендерится заново из записей конфига.

use crate::config::{OparyshConfig, RepoEntry};

/// Ошибка редактирования конфига.
#[derive(Debug, thiserror::Error)]
pub enum ConfigEditError {
    /// Конфиг не разобрался.
    #[error("не удалось разобрать oparysh.conf: {0}")]
    Parse(#[from] toml::de::Error),
    /// Репозиторий с таким именем уже есть в конфиге.
    #[error("репозиторий «{0}» уже есть в конфиге")]
    Duplicate(String),
    /// Репозитория с таким именем нет в конфиге.
    #[error("репозиторий «{0}» не найден в конфиге")]
    NotFound(String),
    /// Некорректное имя репозитория (годится как имя каталога кэша).
    #[error("некорректное имя репозитория «{0}»")]
    BadName(String),
}

/// Проверка имени репозитория: не начинается с `.`, без `/`, `\`, `:` и `..`.
pub fn is_valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name
            .split(['/', '\\', ':'])
            .any(|part| part == ".." || part.is_empty())
}

/// Добавить запись репозитория в конфиг. Возвращает новый текст конфига.
pub fn add_repository(config_toml: &str, entry: RepoEntry) -> Result<String, ConfigEditError> {
    if !is_valid_repo_name(&entry.name) {
        return Err(ConfigEditError::BadName(entry.name));
    }
    let config = OparyshConfig::parse(config_toml)?;
    let mut entries = config.repositories.entries.clone();
    if entries.iter().any(|e| e.name == entry.name) {
        return Err(ConfigEditError::Duplicate(entry.name));
    }
    entries.push(entry);
    Ok(splice_repositories(config_toml, &entries))
}

/// Убрать запись репозитория по имени. Возвращает новый текст конфига.
pub fn remove_repository(config_toml: &str, name: &str) -> Result<String, ConfigEditError> {
    let config = OparyshConfig::parse(config_toml)?;
    let mut entries = config.repositories.entries.clone();
    let before = entries.len();
    entries.retain(|e| e.name != name);
    if entries.len() == before {
        return Err(ConfigEditError::NotFound(name.to_string()));
    }
    Ok(splice_repositories(config_toml, &entries))
}

/// Экранировать строку для TOML basic-string.
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Отрендерить секцию `[repositories]` (с пустой строкой-разделителем при необходимости).
fn render_repositories(entries: &[RepoEntry]) -> String {
    let mut out = String::from("[repositories]\n");
    for e in entries {
        out.push_str("\n[[repositories.entries]]\n");
        out.push_str(&format!("name = \"{}\"\n", toml_escape(&e.name)));
        if let Some(path) = &e.path {
            out.push_str(&format!("path = \"{}\"\n", toml_escape(path)));
        }
        if let Some(url) = &e.url {
            out.push_str(&format!("url = \"{}\"\n", toml_escape(url)));
        }
        if let Some(key) = &e.key {
            out.push_str(&format!("key = \"{}\"\n", toml_escape(key)));
        }
    }
    out
}

/// Является ли строка заголовком TOML-таблицы (`[...]` / `[[...]]`).
fn is_table_header(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('[') && t.ends_with(']')
}

/// Является ли строка заголовком именно секции `[repositories]`.
fn is_repositories_header(line: &str) -> bool {
    line.trim() == "[repositories]"
}

/// Заменить (или добавить) секцию `[repositories]` в тексте конфига.
fn splice_repositories(config_toml: &str, entries: &[RepoEntry]) -> String {
    let rendered = render_repositories(entries);
    let lines: Vec<&str> = config_toml.lines().collect();

    // Найти начало секции [repositories] и конец (следующий заголовок или EOF).
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        if is_repositories_header(line) {
            start = Some(i);
            break;
        }
    }

    let mut out = String::new();
    match start {
        Some(s) => {
            let mut end = lines.len();
            for (i, line) in lines.iter().enumerate().skip(s + 1) {
                let t = line.trim();
                // Под-таблицы самой секции (`[[repositories.entries]]`) ей принадлежат.
                if is_table_header(line) && t != "[[repositories.entries]]" {
                    end = i;
                    break;
                }
            }
            for line in &lines[..s] {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&rendered);
            let tail = &lines[end..];
            if !tail.is_empty() {
                out.push('\n');
                for line in tail {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        None => {
            out.push_str(config_toml);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&rendered);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONF: &str = r#"# комментарий
[general]
jobs = 4

[repositories]

[[repositories.entries]]
name = "local"
path = "/srv/repo"

[toolchain]
cc = "gcc"
"#;

    fn entry(name: &str, path: Option<&str>, url: Option<&str>) -> RepoEntry {
        RepoEntry {
            name: name.to_string(),
            path: path.map(str::to_string),
            url: url.map(str::to_string),
            key: None,
        }
    }

    #[test]
    fn add_appends_and_keeps_rest() {
        let out = add_repository(CONF, entry("stable", None, Some("https://repo.example.com"))).unwrap();
        let parsed = OparyshConfig::parse(&out).unwrap();
        assert_eq!(parsed.repositories.entries.len(), 2);
        assert_eq!(parsed.repositories.entries[0].name, "local");
        assert_eq!(parsed.repositories.entries[1].url.as_deref(), Some("https://repo.example.com"));
        // прочие секции и комментарий сохранились
        assert!(out.contains("# комментарий"));
        assert!(out.contains("jobs = 4"));
        assert!(out.contains("cc = \"gcc\""));
    }

    #[test]
    fn add_duplicate_fails() {
        let err = add_repository(CONF, entry("local", Some("/x"), None)).unwrap_err();
        assert!(matches!(err, ConfigEditError::Duplicate(_)));
    }

    #[test]
    fn add_empty_config() {
        let out = add_repository("", entry("local", Some("/srv/repo"), None)).unwrap();
        let parsed = OparyshConfig::parse(&out).unwrap();
        assert_eq!(parsed.repositories.entries.len(), 1);
    }

    #[test]
    fn add_bad_name() {
        let err = add_repository(CONF, entry("../evil", Some("/x"), None)).unwrap_err();
        assert!(matches!(err, ConfigEditError::BadName(_)));
    }

    #[test]
    fn remove_existing() {
        let out = remove_repository(CONF, "local").unwrap();
        let parsed = OparyshConfig::parse(&out).unwrap();
        assert!(parsed.repositories.entries.is_empty());
        assert!(out.contains("jobs = 4"));
        assert!(out.contains("cc = \"gcc\""));
    }

    #[test]
    fn remove_missing_fails() {
        let err = remove_repository(CONF, "nope").unwrap_err();
        assert!(matches!(err, ConfigEditError::NotFound(_)));
    }

    #[test]
    fn toml_escape_roundtrip() {
        let weird = "quote\"back\\slash\n";
        let out = add_repository("", entry("r", Some(weird), None)).unwrap();
        let parsed = OparyshConfig::parse(&out).unwrap();
        assert_eq!(parsed.repositories.entries[0].path.as_deref(), Some(weird));
    }
}
