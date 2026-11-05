//! Юнит-тесты сбора требований (этап 5.1).

use std::collections::BTreeMap;
use std::path::Path;

use oparysh_core::recipe::Recipe;
use oparysh_core::version::DepReq;

use crate::collect::{collect_requirements, Candidate, PackageSource};
use crate::error::ResolveError;
use crate::repo_source::RepoPackageSource;

/// Поддельный источник пакетов в памяти: `name -> [(version, recipe_toml)]`.
/// Репозиторий у всех кандидатов — `"test"`, если не задан явно через
/// [`FakeSource::with_repos`].
#[derive(Default)]
struct FakeSource {
    // name -> [(version, repo, recipe_toml)]; порядок = порядок приоритета репо.
    pkgs: BTreeMap<String, Vec<(String, String, String)>>,
}

impl FakeSource {
    fn add(&mut self, name: &str, version: &str, recipe_toml: &str) {
        self.add_in("test", name, version, recipe_toml);
    }

    fn add_in(&mut self, repo: &str, name: &str, version: &str, recipe_toml: &str) {
        self.pkgs.entry(name.to_string()).or_default().push((
            version.to_string(),
            repo.to_string(),
            recipe_toml.to_string(),
        ));
    }
}

fn recipe(name: &str, version: &str, deps: &str) -> String {
    format!(
        "[pkg]\nname = \"{name}\"\nversion = \"{version}\"\n\n[deps]\n{deps}\n\n[build]\nsystem = \"make-only\"\n"
    )
}

impl PackageSource for FakeSource {
    fn candidates(&self, name: &str) -> crate::ResolveResult<Vec<Candidate>> {
        Ok(self
            .pkgs
            .get(name)
            .into_iter()
            .flatten()
            .map(|(v, repo, _)| Candidate {
                name: name.to_string(),
                version: oparysh_core::version::Version::parse(v).unwrap(),
                version_str: v.clone(),
                build: 1,
                repo: repo.clone(),
            })
            // Кандидаты в порядке приоритета репозитория, не в порядке вставки.
            .collect())
    }

    fn recipe(&self, candidate: &Candidate) -> crate::ResolveResult<Recipe> {
        let toml = self
            .pkgs
            .get(&candidate.name)
            .and_then(|vs| {
                vs.iter()
                    .find(|(v, repo, _)| v == &candidate.version_str && repo == &candidate.repo)
            })
            .map(|(_, _, t)| t.clone())
            .expect("кандидат из candidates()");
        Recipe::parse(&toml).map_err(|message| ResolveError::RecipeParse {
            name: candidate.name.clone(),
            version: candidate.version_str.clone(),
            message,
        })
    }
}

fn req(s: &str) -> DepReq {
    DepReq::parse(s).unwrap()
}

#[test]
fn chain_a_b_c_run_deps() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=1.0\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "run = [\"c>=1.0\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert_eq!(set.nodes.len(), 3);
    for name in ["a", "b", "c"] {
        assert!(set.nodes.contains_key(name), "{name}");
        assert!(set.nodes[name].as_run);
    }
    assert!(set.nodes["a"].is_target);
    assert!(!set.nodes["b"].is_target);
    assert_eq!(
        set.nodes["b"].required_by,
        vec![(Some("a".to_string()), crate::DepKind::Run)]
    );
}

#[test]
fn build_and_run_kinds_are_tracked() {
    let mut src = FakeSource::default();
    src.add(
        "a",
        "1.0",
        &recipe("a", "1.0", "build = [\"tool>=1.0\"]\nrun = [\"lib>=1.0\"]"),
    );
    src.add("tool", "1.0", &recipe("tool", "1.0", ""));
    src.add("lib", "1.0", &recipe("lib", "1.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert!(set.nodes["tool"].as_build);
    assert!(!set.nodes["tool"].as_run);
    assert!(set.nodes["lib"].as_run);
    assert!(!set.nodes["lib"].as_build);
    assert_eq!(set.build_only().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["tool"]);
}

#[test]
fn build_deps_of_dependencies_are_expanded() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "build = [\"c\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert!(set.nodes.contains_key("c"));
    assert!(set.nodes["c"].as_build);
}

#[test]
fn optional_deps_require_feature() {
    let mut src = FakeSource::default();
    src.add(
        "nginx",
        "1.0",
        &recipe(
            "nginx",
            "1.0",
            "optional = { \"openssl>=3.0\" = \"tls\" }",
        ),
    );
    src.add("openssl", "3.0", &recipe("openssl", "3.0", ""));

    let without = collect_requirements(&[req("nginx")], &src, &[]).unwrap();
    assert!(!without.nodes.contains_key("openssl"));

    let with = collect_requirements(&[req("nginx")], &src, &["tls".to_string()]).unwrap();
    assert!(with.nodes.contains_key("openssl"));
    assert!(with.nodes["openssl"].as_run);
}

#[test]
fn missing_dependency_is_unresolved() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"ghost>=1.0\"]"));

    let err = collect_requirements(&[req("a")], &src, &[]).unwrap_err();
    assert!(matches!(err, ResolveError::Unresolved { ref req, .. } if req == "ghost>=1.0.0"));
}

#[test]
fn no_satisfying_version_is_unresolved() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=2.0\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", ""));

    let err = collect_requirements(&[req("a")], &src, &[]).unwrap_err();
    assert!(matches!(err, ResolveError::Unresolved { ref req, .. } if req == "b>=2.0.0"));
}

#[test]
fn latest_satisfying_is_selected() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=1.0\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", ""));
    src.add("b", "2.0", &recipe("b", "2.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert_eq!(set.nodes["b"].version.to_string(), "2.0.0");
}

#[test]
fn version_conflict_is_reported() {
    // Две цели требуют несовместимые версии одной зависимости.
    let mut src = FakeSource::default();
    src.add("x", "1.0", &recipe("x", "1.0", "run = [\"c>=2.0\"]"));
    src.add("y", "1.0", &recipe("y", "1.0", "run = [\"c<2.0\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));
    src.add("c", "2.0", &recipe("c", "2.0", ""));

    let err = collect_requirements(&[req("x"), req("y")], &src, &[]).unwrap_err();
    assert!(matches!(err, ResolveError::VersionConflict { ref name, .. } if name == "c"));
}

#[test]
fn runtime_cycle_terminates() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "run = [\"a\"]"));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert_eq!(set.nodes.len(), 2);
    assert!(set.nodes["a"].as_run && set.nodes["b"].as_run);
}

#[test]
fn repo_priority_beats_version() {
    // b есть в обоих репозиториях; свежая версия — в менее приоритетном.
    let mut src = FakeSource::default();
    src.add_in("first", "app", "1.0", &recipe("app", "1.0", "run = [\"b>=1.0\"]"));
    src.add_in("first", "b", "1.0", &recipe("b", "1.0", ""));
    src.add_in("second", "b", "9.9", &recipe("b", "9.9", ""));

    let set = collect_requirements(&[req("app")], &src, &[]).unwrap();
    assert_eq!(set.nodes["b"].version.to_string(), "1.0.0");
    assert_eq!(set.nodes["b"].repo, "first");
}

/// Локальный мини-репозиторий в tmp-каталоге: один пакет `lib-1.0`.
fn local_repo(dir: &Path, name: &str, version: &str, recipe_toml: &str, deps: &[&str]) {
    use sha2::{Digest, Sha256};

    let recipe_path = dir.join("recipes").join(name).join(version).join("pkg.toml");
    std::fs::create_dir_all(recipe_path.parent().unwrap()).unwrap();
    std::fs::write(&recipe_path, recipe_toml).unwrap();
    std::fs::create_dir_all(dir.join("dist").join(name).join(version)).unwrap();
    std::fs::write(
        dir.join("dist").join(name).join(version).join(format!("{name}-{version}.tar.gz")),
        b"sources",
    )
    .unwrap();

    let recipe_sha = hex::encode(Sha256::digest(recipe_toml.as_bytes()));
    let src_sha = {
        let mut h = Sha256::new();
        h.update(b"sources");
        hex::encode(h.finalize())
    };
    let deps_toml = deps
        .iter()
        .map(|d| format!("\"{d}\","))
        .collect::<Vec<_>>()
        .join("");
    let index = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("index.toml"))
        .unwrap();
    use std::io::Write;
    let mut index = index;
    if std::fs::metadata(dir.join("index.toml")).unwrap().len() == 0 {
        write!(index, "[repo]\nname = \"test\"\n\n").unwrap();
    }
    write!(
        index,
        "[[pkg]]\nname = \"{name}\"\nversion = \"{version}\"\ndeps = [{deps_toml}]\nrecipe_sha = \"{recipe_sha}\"\nsrc_sha256 = \"{src_sha}\"\n\n"
    )
    .unwrap();
}

#[test]
fn repo_package_source_reads_local_repo() {
    let base = std::env::temp_dir().join(format!("oparysh_resolve_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let repo_dir = base.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let app_recipe = recipe("app", "1.0.0", "run = [\"lib>=1.0\"]");
    let lib_recipe = recipe("lib", "1.0.0", "");
    local_repo(&repo_dir, "app", "1.0.0", &app_recipe, &["lib>=1.0"]);
    local_repo(&repo_dir, "lib", "1.0.0", &lib_recipe, &[]);

    let entries = vec![oparysh_core::config::RepoEntry {
        name: "test".to_string(),
        path: Some(repo_dir.display().to_string()),
        url: None,
        key: None,
    }];
    let source = RepoPackageSource::new(&entries, &base.join("cache")).unwrap();
    let set = collect_requirements(&[req("app")], &source, &[]).unwrap();
    assert_eq!(set.nodes.len(), 2);
    assert_eq!(set.nodes["lib"].repo, "test");

    std::fs::remove_dir_all(&base).unwrap();
}
