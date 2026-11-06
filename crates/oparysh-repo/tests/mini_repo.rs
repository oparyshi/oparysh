//! Интеграционные проверки фикстурного мини-репозитория (этап 4.9 ROADMAP).
//!
//! Фикстура `tests/fixtures/mini-repo` — настоящий локальный репозиторий-
//! каталог из трёх пакетов (`libgreet` → `hello` → `hello-tools`) с реальными
//! `index.toml`, `recipes/` и тарболами в `dist/`. Здесь проверяется, что он
//! читается, проходит сверку хэшей и пригоден для команд repo-подсистемы.

use std::path::PathBuf;

use oparysh_core::config::RepoEntry;
use oparysh_repo::{
    generate_index, info_repos, list_available, outdated_repos, render_index, search_repos,
    sync_repos, GenIndexOptions, LocalRepo, SearchPattern, SyncKind, INDEX_FILE,
};

/// Корень фикстурного мини-репозитория.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mini-repo")
}

/// Запись конфига, указывающая на фикстуру как на локальный репозиторий.
fn entry() -> RepoEntry {
    RepoEntry {
        name: "mini".to_string(),
        path: Some(fixture().display().to_string()),
        url: None,
        key: None,
        snapshot: None,
    }
}

#[test]
fn fixture_opens_and_verifies() {
    let repo = LocalRepo::open(&fixture()).unwrap();

    assert_eq!(repo.index().repo.name, "mini");
    assert_eq!(repo.packages().len(), 3);

    // Все три пакета на месте, рецепты и тарболы сходятся с индексом.
    for (name, version) in [
        ("libgreet", "1.0.0"),
        ("hello", "1.0.0"),
        ("hello-tools", "1.0.0"),
    ] {
        assert!(repo.pkg(name, version).is_ok(), "нет записи {name}:{version}");
        let recipe = repo.read_recipe(name, version).unwrap();
        assert!(recipe.contains(&format!("name     = \"{name}\"")));
        repo.verify_source(name, version).unwrap();
        assert!(repo.dist_file(name, version).unwrap().is_file());
    }
    repo.verify().unwrap();

    // Цепочка зависимостей фикстуры: libgreet ← hello ← hello-tools.
    assert_eq!(
        repo.pkg("hello", "1.0.0").unwrap().parse_deps().unwrap().len(),
        2
    );
    assert!(repo
        .pkg("hello-tools", "1.0.0")
        .unwrap()
        .deps
        .iter()
        .any(|d| d.starts_with("hello>=")));
    assert_eq!(repo.pkg("libgreet", "1.0.0").unwrap().provides, ["greet"]);
}

#[test]
fn index_matches_generator() {
    // Фикстура должна оставаться воспроизводимой: ровно то, что выдаёт
    // `repo-index-gen` с зафиксированными name/origin/updated.
    let root = fixture();
    let mut options = GenIndexOptions::new("mini");
    options.origin = Some("https://example.org/mini".to_string());
    options.updated = Some(1_704_067_200);

    let generated = generate_index(&root, &options).unwrap();
    let rendered = render_index(&generated).unwrap();
    let on_disk = std::fs::read_to_string(root.join(INDEX_FILE)).unwrap();
    assert_eq!(rendered, on_disk);
}

#[test]
fn fixture_works_with_repo_commands() {
    let entries = [entry()];
    let cache = std::env::temp_dir().join(format!(
        "oparysh_mini_repo_cache_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&cache);

    // info
    let info = info_repos(&entries, &cache, "hello", None).unwrap();
    assert_eq!(info.repo, "mini");
    assert_eq!(info.pkg.version, "1.0.0");
    assert!(info.pkg.deps.iter().any(|d| d.starts_with("libgreet>=")));
    assert!(info_repos(&entries, &cache, "nope", None).is_err());

    // list --available: все три пакета по имени.
    let available = list_available(&entries, &cache).unwrap();
    let names: Vec<&str> = available
        .entries
        .iter()
        .map(|e| e.pkg.name.as_str())
        .collect();
    assert_eq!(names, ["hello", "hello-tools", "libgreet"]);

    // list --outdated: установленный hello 0.9.0 устарел.
    let outdated = outdated_repos(
        &entries,
        &cache,
        &[("hello".to_string(), "0.9.0".to_string())],
    )
    .unwrap();
    assert_eq!(outdated.entries.len(), 1);
    assert_eq!(outdated.entries[0].available, "1.0.0");
    assert_eq!(outdated.entries[0].repo, "mini");

    // search: glob по имени и regex по описанию.
    let glob = SearchPattern::parse("hello*").unwrap();
    let hits = search_repos(&entries, &cache, &glob).unwrap();
    let hit_names: Vec<&str> = hits.hits.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(hit_names, ["hello", "hello-tools"]);

    let regex = SearchPattern::parse("/greeting library/").unwrap();
    let hits = search_repos(&entries, &cache, &regex).unwrap();
    assert_eq!(hits.hits.len(), 1);
    assert_eq!(hits.hits[0].name, "libgreet");

    // sync локального репозитория — проверка без скачивания.
    let synced = sync_repos(&entries, &cache, &[]).unwrap();
    assert_eq!(synced.len(), 1);
    assert_eq!(synced[0].kind, SyncKind::Local);
    assert_eq!(synced[0].packages, 3);

    let _ = std::fs::remove_dir_all(&cache);
}
