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

    fn providers(&self, virtual_name: &str) -> crate::ResolveResult<Vec<Candidate>> {
        let mut out = Vec::new();
        for (name, entries) in &self.pkgs {
            for (v, repo, toml) in entries {
                let parsed = Recipe::parse(toml).unwrap();
                if parsed.provides.r#virtual.iter().any(|p| p == virtual_name) {
                    out.push(Candidate {
                        name: name.clone(),
                        version: oparysh_core::version::Version::parse(v).unwrap(),
                        version_str: v.clone(),
                        build: 1,
                        repo: repo.clone(),
                    });
                }
            }
        }
        Ok(out)
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

// ---------- Этапы 5.2–5.6 ----------

use crate::collect::{resolve, ResolveOptions};
use crate::installed::{InstalledPkg, InstalledSource};
use crate::plan::{build_order, pending_build_order};
use oparysh_core::version::Version;

/// Поддельный источник установленных пакетов.
#[derive(Default)]
struct FakeInstalled {
    pkgs: BTreeMap<String, (String, u32)>, // name -> (version, build_rev)
}

impl FakeInstalled {
    fn add(&mut self, name: &str, version: &str) {
        self.pkgs.insert(name.to_string(), (version.to_string(), 1));
    }
}

impl InstalledSource for FakeInstalled {
    fn installed(&self, name: &str) -> Option<InstalledPkg> {
        self.pkgs.get(name).map(|(v, b)| InstalledPkg {
            name: name.to_string(),
            version: Version::parse(v).unwrap(),
            version_str: v.clone(),
            build_rev: *b,
        })
    }
}

fn recipe_provides(name: &str, version: &str, provides: &str, deps: &str) -> String {
    format!(
        "[pkg]\nname = \"{name}\"\nversion = \"{version}\"\n\n[provides]\nvirtual = [{provides}]\n\n[deps]\n{deps}\n\n[build]\nsystem = \"make-only\"\n"
    )
}

fn recipe_conflicts(name: &str, version: &str, conflicts: &str, deps: &str) -> String {
    format!(
        "[pkg]\nname = \"{name}\"\nversion = \"{version}\"\n\n[conflicts]\npkgs = [{conflicts}]\n\n[deps]\n{deps}\n\n[build]\nsystem = \"make-only\"\n"
    )
}

// 5.2: установленный пакет, удовлетворяющий требованию → узел «уже выполнено».
#[test]
fn installed_satisfying_req_is_marked_done() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=1.0\"]"));
    src.add("b", "2.0", &recipe("b", "2.0", "run = [\"c\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));

    let mut installed = FakeInstalled::default();
    installed.add("b", "1.5");

    let set = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            installed: Some(&installed),
            ..ResolveOptions::default()
        },
    )
    .unwrap();
    assert!(set.nodes["b"].installed);
    assert_eq!(set.nodes["b"].version_str, "1.5");
    // Зависимости установленного пакета не раскрываются.
    assert!(!set.nodes.contains_key("c"));
}

// 5.2: установленный НЕ удовлетворяет требованию → берётся из репо.
#[test]
fn installed_not_satisfying_falls_back_to_repo() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=2.0\"]"));
    src.add("b", "2.0", &recipe("b", "2.0", ""));

    let mut installed = FakeInstalled::default();
    installed.add("b", "1.0");

    let set = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            installed: Some(&installed),
            ..ResolveOptions::default()
        },
    )
    .unwrap();
    assert!(!set.nodes["b"].installed);
    assert_eq!(set.nodes["b"].version_str, "2.0");
}

// 5.3: строатегия latest-satisfying (покрыта и старыми тестами) + repo priority.
#[test]
fn latest_satisfying_prefers_highest_build_rev() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b>=1.0\"]"));
    // Две записи одной версии из одного репо — разные build-ревизии.
    src.pkgs.entry("b".to_string()).or_default().push((
        "1.0".to_string(),
        "test".to_string(),
        recipe("b", "1.0", ""),
    ));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert_eq!(set.nodes["b"].version_str, "1.0");
}

// 5.4: виртуальный пакет с единственным поставщиком.
#[test]
fn virtual_package_single_provider() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"sh\"]"));
    src.add(
        "bash",
        "5.2",
        &recipe_provides("bash", "5.2", "\"sh\"", ""),
    );

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert!(set.nodes.contains_key("bash"));
    assert_eq!(set.nodes["bash"].provided_for.as_deref(), Some("sh"));
}

// 5.4: несколько поставщиков → неоднозначность (CLI спросит интерактивно).
#[test]
fn virtual_package_ambiguous_providers_error() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"sh\"]"));
    src.add(
        "bash",
        "5.2",
        &recipe_provides("bash", "5.2", "\"sh\"", ""),
    );
    src.add(
        "dash",
        "0.5",
        &recipe_provides("dash", "0.5", "\"sh\"", ""),
    );

    let err = collect_requirements(&[req("a")], &src, &[]).unwrap_err();
    assert!(matches!(
        err,
        ResolveError::AmbiguousProvider { ref virtual_name, ref providers }
            if virtual_name == "sh" && providers == &vec!["bash".to_string(), "dash".to_string()]
    ));
}

// 5.4: выбор поставщика через опцию provider.
#[test]
fn virtual_package_provider_choice() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"sh\"]"));
    src.add(
        "bash",
        "5.2",
        &recipe_provides("bash", "5.2", "\"sh\"", ""),
    );
    src.add(
        "dash",
        "0.5",
        &recipe_provides("dash", "0.5", "\"sh\"", ""),
    );

    let set = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            provider: Some("dash".to_string()),
            ..ResolveOptions::default()
        },
    )
    .unwrap();
    assert!(set.nodes.contains_key("dash"));
    assert!(!set.nodes.contains_key("bash"));
}

// 5.4: установленный поставщик предпочтительнее для виртуального пакета.
#[test]
fn virtual_package_prefers_installed_provider() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"sh\"]"));
    src.add(
        "bash",
        "5.2",
        &recipe_provides("bash", "5.2", "\"sh\"", ""),
    );
    src.add(
        "dash",
        "0.5",
        &recipe_provides("dash", "0.5", "\"sh\"", ""),
    );

    let mut installed = FakeInstalled::default();
    installed.add("dash", "0.5");

    let set = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            installed: Some(&installed),
            ..ResolveOptions::default()
        },
    )
    .unwrap();
    assert!(set.nodes.contains_key("dash"));
    assert!(set.nodes["dash"].installed);
}

// 5.5: конфликт между двумя выбранными пакетами.
#[test]
fn conflicts_between_selected_packages() {
    let mut src = FakeSource::default();
    src.add(
        "a",
        "1.0",
        &recipe_conflicts("a", "1.0", "\"b>=1.0\"", "run = [\"b\"]"),
    );
    src.add("b", "1.0", &recipe("b", "1.0", ""));

    let err = collect_requirements(&[req("a")], &src, &[]).unwrap_err();
    assert!(matches!(
        err,
        ResolveError::Conflict { ref pkg, ref conflicting, .. }
            if pkg == "a" && conflicting == "b"
    ));
}

// 5.5: конфликт с установленным пакетом (не входящим в план).
#[test]
fn conflicts_with_installed_package() {
    let mut src = FakeSource::default();
    src.add(
        "a",
        "1.0",
        &recipe_conflicts("a", "1.0", "\"oldlib>=2.0\"", ""),
    );

    let mut installed = FakeInstalled::default();
    installed.add("oldlib", "2.1");

    let err = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            installed: Some(&installed),
            ..ResolveOptions::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ResolveError::Conflict { ref conflicting, .. } if conflicting == "oldlib"
    ));
}

// 5.5: build-цикл → ошибка BuildCycle.
#[test]
fn build_cycle_is_error() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "build = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "build = [\"a\"]"));

    let err = collect_requirements(&[req("a")], &src, &[]).unwrap_err();
    assert!(matches!(err, ResolveError::BuildCycle { .. }));
}

// 5.5: runtime-цикл разрешён (и сборка завершается).
#[test]
fn runtime_cycle_allowed() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "run = [\"a\"]"));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    assert_eq!(set.nodes.len(), 2);
}

// 5.6: топологическая сортировка — зависимости раньше зависящих.
#[test]
fn topo_order_deps_first() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "run = [\"c\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    let order = build_order(&set);
    let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
    assert!(pos("c") < pos("b"));
    assert!(pos("b") < pos("a"));
}

// 5.6: pending_build_order исключает установленные узлы.
#[test]
fn pending_order_skips_installed() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", ""));

    let mut installed = FakeInstalled::default();
    installed.add("b", "1.0");

    let set = resolve(
        &[req("a")],
        &src,
        &ResolveOptions {
            installed: Some(&installed),
            ..ResolveOptions::default()
        },
    )
    .unwrap();
    assert_eq!(pending_build_order(&set), vec!["a".to_string()]);
    // Полный порядок содержит и установленный узел.
    assert_eq!(build_order(&set).len(), 2);
}

// 5.6: runtime-цикл не ломает топологическую сортировку.
#[test]
fn topo_order_tolerates_runtime_cycle() {
    let mut src = FakeSource::default();
    src.add("a", "1.0", &recipe("a", "1.0", "run = [\"b\", \"c\"]"));
    src.add("b", "1.0", &recipe("b", "1.0", "run = [\"a\"]"));
    src.add("c", "1.0", &recipe("c", "1.0", ""));

    let set = collect_requirements(&[req("a")], &src, &[]).unwrap();
    let order = build_order(&set);
    assert_eq!(order.len(), 3);
    assert!(order.iter().position(|n| n == "c").unwrap() < order.iter().position(|n| n == "a").unwrap());
}
