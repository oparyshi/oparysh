//! Прогресс-индикаторы для загрузки и сборки (этап 14.1 ROADMAP).
//!
//! Простые текстовые индикаторы с возвратом каретки (`\r`), работающие
//! только в терминале (tty). В не-tty режиме (конвейер, лог) выводятся
//! обычные однострочные сообщения без `\r`.

use std::io::{self, IsTerminal, Write};
use std::time::Instant;

/// Режим вывода: только если stdout — терминал.
pub fn is_tty() -> bool {
    io::stdout().is_terminal()
}

/// Индикатор прогресса загрузки (байт / всего).
#[derive(Debug)]
pub struct DownloadProgress {
    name: String,
    total: Option<u64>,
    downloaded: u64,
    started: Instant,
    finished: bool,
}

impl DownloadProgress {
    pub fn new(name: &str, total: Option<u64>) -> Self {
        DownloadProgress {
            name: name.to_string(),
            total,
            downloaded: 0,
            started: Instant::now(),
            finished: false,
        }
    }

    pub fn update(&mut self, downloaded: u64) {
        self.downloaded = downloaded;
        self.print();
    }

    pub fn finish(&mut self) {
        self.finished = true;
        self.print();
        println!(); // новая строка после завершения
    }

    fn print(&self) {
        if !is_tty() {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        let rate = if elapsed > 0.0 {
            self.downloaded as f64 / elapsed
        } else {
            0.0
        };
        let bar = match self.total {
            Some(total) => {
                let pct = if total > 0 {
                    (self.downloaded as f64 / total as f64 * 100.0) as u64
                } else {
                    0
                };
                let width = 20u64;
                let filled = width * pct / 100;
                let mut b = String::from("[");
                for i in 0..width {
                    if i < filled {
                        b.push('=');
                    } else if i == filled && pct < 100 {
                        b.push('>');
                    } else {
                        b.push(' ');
                    }
                }
                b.push(']');
                format!(
                    "\r{} {} {:>3}% {:>10}/{:<10} {:>6.1} KB/s",
                    self.name, b, pct, self.downloaded, total, rate / 1024.0
                )
            }
            None => format!(
                "\r{} {:>10} байт {:>6.1} KB/s",
                self.name, self.downloaded, rate / 1024.0
            ),
        };
        let _ = io::stdout().write_all(bar.as_bytes());
        let _ = io::stdout().flush();
    }
}

/// Простой спиннер для долгих операций без известного итога.
#[derive(Debug)]
pub struct Spinner {
    message: String,
    phase: u64,
    started: Instant,
    finished: bool,
}

const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl Spinner {
    pub fn new(message: &str) -> Self {
        Spinner {
            message: message.to_string(),
            phase: 0,
            started: Instant::now(),
            finished: false,
        }
    }

    pub fn tick(&mut self) {
        self.phase = self.phase.wrapping_add(1);
        self.print();
    }

    pub fn finish(&mut self, done_message: Option<&str>) {
        self.finished = true;
        if let Some(msg) = done_message {
            if is_tty() {
                println!("\r{}", msg);
            } else {
                println!("{}", msg);
            }
        } else {
            println!();
        }
    }

    fn print(&self) {
        if !is_tty() {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        let ch = SPINNER_CHARS[self.phase as usize % SPINNER_CHARS.len()];
        let line = format!("\r{} {} ({:.1}с)", ch, self.message, elapsed);
        let _ = io::stdout().write_all(line.as_bytes());
        let _ = io::stdout().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_progress_create() {
        let p = DownloadProgress::new("test.tar.gz", Some(1000));
        assert_eq!(p.name, "test.tar.gz");
        assert_eq!(p.total, Some(1000));
        assert_eq!(p.downloaded, 0);
    }

    #[test]
    fn download_progress_no_total() {
        let p = DownloadProgress::new("test.tar.gz", None);
        assert_eq!(p.total, None);
    }

    #[test]
    fn spinner_create() {
        let s = Spinner::new("загрузка...");
        assert_eq!(s.message, "загрузка...");
        assert_eq!(s.phase, 0);
    }
}
