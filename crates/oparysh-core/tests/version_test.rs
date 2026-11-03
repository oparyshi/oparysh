//! Тесты парсера версий и требований (этап 1.3 ROADMAP).

use oparysh_core::version::*;

#[test]
fn parse_simple_version() {
    let v = Version::parse("1.2.3").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
    assert_eq!(v.pre, None);
    assert_eq!(v.build, None);
}

#[test]
fn parse_version_two_parts() {
    let v = Version::parse("1.2").unwrap();
    assert_eq!(v, Version { major: 1, minor: 2, patch: 0, pre: None, build: None });
}

#[test]
fn parse_version_one_part() {
    let v = Version::parse("1").unwrap();
    assert_eq!(v, Version { major: 1, minor: 0, patch: 0, pre: None, build: None });
}

#[test]
fn parse_version_with_pre_and_build() {
    let v = Version::parse("1.2.3-alpha.1+build.4").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
    assert_eq!(v.pre.as_deref(), Some("alpha.1"));
    assert_eq!(v.build.as_deref(), Some("build.4"));
}

#[test]
fn parse_version_invalid() {
    assert!(Version::parse("").is_err());
    assert!(Version::parse("abc").is_err());
    assert!(Version::parse("1.2.3.4").is_err());
}

#[test]
fn version_display() {
    let v = Version::parse("1.2.3").unwrap();
    assert_eq!(v.to_string(), "1.2.3");

    let v = Version::parse("1.2.3-rc.1+build.2").unwrap();
    assert_eq!(v.to_string(), "1.2.3-rc.1+build.2");
}

#[test]
fn version_ordering() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.0.1").unwrap();
    let v3 = Version::parse("1.1.0").unwrap();
    let v4 = Version::parse("2.0.0").unwrap();
    assert!(v1 < v2);
    assert!(v2 < v3);
    assert!(v3 < v4);
    assert!(v4 > v1);
}

#[test]
fn version_prerelease_is_lower_than_release() {
    let release = Version::parse("1.0.0").unwrap();
    let prerelease = Version::parse("1.0.0-alpha.1").unwrap();
    assert!(prerelease < release);
}

#[test]
fn dep_req_without_version() {
    let r = DepReq::parse("zlib").unwrap();
    assert_eq!(r.name, "zlib");
    assert!(r.req.is_none());
}

#[test]
fn dep_req_with_version() {
    let r = DepReq::parse("gcc>=11").unwrap();
    assert_eq!(r.name, "gcc");
    let req = r.req.unwrap();
    assert_eq!(req.op, Op::Ge);
    assert_eq!(req.version.major, 11);
    assert_eq!(req.version.minor, 0);
    assert_eq!(req.version.patch, 0);
}

#[test]
fn dep_req_all_operators() {
    let cases = vec![
        ("pkg=1.0", Op::Eq, "1.0.0"),
        ("pkg!=1.0", Op::Ne, "1.0.0"),
        ("pkg>1.0", Op::Gt, "1.0.0"),
        ("pkg<1.0", Op::Lt, "1.0.0"),
        ("pkg>=1.0", Op::Ge, "1.0.0"),
        ("pkg<=1.0", Op::Le, "1.0.0"),
        ("pkg^1.0", Op::Caret, "1.0.0"),
        ("pkg~1.0", Op::Tilde, "1.0.0"),
    ];
    for (s, op, ver) in cases {
        let r = DepReq::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(r.name, "pkg", "{s}");
        let req = r.req.as_ref().unwrap_or_else(|| panic!("{s}: no req"));
        assert_eq!(req.op, op, "{s}");
        assert_eq!(req.version.to_string(), ver, "{s}");
    }
}

#[test]
fn dep_req_invalid() {
    assert!(DepReq::parse("").is_err());
    assert!(DepReq::parse("=1.0").is_err());
    assert!(DepReq::parse("pkg@1.0").is_err());
}

#[test]
fn satisfies_eq_ne() {
    let v = Version::parse("1.2.3").unwrap();
    assert!(v.satisfies(&VersionReq { op: Op::Eq, version: Version::parse("1.2.3").unwrap() }));
    assert!(!v.satisfies(&VersionReq { op: Op::Ne, version: Version::parse("1.2.3").unwrap() }));
    assert!(v.satisfies(&VersionReq { op: Op::Ne, version: Version::parse("1.2.4").unwrap() }));
}

#[test]
fn satisfies_gt_lt_ge_le() {
    let v = Version::parse("1.2.3").unwrap();
    assert!(v.satisfies(&VersionReq { op: Op::Gt, version: Version::parse("1.2.2").unwrap() }));
    assert!(!v.satisfies(&VersionReq { op: Op::Gt, version: Version::parse("1.2.3").unwrap() }));
    assert!(v.satisfies(&VersionReq { op: Op::Ge, version: Version::parse("1.2.3").unwrap() }));
    assert!(v.satisfies(&VersionReq { op: Op::Lt, version: Version::parse("1.2.4").unwrap() }));
    assert!(!v.satisfies(&VersionReq { op: Op::Lt, version: Version::parse("1.2.3").unwrap() }));
    assert!(v.satisfies(&VersionReq { op: Op::Le, version: Version::parse("1.2.3").unwrap() }));
}

#[test]
fn satisfies_caret() {
    // ^1.2.3 -> >=1.2.3 <2.0.0
    let req = VersionReq { op: Op::Caret, version: Version::parse("1.2.3").unwrap() };
    assert!(Version::parse("1.2.3").unwrap().satisfies(&req));
    assert!(Version::parse("1.9.9").unwrap().satisfies(&req));
    assert!(!Version::parse("2.0.0").unwrap().satisfies(&req));
    assert!(!Version::parse("1.2.2").unwrap().satisfies(&req));

    // ^0.2.3 -> >=0.2.3 <0.3.0 (0.x.x: меняется minor)
    let req = VersionReq { op: Op::Caret, version: Version::parse("0.2.3").unwrap() };
    assert!(Version::parse("0.2.3").unwrap().satisfies(&req));
    assert!(Version::parse("0.2.9").unwrap().satisfies(&req));
    assert!(!Version::parse("0.3.0").unwrap().satisfies(&req));
}

#[test]
fn satisfies_tilde() {
    // ~1.2.3 -> >=1.2.3 <1.3.0
    let req = VersionReq { op: Op::Tilde, version: Version::parse("1.2.3").unwrap() };
    assert!(Version::parse("1.2.3").unwrap().satisfies(&req));
    assert!(Version::parse("1.2.9").unwrap().satisfies(&req));
    assert!(!Version::parse("1.3.0").unwrap().satisfies(&req));
    assert!(!Version::parse("1.2.2").unwrap().satisfies(&req));
}

#[test]
fn dep_req_display() {
    let r = DepReq::parse("gcc>=11").unwrap();
    assert_eq!(r.to_string(), "gcc>=11.0.0");

    let r = DepReq::parse("zlib").unwrap();
    assert_eq!(r.to_string(), "zlib");
}
