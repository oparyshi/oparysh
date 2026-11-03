# oparysh — пакетный менеджер, собирающий из исходников

**oparysh** (*oparysh package manager*) — source-based пакетный менеджер на Rust. Пакеты собираются локально из исходного кода, зависимости разрешаются автоматически, поддерживаются локальные и сетевые репозитории, настраиваются флаги компиляции (`CFLAGS`, `MAKEOPTS`), выбор компилятора и проверка целостности через SHA-256.

---

## Содержание

1. [Архитектура](#1-архитектура)
2. [Структура репозитория проекта](#2-структура-репозитория-проекта)
3. [Структура локальной базы](#3-структура-локальной-базы)
4. [Структура удалённого репозитория](#4-структура-удалённого-репозитория)
5. [Формат рецепта (pkg.toml)](#5-формат-рецепта-pkgtoml)
6. [Конфигурация пользователя (oparysh.conf)](#6-конфигурация-пользователя-oparyshconf)
7. [Разрешение зависимостей](#7-разрешение-зависимостей)
8. [Сборка пакета](#8-сборка-пакета)
9. [Целостность и SHA-256](#9-целостность-и-sha-256)
10. [Команды CLI](#10-команды-cli)
11. [Коды возврата](#11-коды-возврата)
12. [Переменные окружения](#12-переменные-окружения)

---

## 1. Архитектура

oparysh состоит из следующих подсистем:

| Подсистема | Назначение |
|---|---|
| **resolver** | Построение графа зависимостей, топологическая сортировка, выбор версий |
| **fetcher** | Загрузка тарболов из сетевых репозиториев, кэширование |
| **verifier** | Проверка SHA-256 контрольных сумм всех входных артефактов |
| **builder** | Запуск процесса сборки в рабочем каталоге с пользовательскими флагами |
| **installer** | Установка через псевдо-`DESTDIR`, запись файлов в БД, актуация |
| **remover** | Обратная операция — снятие пакетов с проверкой обратных зависимостей |
| **db** | Локальная база установленных пакетов (SQLite) |
| **repo** | Чтение локальных/сетевых индексов репозиториев |

Каждый пакет описывается **рецептом** (`pkg.toml`) — декларативным файлом, в котором задано, где взять исходники, как собрать и что получится на выходе.

---

## 2. Структура репозитория проекта

Репозиторий исходного кода самого oparysh:

```
oparysh/
├── Cargo.toml               # Манифест crate'а (workspace)
├── Cargo.lock
├── oparysh.conf.example         # Пример системной конфигурации
├── docs/                    # Эта документация
├── crates/
│   ├── oparysh-cli/             # Точка входа, разбор аргументов (clap)
│   ├── oparysh-core/            # Оркестрация: install/remove/search и т.д.
│   ├── oparysh-resolver/        # Граф зависимостей, SAT-подобный выбор версий
│   ├── oparysh-fetcher/         # HTTP-клиент, докачка, кэш
│   ├── oparysh-verifier/        # SHA-256, подписи
│   ├── oparysh-builder/         # Санда-бокс сборки, окружение, логи
│   ├── oparysh-installer/       # DESTDIR-установка, манифесты файлов
│   ├── oparysh-db/              # SQLite-слой: миграции, DAO
│   └── oparysh-repo/            # Индексы репозиториев, форматы index.toml
├── tests/
│   ├── integration/         # Сборка фиктивных пакетов end-to-end
│   └── fixtures/            # Мини-репозитории для тестов
└── contrib/
    ├── repo-index-gen/      # Генератор index.toml для мейнтейнеров
    └── shell/               # Bash/zsh-комплшены
```

---

## 3. Структура локальной базы

Все пути по умолчанию (перезываются в `oparysh.conf` и переменными окружения `OPARYSH_*`):

```
/var/lib/oparysh/                    # Корень базы (OPARYSH_DB_DIR)
├── db.sqlite                    # SQLite: метаданные установленных пакетов
├── recipes/                     # Кэш рецептов, из которых ставились пакеты
│   └── gcc/14.2.0/pkg.toml
├── src/                         # Кэш исходников (тарболы + распаковка)
│   └── gcc-14.2.0.tar.xz
├── build/                       # Рабочие каталоги сборки
│   └── gcc-14.2.0/
├── logs/                        # Полные логи сборок: pkg-version-date.log
└── tmp/                         # Временные DESTDIR-каталоги

/usr/local/oparysh/cache/repo/       # Кэш индексов и тарболов сетевых репозиториев
```

### Схема db.sqlite

```sql
-- Установленные пакеты
CREATE TABLE packages (
    name        TEXT PRIMARY KEY,   -- "gcc"
    version     TEXT NOT NULL,      -- "14.2.0"
    build_rev   INTEGER NOT NULL,   -- ревизия рецепта (build = 2)
    arch        TEXT NOT NULL,      -- "x86_64"
    pkg_hash    TEXT NOT NULL,      -- sha256 рецепта + окружения сборки
    install_ts  INTEGER NOT NULL,
    size_bytes  INTEGER NOT NULL
);

-- Список файлов пакета (для корректного удаления)
CREATE TABLE files (
    pkg         TEXT NOT NULL REFERENCES packages(name) ON DELETE CASCADE,
    path        TEXT NOT NULL,       -- "/usr/bin/gcc", абсолютный, канонический
    sha256      TEXT NOT NULL        -- хэш содержимого на момент установки
);
CREATE INDEX idx_files_pkg ON files(pkg);

-- Рёбра графа зависимостей (снимок на момент установки)
CREATE TABLE deps (
    pkg         TEXT NOT NULL REFERENCES packages(name) ON DELETE CASCADE,
    dep_name    TEXT NOT NULL,
    req         TEXT NOT NULL        -- исходное требование: ">=3.2, <4"
);

-- Флаги сборки, зафиксированные при установке (для отчётов и воспроизводимости)
CREATE TABLE build_info (
    pkg         TEXT PRIMARY KEY REFERENCES packages(name) ON DELETE CASCADE,
    cc          TEXT NOT NULL,
    cflags      TEXT NOT NULL,
    makeopts    TEXT NOT NULL,
    profile     TEXT NOT NULL,       -- имя профиля из oparysh.conf
    cmd         TEXT NOT NULL        -- точная команда сборки
);
```

### Другие файлы базы

- `/var/lib/oparysh/lock` — файловый advisory-lock (flock), исключает параллельные транзакции oparysh.
- `/etc/oparysh/oparysh.conf` — системная конфигурация (см. раздел 6).

---

## 4. Структура удалённого репозитория

Сетевой репозиторий — статический каталог, раздаваемый по HTTP(S), либо локальный каталог:

```
repo.example.com/stable/
├── index.toml               # Индекс всех пакетов репозитория
├── recipes/<name>/<ver>/pkg.toml
├── dist/<name>/<ver>/<name>-<ver>.tar.xz    # Тарболы исходников
└── database.tar.zst         # (опционально) снапшот для оффлайн-синхронизации
```

`index.toml` — единственный файл, который oparysh скачивает при `oparysh sync`:

```toml
[repo]
name    = "stable"
.origin = "https://repo.example.com/stable"
updated = 1770000000

[[pkg]]
name    = "gcc"
version = "14.2.0"
build   = 2
arch    = ["x86_64", "aarch64"]
desc    = "GNU Compiler Collection"
tags    = ["devel", "toolchain"]
deps    = ["gmp>=6.2", "mpfr>=4.1", "zlib"]
provides= ["c-compiler"]
conflicts= ["clang<16"]
recipe_sha = "9f2c…"     # sha256 файла pkg.toml
src_sha256 = "b04a…"     # sha256 тарбола в dist/
```

Ключи `[pkg]` зеркально повторяют поля рецепта (раздел 5); `recipe_sha` и `src_sha256` обязательны.

---

## 5. Формат рецепта (pkg.toml)

Рецепты пишутся на TOML. Обязательные секции — `[pkg]` и `[build]`.

```toml
[pkg]
name     = "nginx"
version  = "1.27.3"
build    = 1                  # перебилд при том же апстрим-версии: делайте +1
arch     = ["x86_64"]
desc     = "HTTP-сервер и reverse-proxy"
license  = "BSD-2-Clause"
homepage = "https://nginx.org"

# Источник: локальный путь, URL или имя файла в репозитории
[source]
url      = "https://nginx.org/download/nginx-1.27.3.tar.gz"
sha256   = "5f2b…"            # ОБЯЗАТЕЛЬНО. Отсутствие/неверное значение = отказ сборки
# или: file = "dist/nginx-1.27.3.tar.gz"   # берётся из репозитория

# Требования. Поддерживаются операторы: >=, <=, >, <, =, ^, ~
[deps]
build    = ["gcc>=11", "make>=4.0", "pcre2>=10.32"]   # нужны только при сборке
run      = ["pcre2>=10.32", "zlib>=1.2"]              # нужны в рантайме
# optional = { "openssl>=3.0" = "tls" }              # опциональная фича "tls"

[[patches]]                   # применяется по порядку, каждый проверяется по sha256
file   = "fix-musl.patch"
sha256 = "3c1d…"

[provides]
virtual = ["httpd"]           # виртуальные предоставляемые возможности

[conflicts]
pkgs = ["apache<2.6"]

[build]                       # СБОРКА: всё это юзер может перекрыть через oparysh.conf
system   = "autoconf-make"    # см. таблицу ниже; по умолчанию "autoconf-make"
cc       = ""                 # пусто = берётся из конфига/профиля
cflags   = ""                 # доп. флаги ПОВЕРХ профиля (см. 6)
ldflags  = ""
makeopts = ""                 # доп. аргументы make
parallel = 0                  # 0 = из config (jobs)
env      = { LIBS = "-lm" }   # произвольные переменные окружения

[build.install]
prefix  = "/usr/local"        # DESTDIR применяется всегда

[build.steps]                 # необязательно: полное переопределение команд
# prepare = ["./configure", "--prefix=${OPARYSH_PREFIX}"]
# compile = ["make", "${OPARYSH_MAKEOPTS}"]
# install = ["make", "install", "DESTDIR=${OPARYSH_DESTDIR}"]

[files]                       # необязательные действия после установки
unknown_warn = true           # предупреждать, если make установил незаявленные файлы
```

### Поддерживаемые сборочные системы

| `system` | Действия по умолчанию |
|---|---|
| `autoconf-make` | `./configure --prefix=$PREFIX` → `make` → `make install DESTDIR=` |
| `cmake-ninja` | `cmake -G Ninja -DCMAKE_INSTALL_PREFIX=$PREFIX` → `ninja` → `ninja install` |
| `cargo` | `cargo build --release` → `cargo install --root=$DESTDIR/$PREFIX` |
| `meson` | `meson setup --prefix=$PREFIX` → `ninja` → `ninja install` |
| `make-only` | только `make` / `make install` |
| `custom` | шаги берутся строго из `[build.steps]` |

Внутри шагов доступны подстановки: `${OPARYSH_PREFIX}`, `${OPARYSH_DESTDIR}`, `${OPARYSH_MAKEOPTS}`, `${OPARYSH_CFLAGS}`, `${OPARYSH_CC}`, `${OPARYSH_JOBS}`, `${OPARYSH_SRC_DIR}`.

---

## 6. Конфигурация пользователя (oparysh.conf)

`/etc/oparysh/oparysh.conf`. Именно здесь «тюнят» сборку. Везде работает развёртка `~`.

```toml
[general]
db_dir        = "/var/lib/oparysh"
cache_dir     = "/usr/local/oparysh/cache"
destroot      = "/"               # корень, куда реально устанавливаются файлы
jobs          = 8                 # -j для make; перекрывает параллель из рецепта
keep_build    = false             # не удалять build/-каталог после успешной сборки
assume_yes    = false             # не задавать вопросов

# Компиляторы. cc/cxx/cpp — точные команды или абсолютные пути.
[toolchain]
cc  = "gcc"                       # напр.: "clang", "/opt/gcc-15/bin/gcc"
cxx = "g++"
cpp = "cpp"
linker = ""                       # "", использ. по умолчанию; иначе: "mold", "ld.lld"

# Профили CFLAGS. Выбирается один активный: profile = "release" ([build.active])
[build.active]
profile = "release"

[profile.release]
cflags   = "-O2 -pipe -fstack-protector-strong"
cxxflags = "-O2 -pipe -fstack-protector-strong"
ldflags  = "-Wl,-O1"

[profile.debug]
cflags   = "-O0 -g3"
cxxflags = "-O0 -g3"

[profile.size]
cflags   = "-Os -s"

# Дополнительные флаги, СКЛЕИВАЮТСЯ с флагами активного профиля.
# Полезно, чтобы не переписывать профиль целиком:
[build.extra]
cflags   = "-march=native"
cxxflags = "-march=native"
ldflags  = ""
makeopts = ""                     # добавляется к make в дополнение к -j{jobs}
env      = {}

# Приоритет порядка разрешения (ниже — выше приоритет)
[repositories]                    # несколько [[entries]] или таблицы
[[repositories.entries]]
name = "local"                   # локальный каталог-репо
path = "/srv/oparysh-repo"

[[repositories.entries]]
name   = "stable"
url    = "https://repo.example.com/stable"
key    = "/etc/oparysh/keys/stable.gpg"   # ключ проверки подписанного index.toml (опц.)
```

**Приоритет наложения флагов** (конкатенация слева направо):

```
финальные CFLAGS = [profile.cflags] + [build.extra.cflags] + [recipe.build.cflags]
финальные MAKEOPTS = "-j{jobs} " + [config makeopts] + [recipe.build.makeopts]
CC = toolchain.cc.recipe.build.cc   (если у рецепта cc непуст, он побеждает)
```

Отдельный пакет можно собрать с индивидуальными флагами без правки рецепта:

```
oparysh build --with nginx --set cflags="-O3 -flto" --set cc=clang --jobs 16
```

---

## 7. Разрешение зависимостей

Алгоритм:

1. **Сбор требований.** Начинаем с цели; рекурсивно раскрываем `deps.build` (только на время сборки) и `deps.run`. Опциональные зависимости разрешаются, если включены соответствующие фичи (`oparysh install nginx +tls`).
2. **Сопоставление с установленными.** Если установленный пакет удовлетворяет требованию — включается в граф как уже выполненный узел.
3. **Выбор версии.** Кандидаты берутся со всех включённых репозиториев по приоритету (`[repositories]`); среди версий применяется стратегия `latest-satisfying` по умолчанию (можно `pin` в конфиге).
4. **Виртуальные пакеты.** `provides` виртуальное имя — oparysh выбирает поставщика приоритетнее; при конфликте предлагает выбор интерактивно.
5. **Проверка конфликтов** (`conflicts`) и циклов. Циклы различаем: runtime-цикл разрешён (оряжаемся при сборке существующими пакетами), build-цикл — ошибка.
6. **Топологическая сортировка** (Кан/DFS) → порядок сборки. Порядок выводится пользователю **до** начала работ:

```
The following 3 package(s) will be BUILT, 1 INSTALLED as dependency:

  #1  pcre2  10.44  (dep of nginx)
  #2  zlib   1.3.1  (dep of nginx)
  #3  nginx  1.27.3 (target)

Total DL: 14.2 MiB, build jobs: 8. Proceed? [y/N]
```

7. **Обновления.** `oparysh upgrade` повторяет перерезолвинг для установленных пакетов; пакеты, чьи зависимости должны обновиться, пересобираются (+1 к `build`).

---

## 8. Сборка пакета

Шаги для каждого пакета:

1. **fetch** — тарбол берётся из кэша (`src/`) или скачивается в него.
2. **verify** — проверка `sha256` тарбола, патчей и рецепта (см. 9).
3. **extract** — в `build/<name>-<ver>/`; гарантируется чистый каталог.
4. **prepare / compile / install** — команды из `[build.steps]` или по `system`. Выполняются с:
   - `CC`, `CXX`, `CPP` из профиля toolchain;
   - `CFLAGS`/`CXXFLAGS`/`LDFLAGS` = результирующие;
   - `MAKEOPTS = -j<jobs> …`;
   - `DESTDIR=/var/lib/oparysh/tmp/<pkg>-<ver>-destdir/`.
5. **manifest scan.** Обход DESTDIR: собирается список файлов + sha256 каждого. Незаявленные файлы — предупреждение либо (`--strict`) ошибка.
6. **commit.** Файлы из DESTDIR переносятся в `destroot`, запись + files в `db.sqlite`, лог пишется в `logs/`.
7. **cleanup.** build/ и tmp/ каталоги удаляются (если не `--keep-build`).

Сборка выполняется в суженном окружении: `PATH` фиксируется, переменные вроде `LDFLAGS` пользователем не проникают, кроме явно указанных. Каждый шаг падает → сборка прерывается, лог в `logs/`, каталог `build/` сохраняется.

---

## 9. Целостность и SHA-256

Проверяются **все** артефакты по цепочке:

| Артефакт | Где указан хэш | Когда проверяется |
|---|---|---|
| тарбол исходников | `[source].sha256` | `fetch` и повторно перед `extract` |
| каждый патч | `[[patches]].sha256` | перед применением |
| файл рецепта | `recipe_sha` в `index.toml` | `oparysh sync`, при загрузке из сети |
| тарбол в `dist/` | `src_sha256` в `index.toml` | при скачивании из репо |
| каждый установленный файл | `files.sha256` в БД | `oparysh check` |
| локальная база | `packages.pkg_hash` | при любом запросе к пакету |

Требования:

-- Поддерживается только алгоритм SHA-256 (hex, нижний регистр). Отсутствие `sha256` у источника — жирная ошибка, а не предупреждение.
- Несовпадение — сборка прерывается, артефакт удаляется из кэша, предлагается `oparysh verify --rehash` (только для явного обновления рецепта; по умолчанию никогда не «пересчитывает» хэш сам).
- `oparysh verify` — проверяет текущую целостность установленных файлов против базы; обнаруженные расхождения выводится таблица или `removed|modified|new`.

Подписи (опционально): если у репо указан `key`, то `index.toml` должен быть подписан detached-подписью `index.toml.sig`.

---

## 10. Команды CLI

```
oparysh [<глобальные опции>] <command> [args]
```

Глобальные опции: `--config <path>`, `--db-dir <path>`, `--cache-dir <path>`, `--verbose`, `--yes`, `--jobs N`, `--root <path>` (chroot-установка в другой корень).

### Синхронизация и поиск

| Команда | Описание |
|---|---|
| `oparysh sync [repo …]` | Скачать/обновить `index.toml` всех (или указанных) репозиториев |
| `oparysh search <pattern>` | Поиск по имени/описанию/тегам (glob или regex с `/…/`) |
| `oparysh info <pkg>[@ver]` | Полный метадата пакета: deps, provides, version, repo |
| `oparysh list [--installed\|--available\|--outdated\|--orphaned]` | Список пакетов |

### Установка / удаление

| Команда | Описание |
|---|---|
| `oparysh install <pkg…>` | Разрешить зависимости, собрать и установить цели + build-зависимости |
| `oparysh install —file <pkg.toml>` | Собрать и поставить из локального рецепта, минуя репо |
| `oparysh remove <pkg…>` | Удалить пакет(ы); отказ, если кто-то ещё зависит (см. `--force`) |
| `oparysh remove --force <pkg>` | Удалить даже при обратных зависимостях (выводит список пострадавших) |
| `oparysh autoremove` | Удалить пакеты-«сироты» (стояли как зависимости, никому не нужны) |
| `oparysh upgrade [pkg…]` | Обновить все/указанные пакеты |
| `oparysh downgrade <pkg> <ver>` | Откат на конкретную версию (если она есть в репо/кэше) |

### Сборка

| Команда | Описание |
|---|---|
| `oparysh build <pkg\|recipe-dir>` | Только собрать, **не устанавливая**. Артефакт в `build/` |
| `oparysh build --set k=v …` | Перекрыть cflags/cc/makeopts/… с KLAB-опциями, только на эту сборку |
| `oparysh build --jobs N` | Перекрыть `-j` |
| `oparysh clean [--src\|--build\|--logs\|--all]` | Очистить кэши |

### Диагностика / обслуживание

| Команда | Описание |
|---|---|
| `oparysh check [pkg…]` | Сверить файлы пакета с БД (sha256) — потерянные/изменённые |
| `oparysh verify <pkg\|tarball>` | Проверить хэши входных артефактов |
| `oparysh deps <pkg> [--tree]` | Дерево зависимостей |
| `oparysh rdeps <pkg>` | Обратные зависимости (`grep` по `deps`) |
| `oparysh history [pkg]` | Журнал установок/обновлений/удалений |
| `oparysh rebuild-db` | Валидация и восстановление `db.sqlite` from `.bak` |
| `oparysh why <pkg>` | Объяснить, зачем пакет установлен |

### Репо-мейнтейнинг

| Команда | Описание |
|---|---|
| `oparysh repo add <name> <url\|path>` | Добавить репозиторий в конфиг |
| `oparysh repo list` | Показать репозитории, кол-во пакетов, дату обновления |
| `oparysh repo del <name>` | Убрать репозиторий |
| `oparysh repo gen-index <dir>` | Сгенерировать `index.toml` по каталогу рецептов |

### Примеры

```sh
oparysh sync && oparysh install nginx                # классика
oparysh install nginx --set cflags="-O3 -flto" --set cc=clang
oparysh remove --force pcre2                     # осознанный риск
oparysh check                                    # аудит целостности всех пакетов
```

---

## 11. Коды возврата

| Код | Значение |
|---|---|
| `0` | успех |
| `1` | общая ошибка |
| `2` | неверные аргументы CLI |
| `3` | пакет не найден ни в одном репо |
| `4` | не удаётся удовлетворить зависимости (конфликт/версии) |
| `5` | ошибка загрузки (сеть/репо недоступен) |
| `6` | **sha256 mismatch / артефакт не прошёл проверку целостности** |
| `7` | ошибка сборки (шаг вернул != 0) |
| `8` | ошибка базы данных (блокировка, повреждение) |
| `9` | запрещено: удаление пакета, от которого зависят другие (без `--force`) |

---

## 12. Переменные окружения

| Переменная | Действие | Пример |
|---|---|---|
| `OPARYSH_DB_DIR` | перекрывает `db_dir` | `/mnt/chroot1/var/lib/oparysh` |
| `OPARYSH_CACHE_DIR` | перекрывает `cache_dir` | `/tmp/oparysh-cache` |
| `OPARYSH_ROOT` | эквивалент `--root` | `/mnt/chroot2` |
| `OPARYSH_JOBS` | эквивалент `[general].jobs` | `16` |
| `OPARYSH_CC` / `OPARYSH_CXX` | перекрывают компилятор | `clang` |
| `OPARYSH_CFLAGS` / `OPARYSH_CXXFLAGS` | доп. флаги поверх профиля (как `build.extra`) | `-flto` |
| `OPARYSH_MAKEOPTS` | доп. makeopts | `V=1` |
| `OPARYSH_PROFILE` | перекрыть активный профиль | `debug` |
| `OPARYSH_NO_COLORS` / `NO_COLOR` | отключить цветной вывод | |

Назначение переменных именно таково: oparysh **не** наследует `CFLAGS` из шелла напрямую (чтобы сборки были воспроизводимы); всё через OPARYSH_* или конфиг.

---

## Приложение A. Типичный сценарий

```sh
# 1. Настроить репозитории и тулчейн
oparysh repo add stable https://repo.example.com/stable

# 2. Собрать с нестандартными флагами
oparysh install nginx --set cflags="-O3 -march=native -flto" --set cc=clang --jobs 16

# 3. Проверить, что ничего не развалилось
oparysh check

# 4. Обновление
oparysh sync && oparysh upgrade && oparysh autoremove
```

## Приложение B. Сообщения о проблемах

При баг-репорте прикладывайте: `oparysh info pkg`, лог из `/var/lib/oparysh/logs/`, файл `oparysh.conf` без секретов, вывод `oparysh --version`.
