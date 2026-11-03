//! Парсер версий и требований к версиям (этап 1.3 ROADMAP).
//!
//! Поддерживаемые операторы: `=`, `!=`, `>`, `<`, `>=`, `<=`, `^`, `~`.
//! semver-совместимость (как в npm / Cargo):
//!   `^1.2.3`  → `>=1.2.3 <2.0.0`   (для 0.x.x: `^0.2.3` → `>=0.2.3 <0.3.0`)
//!   `~1.2.3`  → `>=1.2.3 <1.3.0`
//!
//! Пример требования: `"gcc>=11"`, `"pcre2>=10.32"`, `"zlib"`, `"clang^15"`.

use thiserror::Error;

/// Ошибки парсинга версии или требования.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VersionError {
    #[error("неизвестный оператор версии: {0:?}")]
    InvalidOp(String),
    #[error("неверный формат версии: {0}")]
    InvalidVersion(String),
    #[error("неверный формат требования: {0}")]
    InvalidReq(String),
}

/// Версия в формате semver (без учёта пре-релизов и сборок — они хранятся,
/// но не влияют на базовое сравнение major.minor.patch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Пре-релиз (например `alpha.1`, `rc.2`).
    pub pre: Option<String>,
    /// Метаданные сборки (например `build.123`).
    pub build: Option<String>,
}

impl Version {
    /// Разобрать строку версии (`1.2.3`, `1.2`, `1`, `1.2.3-alpha.1+build.4`).
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(VersionError::InvalidVersion(s.to_string()));
        }

        // Отделим метаданные сборки (+build) и пре-релиз (-pre).
        let (main, pre, build) = split_pre_build(s);

        let parts: Vec<&str> = main.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(VersionError::InvalidVersion(s.to_string()));
        }

        let mut iter = parts.into_iter();
        let major = parse_u64(iter.next().unwrap(), s)?;
        let minor = iter.next().map_or(Ok(0), |p| parse_u64(p, s))?;
        let patch = iter.next().map_or(Ok(0), |p| parse_u64(p, s))?;

        Ok(Version {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    /// Проверить, удовлетворяет ли эта версия требованию `req`.
    pub fn satisfies(&self, req: &VersionReq) -> bool {
        match req.op {
            Op::Eq => self == &req.version,
            Op::Ne => self != &req.version,
            Op::Gt => self > &req.version,
            Op::Lt => self < &req.version,
            Op::Ge => self >= &req.version,
            Op::Le => self <= &req.version,
            Op::Caret => satisfies_caret(self, &req.version),
            Op::Tilde => satisfies_tilde(self, &req.version),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Сначала по числовым компонентам.
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
        // При равных major.minor.patch версия без пре-релиза считается старше.
        match (self.pre.is_none(), other.pre.is_none()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{}", pre)?;
        }
        if let Some(build) = &self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

/// Оператор сравнения версии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Eq,    // =
    Ne,    // !=
    Gt,    // >
    Lt,    // <
    Ge,    // >=
    Le,    // <=
    Caret, // ^
    Tilde, // ~
}

impl Op {
    pub fn from_str(s: &str) -> Result<Self, VersionError> {
        match s {
            "=" => Ok(Op::Eq),
            "!=" => Ok(Op::Ne),
            ">" => Ok(Op::Gt),
            "<" => Ok(Op::Lt),
            ">=" => Ok(Op::Ge),
            "<=" => Ok(Op::Le),
            "^" => Ok(Op::Caret),
            "~" => Ok(Op::Tilde),
            _ => Err(VersionError::InvalidOp(s.to_string())),
        }
    }
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Eq => write!(f, "="),
            Op::Ne => write!(f, "!="),
            Op::Gt => write!(f, ">"),
            Op::Lt => write!(f, "<"),
            Op::Ge => write!(f, ">="),
            Op::Le => write!(f, "<="),
            Op::Caret => write!(f, "^"),
            Op::Tilde => write!(f, "~"),
        }
    }
}

/// Требование к версии: оператор + версия.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq {
    pub op: Op,
    pub version: Version,
}

impl std::fmt::Display for VersionReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.op, self.version)
    }
}

/// Полное требование зависимости: имя пакета + опциональное требование к версии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepReq {
    pub name: String,
    pub req: Option<VersionReq>,
}

impl DepReq {
    /// Разобрать строку вида `"gcc>=11"`, `"pcre2>=10.32"`, `"zlib"`, `"clang^15"`.
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(VersionError::InvalidReq(s.to_string()));
        }

        let chars: Vec<char> = s.chars().collect();
        let mut idx = 0;
        while idx < chars.len() {
            let c = chars[idx];
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                idx += 1;
            } else {
                break;
            }
        }

        let name = &s[..idx];
        if name.is_empty() {
            return Err(VersionError::InvalidReq(s.to_string()));
        }

        let rest = s[idx..].trim_start();
        if rest.is_empty() {
            return Ok(DepReq {
                name: name.to_string(),
                req: None,
            });
        }

        // Операторы могут быть двухсимвольными: >=, <=, !=
        let (op_str, ver_str) = if rest.starts_with(">=")
            || rest.starts_with("<=")
            || rest.starts_with("!=")
        {
            (&rest[..2], rest[2..].trim())
        } else if rest.starts_with('=')
            || rest.starts_with('>')
            || rest.starts_with('<')
            || rest.starts_with('^')
            || rest.starts_with('~')
        {
            (&rest[..1], rest[1..].trim())
        } else {
            return Err(VersionError::InvalidReq(s.to_string()));
        };

        let op = Op::from_str(op_str)?;
        let version = Version::parse(ver_str)?;

        Ok(DepReq {
            name: name.to_string(),
            req: Some(VersionReq { op, version }),
        })
    }
}

impl std::fmt::Display for DepReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.req {
            Some(req) => write!(f, "{}{}", self.name, req),
            None => write!(f, "{}", self.name),
        }
    }
}

// -------------------- внутренниеhelpers --------------------

fn parse_u64(s: &str, original: &str) -> Result<u64, VersionError> {
    s.parse::<u64>().map_err(|_| VersionError::InvalidVersion(original.to_string()))
}

/// Разделить строку на основную часть, пре-релиз и метаданные сборки.
/// `"1.2.3-alpha.1+build.4"` → `("1.2.3", Some("alpha.1"), Some("build.4"))`
fn split_pre_build(s: &str) -> (&str, Option<String>, Option<String>) {
    let mut main = s;
    let mut pre = None;
    let mut build = None;

    if let Some(pos) = s.find('+') {
        build = Some(s[pos + 1..].to_string());
        main = &s[..pos];
    }
    if let Some(pos) = main.find('-') {
        pre = Some(main[pos + 1..].to_string());
        main = &main[..pos];
    }

    (main, pre, build)
}

/// Верхняя граница для оператора `^` (caret).
/// `^1.2.3` → `<2.0.0`; `^0.2.3` → `<0.3.0`.
fn upper_bound_caret(req: &Version) -> Version {
    if req.major == 0 {
        Version {
            major: 0,
            minor: req.minor + 1,
            patch: 0,
            pre: None,
            build: None,
        }
    } else {
        Version {
            major: req.major + 1,
            minor: 0,
            patch: 0,
            pre: None,
            build: None,
        }
    }
}

/// Верхняя граница для оператора `~` (tilde).
/// `~1.2.3` → `<1.3.0`.
fn upper_bound_tilde(req: &Version) -> Version {
    Version {
        major: req.major,
        minor: req.minor + 1,
        patch: 0,
        pre: None,
        build: None,
    }
}

fn satisfies_caret(v: &Version, req: &Version) -> bool {
    if v < req {
        return false;
    }
    v < &upper_bound_caret(req)
}

fn satisfies_tilde(v: &Version, req: &Version) -> bool {
    if v < req {
        return false;
    }
    v < &upper_bound_tilde(req)
}
