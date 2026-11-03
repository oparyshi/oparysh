//! Типы рецепта: юнит-тесты на TOML-разбор (шаги 1.1, 1.2, 1.6).

use oparysh_core::recipe::*;
use std::collections::BTreeMap;

/// Рецепт-пример из раздела 5 DOCS.
const EXAMPLE: &str = r#"
[pkg]
name     = "nginx"
version  = "1.27.3"
build    = 1
arch     = ["x86_64"]
desc     = "HTTP-сервер и reverse-proxy"
license  = "BSD-2-Clause"
homepage = "https://nginx.org"

[source]
url      = "https://nginx.org/download/nginx-1.27.3.tar.gz"
sha256   = "5f2bab"

[deps]
build    = ["gcc>=11", "make>=4.0", "pcre2>=10.32"]
run      = ["pcre2>=10.32", "zlib>=1.2"]
optional = { "openssl>=3.0" = "tls" }

[[patches]]
file   = "fix-musl.patch"
sha256 = "3c1d"

[provides]
virtual = ["httpd"]

[conflicts]
pkgs = ["apache<2.6"]

[build]
system   = "autoconf-make"
cc       = ""
cflags   = ""
ldflags  = ""
makeopts = ""
parallel = 0
env      = { LIBS = "-lm" }

[build.install]
prefix  = "/usr/local"

[files]
unknown_warn = true
"#;

#[test]
fn parse_full_example() {
    let r = Recipe::parse(EXAMPLE).unwrap();
    assert_eq!(r.pkg.name, "nginx");
    assert_eq!(r.pkg.version, "1.27.3");
    assert_eq!(r.pkg.build, 1);
    assert_eq!(r.pkg.arch, vec!["x86_64"]);
    assert_eq!(r.source.url.as_deref(), Some("https://nginx.org/download/nginx-1.27.3.tar.gz"));
    assert_eq!(r.source.sha256.as_deref(), Some("5f2bab"));
    assert_eq!(r.deps.build, vec!["gcc>=11", "make>=4.0", "pcre2>=10.32"]);
    assert_eq!(r.deps.run, vec!["pcre2>=10.32", "zlib>=1.2"]);
    assert_eq!(r.deps.optional.get("openssl>=3.0").map(String::as_str), Some("tls"));
    assert_eq!(r.patches.len(), 1);
    assert_eq!(r.patches[0].file, "fix-musl.patch");
    assert_eq!(r.patches[0].sha256, "3c1d");
    assert_eq!(r.provides.r#virtual, vec!["httpd"]);
    assert_eq!(r.conflicts.pkgs, vec!["apache<2.6"]);
    assert_eq!(r.build.system, "autoconf-make");
    assert_eq!(r.build.parallel, 0);
    assert_eq!(r.build.env.get("LIBS").map(String::as_str), Some("-lm"));
    assert_eq!(r.build.install.prefix, "/usr/local");
    assert!(r.build.steps.prepare.is_none());
    assert_eq!(r.source.url.as_deref(), Some("https://nginx.org/download/nginx-1.27.3.tar.gz"));
    assert!(r.files.unknown_warn);
}

#[test]
fn minimal_recipe_defaults() {
    let r = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "https://example.com/hello-1.0.tar.gz"
sha256 = "abcd"

[build]
"#,
    )
    .unwrap();
    assert_eq!(r.pkg.build, 0);
    assert_eq!(r.build.system, "autoconf-make"); // значение по умолчанию
    assert_eq!(r.build.install.prefix, "/usr/local");
    assert!(r.deps.build.is_empty() && r.deps.run.is_empty());
    assert!(r.patches.is_empty());
    assert!(r.provides.r#virtual.is_empty());
    assert!(r.conflicts.pkgs.is_empty());
    assert!(!r.files.unknown_warn);
}

#[test]
fn source_file_variant() {
    let r = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
file = "dist/hello-1.0.tar.gz"
sha256 = "abcd"

[build]
"#,
    )
    .unwrap();
    assert_eq!(r.source.file.as_deref(), Some("dist/hello-1.0.tar.gz"));
    assert!(r.source.url.is_none());
}

#[test]
fn steps_override() {
    let r = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "u"
sha256 = "s"

[build]
system = "custom"

[build.steps]
prepare = ["./configure", "--prefix=${OPARYSH_PREFIX}"]
compile = ["make", "${OPARYSH_MAKEOPTS}"]
install = ["make", "install", "DESTDIR=${OPARYSH_DESTDIR}"]
"#,
    )
    .unwrap();
    let steps = r.build.steps;
    assert_eq!(steps.compile.unwrap(), vec!["make", "${OPARYSH_MAKEOPTS}"]);
    assert_eq!(steps.install.unwrap(), vec!["make", "install", "DESTDIR=${OPARYSH_DESTDIR}"]);
    assert_eq!(steps.prepare.unwrap()[0], "./configure");
}

#[test]
fn unknown_field_rejected() {
    let err = Recipe::parse(
        r#"
[pkg]
name = "hello"
version = "1.0"
bogus = 1

[source]
url = "u"
sha256 = "s"

[build]
"#,
    )
    .unwrap_err();
    assert!(err.contains("bogus"), "unexpected: {err}");
}

#[test]
fn missing_required_sections_rejected() {
    // нет [build]
    assert!(Recipe::parse(
        "[pkg]\nname=\"a\"\nversion=\"1\"\n[source]\nurl=\"u\"\nsha256=\"s\"\n"
    )
    .is_err());
    // нет [pkg]
    assert!(Recipe::parse("[build]\n").is_err());
}

#[test]
fn optional_dedup_order() {
    let mut m = BTreeMap::new();
    m.insert("a>=1".to_string(), "f1".to_string());
    m.insert("b".to_string(), "f2".to_string());
    let d = Deps {
        build: vec![],
        run: vec![],
        optional: m,
    };
    assert_eq!(d.optional.len(), 2);
}

#[test]
fn roundtrip_serialization() {
    let r = Recipe::parse(EXAMPLE).unwrap();
    let toml_s = toml::to_string(&r).unwrap();
    let r2 = Recipe::parse(&toml_s).unwrap();
    assert_eq!(r, r2);
}
