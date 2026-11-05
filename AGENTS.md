# AGENTS.md — oparysh

Source-based пакетный менеджер на Rust (workspace). Спецификация проекта — `DOCS.md`, план работ — `ROADMAP.md` (этапы 0–3 ☑ выполнены, этап 4 repo — следующий). README и ROADMAP периодически расходятся с кодом: DOCS.md — первичный источник по формату рецептов, конфига, индексов, кодов возврата и переменных окружения.

## Команды

```sh
cargo build --workspace
cargo test --workspace          # то же, что гоняет CI (.github/workflows/ci.yml)
cargo test -p oparysh-core      # тесты одного crate
```

Отдельного `lint`/`fmt` в CI нет. `tests/integration/` пока пуст; юнит-тесты лежат внутри crate'­ов, фикстуры — `tests/fixtures/hello-1.0` (фальшивый make-пакет: `pkg.toml`, `hello.c`, `Makefile`, и его же `src_sha256`/`recipe_sha` нужны для тестов verifier/db).

## Структура

9 crate'ов в `crates/`, соответствуют подсистемам из DOCS.md §1: `cli` (только clap + точка входа оркестрации), `core` (типы рецепта `pkg.toml`, `index.toml`, `oparysh.conf`, версии, валидация — по DOCS §4–6), `resolver`, `fetcher` (ureq), `verifier` (SHA-256), `builder`, `installer`, `db` (rusqlite bundled + flock), `repo`. `contrib/` (комплшены, генератор index.toml) — заглушки, не реализовано.

Версии общих зависимостей и пути внутренних crate'ов — через `[workspace.dependencies]` в корневом `Cargo.toml` (наследуются `workspace = true`). Синхронный HTTP (ureq 3), не tokio/reqwest.

## Ключевые соглашения (иначе будет сломано)

- **Коды возврата фиксированы** (DOCS.md §11): например 3 = пакет не найден, **6 = sha256 mismatch** (и артефакт удаляется из кэша), 7 = ошибка сборки, 8 = ошибка/блокировка БД, 9 = удаление с rdeps без `--force`. Новые ошибки должны мапиться в существующие коды.
- **sha256 источника в рецепте обязателен** — отсутствие = жирная ошибка, не предупреждение. Целостность проверяется и на каждый установленный файл (`oparysh check`).
- **Сборки воспроизводимы**: `CFLAGS` из шелла **не наследуются** — только `OPARYSH_CFLAGS` и др. (`OPARYSH_DB_DIR`, `OPARYSH_CACHE_DIR`, `OPARYSH_ROOT`, `OPARYSH_JOBS`, `OPARYSH_CC`, `OPARYSH_PROFILE`, …) или конфиг. См. DOCS.md §12.
- **БД**: SQLite в `db_dir` (по умолчанию `/var/lib/oparysh`), advisory flock (`/var/lib/oparysh/lock`, err 8), миграции — нумерованные SQL в таблице `_migrations`, `.bak` + `rebuild-db`. Для тестов используйте временные `db_dir`, не системный.
- Прогресс реализации — отметить `☑` в `ROADMAP.md` после завершения этапа; каждый шаг должен держать workspace собираемым.
