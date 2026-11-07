//! Выбор языка интерфейса по окружению (LC_ALL → LC_MESSAGES → LANG).
//!
//! `ru*` — русский, иначе английский. Прочитанное значение кэшируется
//! в `OnceLock`, чтобы вывод команды не переключал язык посреди вывода.

/// Язык интерфейса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    /// Определить язык из окружения: `LC_ALL` → `LC_MESSAGES` → `LANG`,
    /// префикс `ru` (без учёта регистра) → [`Lang::Ru`], иначе [`Lang::En`].
    pub fn current() -> Lang {
        static LANG_RU: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *LANG_RU.get_or_init(|| {
            for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
                if let Ok(l) = std::env::var(var) {
                    let l = l.trim().to_lowercase();
                    if l.is_empty() || l == "c" || l == "posix" {
                        continue;
                    }
                    if l.starts_with("ru") {
                        return true;
                    }
                }
            }
            false
        }) {
            Lang::Ru
        } else {
            Lang::En
        }
    }

    /// Русский ли язык (для прямых ветвлений строк).
    pub fn is_ru(self) -> bool {
        self == Lang::Ru
    }
}

/// Строка интерфейса: [`Lang::Ru`] — русский вариант, иначе английский.
pub fn t(ru: impl AsRef<str>, en: impl AsRef<str>) -> String {
    t_lang(Lang::current(), ru, en)
}

/// Строка интерфейса для явно указанного языка (например, в рендере плана).
pub fn t_lang(lang: Lang, ru: impl AsRef<str>, en: impl AsRef<str>) -> String {
    if lang.is_ru() {
        ru.as_ref().to_owned()
    } else {
        en.as_ref().to_owned()
    }
}
