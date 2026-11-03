//! Валидатор рецепта: юнит-тесты (шаг 1.2).

use oparysh_core::recipe::*;
use oparysh_core::validate::{validate, validate_recipe};

const MINIMAL: &str = r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "https://example.com/hello-1.0.tar.gz"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[build]
"#;

fn minimal() -> Recipe {
    Recipe::parse(MINIMAL).unwrap()
}

#[test]
fn minimal_recipe_is_valid() {
    let r = minimal();
    assert_eq!(validate_recipe(&r), Vec::<String>::new());
    assert!(validate(&r).is_ok());
}

#[test]
fn missing_sha256_is_fatal_error() {
    // Отсутствие sha256 у источника — жирная ошибка, не предупреждение.
    let toml = &MINIMAL.replace(
        "sha256 = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
        "",
    );
    let r = Recipe::parse(toml).unwrap();
    let errs = validate_recipe(&r);
    assert!(
        errs.iter().any(|e| e.contains("sha256") && e.contains("обязательное")),
        "unexpected: {errs:?}"
    );
    assert!(validate(&r).is_err());
}

#[test]
fn malformed_sha256_rejected() {
    let mut r = minimal();
    r.source.sha256 = Some("DEADBEEF".into());
    assert!(validate(&r).is_err());
    r.source.sha256 = Some(
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF".into(),
    );
    assert!(validate(&r).is_err(), "верхний регистр не допускается (hex, нижний регистр)");
}

#[test]
fn missing_pkg_name_and_version() {
    // Пустые обязательные поля: serde гарантирует присутствие при разборе,
    // но валидатор дополнительно отсекает пустые значения.
    let mut r = minimal();
    r.pkg.name = "".into();
    r.pkg.version = "  ".into();
    let errs = validate_recipe(&r);
    assert_eq!(errs.len(), 2, "{errs:?}");
}

#[test]
fn missing_source_selector() {
    let r = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[build]
"#,
    )
    .unwrap();
    assert!(validate(&r).is_err());
}

#[test]
fn url_and_file_exclusive() {
    let r = Recipe::parse(&MINIMAL.replace(
        "[build]",
        "file = \"dist/hello.tar.xz\"\n\n[build]",
    ))
    .unwrap();
    assert!(validate(&r).is_err());
}

#[test]
fn unknown_build_system_rejected() {
    let r = Recipe::parse(&MINIMAL.replace("[build]", "[build]\nsystem = \"worley\"")).unwrap();
    assert!(validate(&r).is_err());
}

#[test]
fn custom_system_requires_steps() {
    let bad = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "u"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[build]
system = "custom"
"#,
    )
    .unwrap();
    assert!(validate(&bad).is_err());

    let ok = Recipe::parse(
        &r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "u"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[build]
system = "custom"

[build.steps]
compile = ["make"]
install = ["make", "install"]
"#,
    )
    .unwrap();
    assert!(validate(&ok).is_ok());
}

#[test]
fn patch_sha256_required_format() {
    let bad = Recipe::parse(&MINIMAL.replace(
        "[build]",
        "[[patches]]\nfile = \"fix.patch\"\nsha256 = \"\"\n\n[build]",
    ))
    .unwrap();
    assert!(validate(&bad).is_err());

    let good = Recipe::parse(&MINIMAL.replace(
        "[build]",
        "[[patches]]\nfile = \"fix.patch\"\nsha256 = \"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\"\n\n[build]",
    ))
    .unwrap();
    assert!(validate(&good).is_ok());
}
