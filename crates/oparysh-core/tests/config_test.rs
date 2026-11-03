//! Тесты парсера oparysh.conf (этап 1.5 ROADMAP, раздел 6 DOCS).

use oparysh_core::config::OparyshConfig;

const CONFIG: &str = r#"
[general]
db_dir = "/var/lib/oparysh"
cache_dir = "/usr/local/oparysh/cache"
destroot = "/"
jobs = 8
keep_build = false
assume_yes = false

[toolchain]
cc = "gcc"
cxx = "g++"
cpp = "cpp"
linker = ""

[build.active]
profile = "release"

[profile.release]
cflags = "-O2 -pipe -fstack-protector-strong"
cxxflags = "-O2 -pipe -fstack-protector-strong"
ldflags = "-Wl,-O1"

[profile.debug]
cflags = "-O0 -g3"
cxxflags = "-O0 -g3"

[build.extra]
cflags = "-march=native"
cxxflags = "-march=native"
ldflags = ""
makeopts = ""
env = {}

[repositories]
[[repositories.entries]]
name = "local"
path = "/srv/oparysh-repo"

[[repositories.entries]]
name = "stable"
url = "https://repo.example.com/stable"
key = "/etc/oparysh/keys/stable.gpg"
"#;

#[test]
fn parse_full_config() {
    let cfg = OparyshConfig::parse(CONFIG).unwrap();

    assert_eq!(cfg.general.db_dir, "/var/lib/oparysh");
    assert_eq!(cfg.general.cache_dir, "/usr/local/oparysh/cache");
    assert_eq!(cfg.general.destroot, "/");
    assert_eq!(cfg.general.jobs, 8);
    assert!(!cfg.general.keep_build);
    assert!(!cfg.general.assume_yes);

    assert_eq!(cfg.toolchain.cc, "gcc");
    assert_eq!(cfg.toolchain.cxx, "g++");
    assert_eq!(cfg.toolchain.cpp, "cpp");
    assert_eq!(cfg.toolchain.linker, "");

    assert_eq!(cfg.build.active.profile, "release");

    let release = cfg.find_profile("release").unwrap();
    assert_eq!(release.cflags, "-O2 -pipe -fstack-protector-strong");
    assert_eq!(release.cxxflags, "-O2 -pipe -fstack-protector-strong");
    assert_eq!(release.ldflags, "-Wl,-O1");

    let debug = cfg.find_profile("debug").unwrap();
    assert_eq!(debug.cflags, "-O0 -g3");

    assert_eq!(cfg.build.extra.cflags, "-march=native");
    assert!(cfg.build.extra.env.is_empty());

    assert_eq!(cfg.repositories.entries.len(), 2);
    assert_eq!(cfg.repositories.entries[0].name, "local");
    assert_eq!(cfg.repositories.entries[0].path.as_deref(), Some("/srv/oparysh-repo"));
    assert!(cfg.repositories.entries[0].url.is_none());

    assert_eq!(cfg.repositories.entries[1].name, "stable");
    assert_eq!(cfg.repositories.entries[1].url.as_deref(), Some("https://repo.example.com/stable"));
    assert_eq!(cfg.repositories.entries[1].key.as_deref(), Some("/etc/oparysh/keys/stable.gpg"));
}

#[test]
fn config_defaults() {
    let minimal = r#"
[general]
db_dir = "/var/lib/oparysh"
cache_dir = "/cache"
destroot = "/"
jobs = 4
keep_build = false
assume_yes = false

[toolchain]
cc = "gcc"
cxx = "g++"
cpp = "cpp"

[build.active]
profile = "release"
"#;
    let cfg = OparyshConfig::parse(minimal).unwrap();
    // build.extra should default to empty
    assert_eq!(cfg.build.extra.cflags, "");
    // no extra profiles defined
    assert!(cfg.find_profile("debug").is_none());
    // no repositories
    assert!(cfg.repositories.entries.is_empty());
}

#[test]
fn config_invalid_toml() {
    assert!(OparyshConfig::parse("not valid toml [[[").is_err());
}
