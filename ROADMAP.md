# oparysh — ROADMAP

Пошаговый план реализации пакетного менеджера по [DOCS.md](DOCS.md).
Каждый шаг — маленький, проверяемый; в конце шага проект **собирается и работает**.
Отмечай прогресс: `☐` — не начато, `◐` — в работе, `☑` — готово.

Легенда зависимостей между этапами указана строкой «нужно:».

---

## Этап 0. Каркас проекта ☑
**нужно:** ничего

- [x] 0.1. `cargo init` workspace + `crates/` c пустыми crate'ами: `oparysh-cli`, `oparysh-core`, `oparysh-resolver`, `oparysh-fetcher`, `oparysh-verifier`, `oparysh-builder`, `oparysh-installer`, `oparysh-db`, `oparysh-repo`
- [x] 0.2. Выбрать и добавить зависимости: `clap`, `serde`, `toml`, `sha2`, `hex`, `rusqlite`, `reqwest` (или `ureq`), `anyhow`/`thiserror`
- [x] 0.3. Структура каталогов: `tests/integration`, `tests/fixtures`, `contrib/`
- [x] 0.4. `oparysh --version` собирается и печатает версию
- [x] 0.5. CI-скелет: `cargo build && cargo test` на каждый коммит

## Этап 1. Модель данных и разбор рецепта ☑
**нужно:** этап 0

- [x] 1.1. Определить типы рецепта (`pkg.toml`): `Pkg`, `Source`, `Deps`, `Patch`, `Provides`, `Conflicts`, `Build` — serde-структуры по разделу 5 DOCS
- [x] 1.2. Валидатор рецепта: обязательные поля (`[pkg]`, `[build]`, `sha256` источника — его отсутствие = жирная ошибка, не предупреждение)
- [x] 1.3. Парсер требований версий: операторы `>=, <=, >, <, =, ^, ~` + тип `VersionReq` / `Version` со сравнением (semver-совместимость ^ и ~)
- [x] 1.4. Парсер `index.toml` (раздел 4): `[repo]` + `[[pkg]]`, поля `recipe_sha`, `src_sha256`
- [x] 1.5. Парсер `oparysh.conf` (раздел 6): general, toolchain, profiles, build.extra, repositories
- [x] 1.6. Юнит-тесты на все парсеры на TOML-фикстурах
- [x] 1.7. Пример рецепта-фикстуры: фиктивный пакет `hello-1.0` (простой make-проект) для тестов

## Этап 2. Verifier (SHA-256) ☑
**нужно:** этап 1

- [x] 2.1. Функция `sha256_file(path) -> hex`
- [x] 2.2. Проверка тарбола против `[source].sha256`; несовпадение → код возврата **6**, артефакт удаляется из кэша
- [x] 2.3. Проверка каждого патча против `[[patches]].sha256`
- [x] 2.4. Проверка `recipe_sha` / `src_sha256` из index.toml
- [x] 2.5. Команда `oparysh verify <tarball>` (входные артефакты) — без `--rehash` по умолчанию
- [x] 2.6. Тесты: нужный/неверный хэш, отсутствующий хэш

## Этап 3. Локальная база (db) ☑
**нужно:** этап 1

- [x] 3.1. Схема БД по разделу 3: `packages`, `files`, `deps`, `build_info` + индекс `idx_files_pkg`
- [x] 3.2. Механизм миграций (нумерованные SQL-миграции, таблица `_migrations`)
- [x] 3.3. DAO: insert/get/list/delete package, insert files скопом, insert deps
- [x] 3.4. Advisory-lock: flock на `/var/lib/oparysh/lock`, ошибка **8** при блокировке
- [x] 3.5. Запись `pkg_hash` (sha256 рецепта + окружения сборки) и `build_info`
- [x] 3.6. Резервная копия `.bak` + команда `oparysh rebuild-db`
- [x] 3.7. Интеграционный тест: временный `db_dir`, install-запись, чтение, удаление каскадом

## Этап 4. Repo (индексы) ☑
**нужно:** этап 1, 2

- [x] 4.1. Чтение локального репозитория-каталога: `index.toml` + `recipes/` + `dist/`
- [x] 4.2. HTTP-клиент: скачивание `index.toml` в кэш (`/usr/local/oparysh/cache/repo/`)
- [x] 4.3. Приоритет репозиториев из `[repositories]` (local > network, порядок в конфиге)
- [x] 4.4. Команда `oparysh sync [repo…]` (+ проверка `recipe_sha` после скачивания)
- [x] 4.5. Команды `repo add / repo list / repo del` (правка конфига)
- [x] 4.6. Команда `oparysh search` (glob и `/regex/`) по имени/описанию/тегам
- [x] 4.7. Команды `oparysh info`, `oparysh list [--installed|--available|--outdated|--orphaned]`
- [x] 4.8. `contrib/repo-index-gen`: генератор `index.toml` по каталогу рецептов (основа `repo gen-index`)
- [x] 4.9. Тестовый фикстурный мини-репозиторий (2–3 пакета, локальный каталог)

## Этап 5. Resolver (зависимости) ☑
**нужно:** этап 1, 4

- [x] 5.1. Сбор требований: рекурсивное раскрытие `deps.build` и `deps.run`
- [x] 5.2. Учёт установленных: удовлетворённые требования → узлы «уже выполнено»
- [x] 5.3. Выбор версии: кандидаты по приоритету репо, стратегия `latest-satisfying`
- [x] 5.4. Виртуальные пакеты: `provides` → выбор поставщика, при конфликте интерактивный выбор
- [x] 5.5. Проверка `conflicts` и обнаружение циклов (runtime-цикл разрешён, build-цикл — ошибка **4**)
- [x] 5.6. Топологическая сортировка (Кан или DFS) → порядок сборки
- [x] 5.7. Вывод плана перед началом работ (формат из раздела 7: `#1 pcre2 10.44 (dep of nginx)… Total DL… Proceed? [y/N]`) + `--yes`
- [x] 5.8. Команды `deps [--tree]`, `rdeps`, `why`
- [x] 5.9. Тесты resolver'а: цепочка a→b→c, конфликт версий, цикл, виртуальный пакет

## Этап 6. Fetcher (кэш и загрузка) ☑
**нужно:** этап 2, 4

- [x] 6.1. Кэш исходников `src/`: найти в кэше → проверить sha256 → иначе скачать
- [x] 6.2. Скачивание тарболов из `[source].url` и из `dist/` репозитория (с проверкой `src_sha256`)
- [x] 6.3. Распаковка `.tar.gz` / `.tar.xz` / `.tar.zst` в `build/<name>-<ver>/` (гарантированно чистый каталог)
- [x] 6.4. Применение патчей по порядку (после проверки каждого хэша)
- [x] 6.5. Коды ошибок: сеть недоступна → **5**
- [x] 6.6. Команда `oparysh clean [--src|--build|--logs|--all]`
- [x] 6.7. Тесты: кэш-хит, кэш-мисс, битый хэш

## Этап 7. Builder (сборка) ☑
**нужно:** этап 1, 6

- [x] 7.1. Вычисление финальных флагов (раздел 6): `CFLAGS = profile + build.extra + recipe.build.cflags`, `MAKEOPTS = -j{jobs} + …`, приоритет `cc`: config → recipe
- [x] 7.2. Подстановки `${OPARYSH_PREFIX}` и др. в шагах
- [x] 7.3. Сборочные системы: `autoconf-make`, затем `make-only`, `custom` (через `[build.steps]`)
- [x] 7.4. Сборочные системы: `cmake-ninja`, `cargo`, `meson`
- [x] 7.5. Суженное окружение процесса: фиксированный `PATH`, `CC/CXX/CPP/CFLAGS/…` только из профиля; `env` из `env = {}`
- [x] 7.6. Логи сборки в `logs/<pkg>-<ver>-<date>.log`; шаг упал → сборка прервана, `build/` сохранён, код **7**
- [x] 7.7. Перекрытия (`--set cflags=… --set cc=…`, `--jobs N`) только на одну сборку
- [x] 7.8. Команда `oparysh build <pkg|recipe-dir>` — собрать, **не устанавливая**
- [x] 7.9. Интеграционный тест: сборка фиктивного `hello` из fixtures end-to-end в tmp-каталоге (`crates/oparysh-builder/tests/fixture_hello_e2e.rs`)

## Этап 8. Installer (установка) ☑
**нужно:** этап 3, 7

- [x] 8.1. Псевдо-`DESTDIR`: сборка в `<db_dir>/tmp/<pkg>-<ver>-destdir/` (`oparysh_installer::destdir`, `oparysh build`)
- [x] 8.2. Manifest scan: обход DESTDIR, список файлов + sha256 каждого; незаявленное (вне `install.prefix`) — предупреждение, с `--strict` — ошибка (учёт `[files].unknown_warn`) (`oparysh_installer::scan_manifest`, `ScanPolicy`)
- [x] 8.3. Commit: перенос файлов в `destroot`, запись `packages` + `files` + `deps` + `build_info` в БД единой транзакцией (`Database::commit_package`, `oparysh_installer::commit_package` / `move_to_destroot`)
- [x] 8.4. Кэш рецепта в `recipes/<name>/<ver>/pkg.toml` и cleanup (`build/`, `tmp/<pkg>-<ver>-destdir/`), если не `--keep-build` (`oparysh_installer::cache_recipe`, `oparysh_installer::cleanup`)
- [x] 8.5. Команда `install --file <pkg.toml>` — установка из локального рецепта минуя репо
- [x] 8.6. Оркестрация `oparysh install`: resolver → подтверждение → fetch/build/install по топологическому порядку
- [x] 8.7. Учёт `deps.build` vs `deps.run`: build-зависимости не тянутся в рантайм-граф
- [x] 8.8. Интеграционный тест: `install` цепочки из 2–3 фикстурных пакетов в изолированном корне

## Этап 9. Remover (удаление) ☑
**нужно:** этап 3, 8

- [x] 9.1. Удаление по манифесту `files` (только свои файлы, абсолютные канонические пути)
- [x] 9.2. Проверка обратных зависимостей: отказ без `--force` с кодом **9** (+ список пострадавших)
- [x] 9.3. `remove --force`, `autoremove` (сироты — установлены как зависимости, никому не нужны)
- [x] 9.4. Очистка связанной таблицы: recipes-кэш, записи БД (каскад)
- [x] 9.5. Тесты: удаление свободного пакета, отказ при rdeps, force, autoremove

## Этап 10. Upgrade / Downgrade / история ☑
**нужно:** этап 8, 9, 5

- [x] 10.1. `oparysh upgrade [pkg…]`: перерезолвинг установленных, пересборка зависимых (+1 к `build`)
- [x] 10.2. `oparysh downgrade <pkg> <ver>` из репо/кэша
- [x] 10.3. `oparysh history [pkg]` — журнал установок/обновлений/удалений (таблица events в БД или журнал-файлы)
- [x] 10.4. Проверка `--outdated`, `--orphaned` для `list`
- [x] 10.5. Сценарий-тест: install → upgrade → autoremove (как в приложении A) (`crates/oparysh-cli/tests/scenario_install_upgrade_autoremove.rs`)

## Этап 11. Диагностика и целостность установленного ☑
**нужно:** этап 3, 8

- [x] 11.1. `oparysh check [pkg…]`: сверка файлов с БД (sha256) → отчёт `removed | modified | new` (`crates/oparysh-cli/src/main.rs::run_check`, тесты `tests/check_cmd.rs`)
- [x] 11.2. Цветной вывод + `NO_COLOR` / `OPARYSH_NO_COLORS` (`crates/oparysh-cli/src/style.rs`, unit-тесты)
- [x] 11.3. Единые коды возврата (раздел 11): 0–9 во всех командах, тесты на коды (`crates/oparysh-cli/tests/exit_codes.rs`)
- [x] 11.4. Переменные окружения `OPARYSH_*` (раздел 12) — перекрытие конфига, тест (`OparyshConfig::apply_env_overrides[_with]`, `crates/oparysh-core/tests/env_overrides_test.rs`)
- [x] 11.5. `--root <path>` / `--db-dir` / `--cache-dir` / `--config` глобальные опции (`crates/oparysh-cli/tests/global_opts.rs`)
- [x] 11.6. `oparysh clean --all`, `check` для всех пакетов (аудит) (`oparysh check` без аргументов = полный аудит; `tests/check_cmd.rs::check_named_package_and_full_audit`)

## Этап 12. Конфиг и профили — полный цикл ☑
**нужно:** этап 7

- [x] 12.1. Развёртка `~` в путях, `oparysh.conf.example` в корне
- [x] 12.2. Активный профиль (`[build.active]`) + `OPARYSH_PROFILE` перекрытие
- [x] 12.3. `linker` ("mold", "ld.lld") в toolchain
- [x] 12.4. `assume_yes`, `keep_build`, `jobs` из `[general]`
- [x] 12.5. Тесты приоритетов флагов: profile ⊕ extra ⊕ recipe (конкатенация слева направо)

## Этап 13. Репозитории сетевые — полный цикл ☑
**нужно:** этап 4, 6

- [x] 13.1. `database.tar.zst` — оффлайн-снапшот (опционально)
- [x] 13.2. Подписи: detached `index.toml.sig` + `key` в записи репозитория; без ключа/с неверной подписью — отказ
- [x] 13.3. Докачка (resume) больших тарболов
- [x] 13.4. Тесты: подделанный index → отказ, отложенная догрузка

## Этап 14. UX и polish ☐
**нужно:** этапы 1–12 (по мере готовности)

- [ ] 14.1. Прогресс-бары загрузки/сборки, аккуратные таблицы вывода
- [ ] 14.2. Bash/zsh-комплшены в `contrib/shell`
- [ ] 14.3. Man-страница / `oparysh help` для всех команд
- [ ] 14.4. Ошибки в человеческом виде (thiserror → текст + hint), никаких паник на пользовательском вводе
- [ ] 14.5. Полный e2e-сценарий приложения A в интеграционных тестах: repo add → install --set … → check → sync/upgrade/autoremove

---

## Примерный порядок и вехи

| Веха | Этапы | Что можно делать руками |
|---|---|---|
| **M1 Умею читать и понимать пакеты** | 0–2 | парсинг рецептов, verify tarball |
| **M2 Знаю, что установлено** | 3 | БД, локальный lock |
| **M3 Вижу, что доступно** | 4 | sync / search / info / list на локальном репо |
| **M4 Строю план** | 5 | resolver с планом и подтверждением |
| **M5 Собираю** | 6–7 | fetch + build фиктивного пакета |
| **M6 Ставлю и снимаю** | 8–9 | полные install/remove в тестовом корне |
| **M7 Жизненный цикл** | 10–11 | upgrade/autoremove/check/history |
| **M8 Всё настроить** | 12–13 | профили, env-переменные, подписанные репо |
| **M9 Release-ready** | 14 | complешены, man, e2e, docs |

Правило: на каждый пункт — один PR (или одна сессия), тесты идут вместе с кодом, сборка зелёная после каждого пункта.
