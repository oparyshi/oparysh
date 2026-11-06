//! Цветной вывод (этап 11.2).
//!
//! Цвет отключается, если задана переменная `NO_COLOR` или `OPARYSH_NO_COLORS`
//! (presence, независимо от значения — как в no-color.org), либо если stdout
//! не является терминалом. Принудительно включить цвет нельзя — только
//! терминалу нужно оформление.

use std::io::IsTerminal;

/// Чистое решение: включён ли цвет.
pub fn decide(no_color: bool, oparysh_no_colors: bool, is_tty: bool) -> bool {
    is_tty && !no_color && !oparysh_no_colors
}

/// Включён ли цветной вывод в текущем окружении.
pub fn enabled() -> bool {
    decide(
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var_os("OPARYSH_NO_COLORS").is_some(),
        std::io::stdout().is_terminal(),
    )
}

/// Окрасить `text` кодом `code`, если цвет включён.
fn paint(code: &str, text: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Зелёный — успех/целостность.
pub fn green(text: &str) -> String {
    paint("32", text)
}

/// Красный — ошибка/повреждение.
pub fn red(text: &str) -> String {
    paint("31", text)
}

/// Жёлтый — предупреждение/«new».
pub fn yellow(text: &str) -> String {
    paint("33", text)
}

/// Жирный — заголовки.
pub fn bold(text: &str) -> String {
    paint("1", text)
}

/// Приглушённый — второстепенный текст.
pub fn dim(text: &str) -> String {
    paint("2", text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_only_on_tty_without_opt_out() {
        assert!(decide(false, false, true));
        assert!(!decide(false, false, false));
        assert!(!decide(true, false, true));
        assert!(!decide(false, true, true));
        assert!(!decide(true, true, true));
    }
}
