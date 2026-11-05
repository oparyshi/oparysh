# mini-repo — фикстурный локальный репозиторий

Мини-репозиторий для тестов repo-подсистемы (этап 4.9 ROADMAP). Формат —
как у настоящего репозитория из DOCS §4:

```text
mini-repo/
├── index.toml
├── recipes/<name>/<ver>/pkg.toml
└── dist/<name>/<ver>/<name>-<ver>.tar.gz
```

Три пакета образуют короткую цепочку зависимостей:

- `libgreet` 1.0.0 — библиотека, `provides = ["greet"]`;
- `hello` 1.0.0 — зависит в рантайме от `libgreet>=1.0`;
- `hello-tools` 1.0.0 — зависит в рантайме от `hello>=1.0`.

Хэши `recipe_sha`/`src_sha256` в `index.toml` настоящие. Пересобрать индекс
после изменения рецептов или тарболов:

```sh
cargo run -p oparysh-repo-index-gen -- tests/fixtures/mini-repo \
    --name mini --origin https://example.org/mini --updated 1704067200
```

Интеграционные проверки фикстуры — `crates/oparysh-repo/tests/mini_repo.rs`.
