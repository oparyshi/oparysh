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

fn plan_for(build_section: &str) -> BuildPlan {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(build_section);
    build_plan(
        &config,
        &recipe,
        "/build/hello-1.0",
        "/tmp/hello-1.0-destdir",
    )
    .unwrap()
}

fn argv(plan: &BuildPlan, phase: BuildPhase) -> Vec<String> {
    plan.steps
        .iter()
        .find(|s| s.phase == phase)
        .unwrap()
        .argv
        .clone()
}

#[test]
fn autoconf_make_default_plan() {
    let plan = plan_for("");
    assert_eq!(plan.system, "autoconf-make");
    assert_eq!(argv(&plan, BuildPhase::Prepare), ["./configure", "--prefix=/usr/local"]);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["make", "-j8", "V=1"]);
    assert_eq!(
        argv(&plan, BuildPhase::Install),
        ["make", "install", "DESTDIR=/tmp/hello-1.0-destdir"]
    );
    assert_eq!(plan.environment.get("DESTDIR").unwrap(), "/tmp/hello-1.0-destdir");
}

#[test]
fn make_only_has_no_prepare_step() {
    let plan = plan_for(r#"system = "make-only""#);
    assert_eq!(
        plan.steps.iter().map(|s| s.phase).collect::<Vec<_>>(),
        [BuildPhase::Compile, BuildPhase::Install]
    );
}

#[test]
fn custom_uses_only_explicit_steps_and_substitutes() {
    let plan = plan_for(r#"system = "custom"
[build.steps]
prepare = ["echo", "${OPARYSH_PREFIX}"]
compile = ["make", "${OPARYSH_MAKEOPTS}"]
install = ["make", "install", "DESTDIR=${OPARYSH_DESTDIR}"]
"#);
    assert_eq!(argv(&plan, BuildPhase::Prepare), ["echo", "/usr/local"]);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["make", "-j8 V=1"]);
    assert_eq!(argv(&plan, BuildPhase::Install), ["make", "install", "DESTDIR=/tmp/hello-1.0-destdir"]);
}

#[test]
fn partial_steps_override_only_their_phase() {
    let plan = plan_for(r#"system = "autoconf-make"
[build.steps]
compile = ["gmake", "all"]
"#);
    assert_eq!(argv(&plan, BuildPhase::Prepare), ["./configure", "--prefix=/usr/local"]);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["gmake", "all"]);
    assert_eq!(argv(&plan, BuildPhase::Install).first().unwrap(), "make");
}

#[test]
fn cmake_ninja_default_plan() {
    let plan = plan_for(r#"system = "cmake-ninja""#);
    assert_eq!(argv(&plan, BuildPhase::Prepare), ["cmake", "-G", "Ninja", "-DCMAKE_INSTALL_PREFIX=/usr/local", "."]);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["ninja"]);
    assert_eq!(argv(&plan, BuildPhase::Install), ["ninja", "install"]);
    assert_eq!(plan.steps.iter().find(|s| s.phase == BuildPhase::Install).unwrap().env.get("DESTDIR").unwrap(), "/tmp/hello-1.0-destdir");
}

#[test]
fn cargo_default_plan_joins_destdir_and_prefix() {
    let plan = plan_for(r#"system = "cargo""#);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["cargo", "build", "--release"]);
    assert_eq!(
        argv(&plan, BuildPhase::Install),
        ["cargo", "install", "--root=/tmp/hello-1.0-destdir/usr/local"]
    );
    assert!(plan.steps.iter().all(|s| s.phase != BuildPhase::Prepare));
}

#[test]
fn meson_default_plan() {
    let plan = plan_for(r#"system = "meson""#);
    assert_eq!(argv(&plan, BuildPhase::Prepare), ["meson", "setup", "--prefix=/usr/local", "build"]);
    assert_eq!(argv(&plan, BuildPhase::Compile), ["ninja", "-C", "build"]);
    assert_eq!(argv(&plan, BuildPhase::Install), ["ninja", "-C", "build", "install"]);
}

#[test]
fn recipe_environment_is_preserved_and_destdir_added() {
    let plan = plan_for(r#"env = { LIBS = "-lm" }"#);
    assert_eq!(plan.environment.get("LIBS").unwrap(), "-lm");
    assert_eq!(plan.environment.get("DESTDIR").unwrap(), "/tmp/hello-1.0-destdir");
}

#[test]
fn narrowed_environment_has_fixed_path_and_profile_compilers() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("cflags = \"-DHELLO=1\"\n");
    let flags = compute_flags(&config, &recipe).unwrap();
    let env = narrowed_environment(&config, &recipe, &flags, "/tmp/hello-1.0-destdir");

    assert_eq!(env.get("PATH").unwrap(), BUILD_PATH);
    assert_eq!(env.get("CC").unwrap(), "gcc");
    assert_eq!(env.get("CXX").unwrap(), "g++");
    assert_eq!(env.get("CPP").unwrap(), "cpp");
    assert_eq!(env.get("CFLAGS").unwrap(), "-O2 -pipe -march=native -DHELLO=1");
    assert_eq!(env.get("CXXFLAGS").unwrap(), "-O2 -pipe -march=native");
    assert_eq!(env.get("LDFLAGS").unwrap(), "-Wl,-O1 -Wl,--as-needed");
    assert_eq!(env.get("MAKEOPTS").unwrap(), "-j8 V=1");
    assert_eq!(env.get("DESTDIR").unwrap(), "/tmp/hello-1.0-destdir");
}

#[test]
fn narrowed_environment_does_not_inherit_shell_vars() {
    // Пользовательские переменные из шелла не должны проникать в сборку.
    #[allow(deprecated)]
    {
        std::env::set_var("CFLAGS", "-O0 -DSHELL");
        std::env::set_var("PATH", "/evil/bin");
        std::env::set_var("OPARYSH_SECRET", "leak");
    }
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("");
    let flags = compute_flags(&config, &recipe).unwrap();
    let env = narrowed_environment(&config, &recipe, &flags, "/tmp/x");

    assert_eq!(env.get("CFLAGS").unwrap(), "-O2 -pipe -march=native");
    assert_eq!(env.get("PATH").unwrap(), BUILD_PATH);
    assert!(!env.contains_key("OPARYSH_SECRET"));

    #[allow(deprecated)]
    {
        std::env::remove_var("CFLAGS");
        std::env::remove_var("PATH");
        std::env::remove_var("OPARYSH_SECRET");
    }
}

#[test]
fn explicit_env_is_preserved_but_cannot_override_canonical() {
    let config = OparyshConfig::parse(
        "[general]\njobs = 4\n\n[toolchain]\ncc = \"gcc\"\n\n[profile.release]\ncflags = \"\"\n\n[build.extra]\nenv = { EXTRA = \"from-config\", CC = \"clang\" }\n",
    )
    .unwrap();
    let recipe = make_recipe(r#"env = { LIBS = "-lm", CFLAGS = "-O0" }"#);
    let flags = compute_flags(&config, &recipe).unwrap();
    let env = narrowed_environment(&config, &recipe, &flags, "/tmp/x");

    // Явно указанные переменные сохранены.
    assert_eq!(env.get("EXTRA").unwrap(), "from-config");
    assert_eq!(env.get("LIBS").unwrap(), "-lm");
    // Канонические переменные профиля побеждают `env` рецепта/конфига.
    assert_eq!(env.get("CC").unwrap(), "gcc");
    assert_eq!(env.get("CFLAGS").unwrap(), "");
}

#[test]
fn env_for_step_merges_step_and_plan() {
    let plan = plan_for(r#"env = { LIBS = "-lm" }"#);
    let install = plan
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Install)
        .unwrap();
    let env = plan.env_for_step(install);
    assert_eq!(env.get("LIBS").unwrap(), "-lm");
    assert_eq!(env.get("DESTDIR").unwrap(), "/tmp/hello-1.0-destdir");
    assert_eq!(env.get("PATH").unwrap(), BUILD_PATH);
}

#[test]
fn custom_without_steps_is_error() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"system = "custom""#);
    let err = build_plan(&config, &recipe, "/build/x", "/tmp/x").unwrap_err();
    assert_eq!(err, BuildPlanError::EmptyCustomSteps);
}

// ---------------------------------------------------------------------------
// 7.7 — перекрытия --set / --jobs (только на одну сборку)
// ---------------------------------------------------------------------------

fn override_pairs(pairs: &[&str]) -> oparysh_builder::FlagOverrides {
    let args: Vec<String> = pairs.iter().map(|s| s.to_string()).collect();
    oparysh_builder::FlagOverrides::parse(&args).unwrap()
}

#[test]
fn overrides_parse_known_keys() {
    let o = override_pairs(&["cflags=-O3 -flto", "cc=clang", "makeopts=V=2"]);
    assert_eq!(o.sets["cflags"], "-O3 -flto");
    assert_eq!(o.sets["cc"], "clang");
    assert_eq!(o.sets["makeopts"], "V=2");
    assert_eq!(o.jobs, None);
}

#[test]
fn overrides_parse_rejects_unknown_key_and_missing_eq() {
    let err = oparysh_builder::FlagOverrides::parse(&["tuples=1".to_string()]).unwrap_err();
    assert!(err.contains("неизвестный флаг"), "{err}");
    let err = oparysh_builder::FlagOverrides::parse(&["cflags".to_string()]).unwrap_err();
    assert!(err.contains("k=v"), "{err}");
}

#[test]
fn overrides_replace_final_flag_values() {
    // --set перекрывает финальное значение (профиль + extra + рецепт) целиком.
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("cflags = \"-DHELLO=1\"\n");
    let o = override_pairs(&["cflags=-O3 -flto", "cc=clang"]);
    let f = compute_flags_with(&config, &recipe, &o).unwrap();
    assert_eq!(f.cflags, "-O3 -flto");
    assert_eq!(f.cc, "clang");
    // Остальные флаги не затронуты.
    assert_eq!(f.cxx, "g++");
    assert_eq!(f.cxxflags, "-O2 -pipe -march=native");
    assert_eq!(f.jobs, 8);
    assert_eq!(f.makeopts, "-j8 V=1");
}

#[test]
fn overrides_jobs_replaces_jobs_and_makeopts_lead() {
    let config = OparyshConfig::parse(CONF).unwrap();
    // `parallel` в рецепте должен проиграть --jobs.
    let recipe = make_recipe("parallel = 3\n");
    let mut o = override_pairs(&[]);
    o.jobs = Some(16);
    let f = compute_flags_with(&config, &recipe, &o).unwrap();
    assert_eq!(f.jobs, 16);
    assert_eq!(f.makeopts, "-j16 V=1");
    // Рецепт сам по себе дал бы -j3.
    let f0 = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f0.makeopts, "-j3 V=1");
}

#[test]
fn overrides_set_makeopts_replaces_entirely() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("");
    let o = override_pairs(&["makeopts=V=2 VERBOSE=1"]);
    let f = compute_flags_with(&config, &recipe, &o).unwrap();
    // --set перекрывает значение целиком; число заданий при этом не трогается
    // (его меняет --jobs, который пересобирает лидер -j).
    assert_eq!(f.makeopts, "V=2 VERBOSE=1");
    assert_eq!(f.jobs, 8);
}

#[test]
fn overrides_do_not_mutate_config_or_recipe() {
    // Перекрытия живут только внутри вычисления: повторный вызов без
    // перекрытий даёт исходные флаги (конфиг/рецепт не изменены).
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe("");
    let _ = compute_flags_with(&config, &recipe, &override_pairs(&["cflags=-O0", "cc=tcc"])).unwrap();
    let f = compute_flags(&config, &recipe).unwrap();
    assert_eq!(f.cflags, "-O2 -pipe -march=native");
    assert_eq!(f.cc, "gcc");
}

#[test]
fn overrides_flow_into_plan_environment_and_steps() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"system = "autoconf-make""#);
    let mut o = override_pairs(&["cflags=-O3 -flto", "cc=clang"]);
    o.jobs = Some(2);
    let plan = build_plan_with(&config, &recipe, "/b/hello-1.0", "/d/hello", &o).unwrap();
    let env = &plan.environment;
    assert_eq!(env.get("CC").unwrap(), "clang");
    assert_eq!(env.get("CFLAGS").unwrap(), "-O3 -flto");
    let compile = plan
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Compile)
        .unwrap();
    assert_eq!(compile.argv, ["make", "-j2", "V=1"]);
    assert_eq!(compile.env.len(), 0);
    // Без перекрытий — прежний план.
    let plain = build_plan(&config, &recipe, "/b/hello-1.0", "/d/hello").unwrap();
    assert_eq!(plain.environment.get("CC").unwrap(), "gcc");
    assert_eq!(plain.environment.get("CFLAGS").unwrap(), "-O2 -pipe -march=native");
}

#[test]
fn unknown_system_is_error() {
    let config = OparyshConfig::parse(CONF).unwrap();
    let recipe = make_recipe(r#"system = "tup""#);
    let err = build_plan(&config, &recipe, "/build/x", "/tmp/x").unwrap_err();
    assert_eq!(err, BuildPlanError::UnknownSystem("tup".into()));
}
