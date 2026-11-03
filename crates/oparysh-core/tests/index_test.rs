//! Тесты парсера index.toml (этап 1.4 ROADMAP, раздел 4 DOCS).

use oparysh_core::index::RepoIndex;

const INDEX: &str = r#"[repo]
name = "stable"
".origin" = "https://repo.example.com/stable"
updated = 1770000000

[[pkg]]
name    = "gcc"
version = "14.2.0"
build   = 2
arch    = ["x86_64", "aarch64"]
desc    = "GNU Compiler Collection"
tags    = ["devel", "toolchain"]
deps    = ["gmp>=6.2", "mpfr>=4.1", "zlib"]
provides = ["c-compiler"]
conflicts = ["clang<16"]
recipe_sha = "9f2c000000000000000000000000000000000000000000000000000000000000"
src_sha256 = "b04a000000000000000000000000000000000000000000000000000000000000"

[[pkg]]
name    = "zlib"
version = "1.3.1"
build   = 1
arch    = ["x86_64"]
desc    = "Compression library"
tags    = []
deps    = []
provides = []
conflicts = []
recipe_sha = "aaaa000000000000000000000000000000000000000000000000000000000000"
src_sha256 = "bbbb00000000000000000000000000000000000000000000000000000000000000"
"#;

#[test]
fn parse_full_index() {
    let idx = RepoIndex::parse(INDEX).unwrap();
    assert_eq!(idx.repo.name, "stable");
    assert_eq!(idx.repo.origin.as_deref(), Some("https://repo.example.com/stable"));
    assert_eq!(idx.repo.updated, 1770000000);

    assert_eq!(idx.pkg.len(), 2);

    let gcc = &idx.pkg[0];
    assert_eq!(gcc.name, "gcc");
    assert_eq!(gcc.version, "14.2.0");
    assert_eq!(gcc.build, 2);
    assert_eq!(gcc.arch, vec!["x86_64", "aarch64"]);
    assert_eq!(gcc.desc, "GNU Compiler Collection");
    assert_eq!(gcc.tags, vec!["devel", "toolchain"]);
    assert_eq!(gcc.deps, vec!["gmp>=6.2", "mpfr>=4.1", "zlib"]);
    assert_eq!(gcc.provides, vec!["c-compiler"]);
    assert_eq!(gcc.conflicts, vec!["clang<16"]);
    assert_eq!(gcc.conflicts, vec!["clang<16"]);
    assert_eq!(gcc.recipe_sha.len(), 64);
    assert!(gcc.recipe_sha.starts_with("9f2c"));
    assert_eq!(gcc.src_sha256.len(), 64);
    assert!(gcc.src_sha256.starts_with("b04a"));
    let zlib = &idx.pkg[1];
    assert_eq!(zlib.name, "zlib");
    assert_eq!(zlib.build, 1);
    assert!(zlib.deps.is_empty());
    assert!(zlib.provides.is_empty());
}

#[test]
fn parse_deps_returns_dep_reqs() {
    let idx = RepoIndex::parse(INDEX).unwrap();
    let gcc = &idx.pkg[0];
    let deps = gcc.parse_deps().unwrap();
    assert_eq!(deps.len(), 3);
    assert_eq!(deps[0].name, "gmp");
    let req0 = deps[0].req.as_ref().unwrap();
    assert_eq!(req0.op, oparysh_core::version::Op::Ge);
    assert_eq!(req0.version.major, 6);
    assert_eq!(deps[2].name, "zlib");
    assert!(deps[2].req.is_none());
}

#[test]
fn index_missing_required_fields() {
    // recipe_sha и src_sha256 обязательны — без них десериализация упадёт
    let bad = r#"
[repo]
name = "stable"

[[pkg]]
name = "gcc"
version = "1.0"
"#;
    assert!(RepoIndex::parse(bad).is_err());
}
