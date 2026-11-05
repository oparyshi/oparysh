use oparysh_builder::*;
use oparysh_core::config::OparyshConfig;
use oparysh_core::recipe::Recipe;

const CONF: &str = r#"
[general]
jobs = 8

[toolchain]
cc = "gcc"
cxx = "g++"
cpp = "cpp"

[build.active]
profile = "release"

[profile.release]
cflags = "-O2 -pipe"
cxxflags = "-O2 -pipe"
ldflags = "-Wl,-O1"

[profile.debug]
cflags = "-O0 -g3"

[build.extra]
cflags = "-march=native"
cxxflags = "-march=native"
ldflags = "-Wl,--as-needed"
makeopts = "V=1"
"#;

fn make_recipe(build_section: &str) -> Recipe {
    let toml = format!(
        r#"
[pkg]
name = "hello"
version = "1.0"

[source]
url = "https://example.com/hello.tar.gz"
sha256 = "deadbeef"

[build]
{build_section}
"#
    );
    Recipe::parse(&toml).unwrap()
}

#[test]
fn flags_profile_extra_recipe_concat_left_to_right() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"cflags = "-DHELLO=1"
ldflags = "-lm"
makeopts = "VERBOSE=1"
"#);
    let f = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f.cflags, "-O2 -pipe -march=native -DHELLO=1");
    assert_eq!(f.cxxflags, "-O2 -pipe -march=native");
    assert_eq!(f.ldflags, "-Wl,-O1 -Wl,--as-needed -lm");
    assert_eq!(f.makeopts, "-j8 V=1 VERBOSE=1");
    assert_eq!(f.jobs, 8);
    assert_eq!(f.cc, "gcc");
    assert_eq!(f.cxx, "g++");
    assert_eq!(f.prefix, "/usr/local");
}

#[test]
fn recipe_cc_and_parallel_override_config() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"cc = "clang"
parallel = 16
"#);
    let f = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f.cc, "clang");
    assert_eq!(f.jobs, 16);
    assert_eq!(f.makeopts, "-j16 V=1");
}

#[test]
fn empty_recipe_cc_falls_back_to_config() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"cc = ""
"#);
    let f = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f.cc, "gcc");
}

#[test]
fn missing_active_profile_is_error() {
    let config = OparyshConfig::parse("[build.active]\nprofile = \"nope\"\n").unwrap();
    let recipe = make_recipe("");
    let err = compute_flags(&config, &recipe).unwrap_err();
    assert_eq!(err, FlagError::UnknownProfile("nope".into()));
}

#[test]
fn empty_fragments_are_skipped() {
    let config = OparyshConfig::parse("[profile.release]\ncflags = \"\"\n").unwrap();
    let recipe = make_recipe("cflags = \"-O3\"");
    let f = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f.cflags, "-O3");
}

fn subs() -> Substitutions {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("cflags = \"-DHELLO=1\"\n");
    let f = compute_flags(&config, &recipe).unwrap();
    Substitutions::new(f, "/var/lib/oparysh/build/hello-1.0", "/var/lib/oparysh/tmp/hello-1.0-destdir")
}

#[test]
fn substitute_all_documented_vars() {
    let s = subs();
    let out = substitute(
        "./configure --prefix=${OPARYSH_PREFIX} CC=${OPARYSH_CC} CFLAGS='${OPARYSH_CFLAGS}' jobs=${OPARYSH_JOBS} src=${OPARYSH_SRC_DIR} dest=${OPARYSH_DESTDIR} mo=${OPARYSH_MAKEOPTS}",
        &s,
    )
    .unwrap();
    assert_eq!(
        out,
        "./configure --prefix=/usr/local CC=gcc CFLAGS='-O2 -pipe -march=native -DHELLO=1' jobs=8 src=/var/lib/oparysh/build/hello-1.0 dest=/var/lib/oparysh/tmp/hello-1.0-destdir mo=-j8 V=1"
    );
}

#[test]
fn substitute_args_maps_each_element() {
    let s = subs();
    let args = vec![
        "make".to_string(),
        "${OPARYSH_MAKEOPTS}".to_string(),
        "DESTDIR=${OPARYSH_DESTDIR}".to_string(),
    ];
    let out = substitute_args(&args, &s).unwrap();
    assert_eq!(out, vec!["make", "-j8 V=1", "DESTDIR=/var/lib/oparysh/tmp/hello-1.0-destdir"]);
}

#[test]
fn unknown_var_is_error() {
    let s = subs();
    let err = substitute("echo ${OPARYSH_NOPE}", &s).unwrap_err();
    assert_eq!(err, SubstError::UnknownVar("OPARYSH_NOPE".into()));
}

#[test]
fn unterminated_var_is_error() {
    let s = subs();
    let err = substitute("echo ${OPARYSH_PREFIX", &s).unwrap_err();
    assert_eq!(err, SubstError::Unterminated);
}

#[test]
fn plain_text_without_vars_unchanged() {
    let s = subs();
    assert_eq!(substitute("make install", &s).unwrap(), "make install");
}
