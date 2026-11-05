# oparysh

**oparysh** (*oparysh package manager*) — source-based пакетный менеджер на Rust.
Пакеты собираются локально из исходного кода: зависимости разрешаются автоматически,
флаги компиляции (`CFLAGS`, `MAKEOPTS`) и компилятор настраиваются, целостность
каждого артефакта проверяется по SHA-256.

```sh
oparysh sync && oparysh install nginx
```

---

## Почему из исходников?

- **Тюнинг под машину.** `-march=native`, `-O3 -flto`, свой компилятор (`clang`, `mold`) — на уровне одной команды или всего профиля.
- **Прозрачность.** Вы точно знаете, чем и с какими флагами собрано: всё зафиксировано в БД (`build_info`).
- **Контроль.** Никакого неявного кода из бинарных пакетов: источник проверяется по `sha256` (несовпадение = жирная ошибка, а не предупреждение).

Если бинарные пакеты — ваша тема, oparysh, скорее всего, не для вас. И это нормально.

---

## Возможности

| | |
|---|---|
| Автоматическое разрешение зависимостей | топосортировка, виртуальные пакеты (`provides`), проверка конфликтов |
| Локальные и HTTP(S)-репозитории | статический `index.toml`, приоритеты, опциональные подписи |
| Шесть сборочных систем | `autoconf-make`, `cmake-ninja`, `cargo`, `meson`, `make-only`, `custom` |
| Профили сборки | `release` / `debug` / `size` + свои; флаги склеиваются: профиль ⊕ конфиг ⊕ рецепт |
| Ручная перекрытия | `oparysh build nginx --set cflags="-O3 -flto" --set cc=clang --jobs 16` |
| Целостность | SHA-256 тарболов, патчей, рецептов **и** каждого установленного файла (`oparysh check`) |
| Изолированная сборка | псевдо-`DESTDIR`, фиксированное окружение, воспроизводимость |
| Аккуратное удаление | по манифесту файлов, с проверкой обратных зависимостей (`rdeps`), `autoremove` сирот |

---

## Состояние

🚧 В разработке. План реализации — в [ROADMAP.md](ROADMAP.md), детальная спецификация — в [DOCS.md](DOCS.md).
MVP (M1–M6: разбор рецептов → сборка → установка) в процессе.

---

## Быстрый старт (план)

```sh
# 1. Указать репозиторий
oparysh repo add stable https://repo.example.com/stable

# 2. Настроить сборку под себя: /etc/oparysh/oparysh.conf
#    профили CFLAGS, компилятор, число потоков (-j) и т.д.

# 3. Установить — с индивидуальными флагами и тулчейном
oparysh install nginx --set cflags="-O3 -march=native -flto" --set cc=clang --jobs 16

# 4. Проверить, что ничего не развалилось
oparysh check

# 5. Обновление
oparysh sync && oparysh upgrade && oparysh autoremove
```

---

## Как это работает

Сборка одного пакета:

1. **fetch** — тарбол из кэша или скачивается в кэш (`src/`)
2. **verify** — проверка sha256 тарбола, патчей и рецепта
3. **extract** — чистый каталог `build/<name>-<ver>/`
4. **prepare / compile / install** — с флагами из профиля/конфига/рецепта, `DESTDIR` всегда включён
5. **manifest scan** — обход DESTDIR: список файлов + sha256 каждого
6. **commit** — перенос в целевой корень, запись в SQLite (`packages`, `files`, `deps`, `build_info`)
7. **cleanup** — каталоги сборки удаляются (если не `--keep-build`)

Каждый шаг падает → сборка прерывается, лог падения — в `logs/`, каталог `build/` сохраняется для разбора.

---

## Основные команды

```sh
oparysh sync                 # обновить индексы репозиториев
oparysh search <pattern>     # поиск по имени/описанию/тегам
oparysh info <pkg>           # метаданные пакета
oparysh plan <pkg…>          # показать план сборки и запросить подтверждение
oparysh deps <pkg> [--tree]  # зависимости пакета
oparysh rdeps <pkg>          # обратные зависимости установленных пакетов
oparysh install <pkg…>       # разрешить → собрать → установить
oparysh remove <pkg…>        # удалить (отказ, если кто-то зависит)
oparysh upgrade              # обновить установленные пакеты
oparysh autoremove           # удалить сироты
oparysh build <pkg|dir>      # только собрать, не устанавливая
oparysh check                # аудит целостности всех пакетов
oparysh why <pkg>            # зачем пакет установлен
oparysh clean --all          # очистить кэши
```

Полный список — в [DOCS.md, раздел 10](DOCS.md#10-команды-cli).

---

## Конфигурация

`/etc/oparysh/oparysh.conf` — управление сборкой:

```toml
[toolchain]
cc  = "clang"
cxx = "clang++"

[build.active]
profile = "release"          # release / debug / size / свой

[profile.release]
cflags   = "-O2 -pipe -fstack-protector-strong"
cxxflags = "-O2 -pipe -fstack-protector-strong"
ldflags  = "-Wl,-O1"

[build.extra]
cflags = "-march=native"     # склеивается с профилем, не перезаписывает

[general]
jobs = 8
```

Флаги можно перекрыть разово, без правки конфига и рецепта:

```sh
oparysh build nginx --set cflags="-O3 -flto" --set cc=clang --jobs 16
```

oparysh **не** наследует `CFLAGS` из шелла — только через конфиг или `OPARYSH_*`-переменные,
чтобы сборки были воспроизводимы.

---

## Целостность

Проверяется вся цепочка:

```
тарбол исходников  →  патчи  →  рецепт из репо  →  тарбол из dist/  →  установленные файлы
```

Алгоритм — только SHA-256 (hex, lowercase). Нет `sha256` у источника → **отказ сборки**,
не предупреждение. Расхождение при `check` → таблица `removed | modified | new`.
oparysh никогда не «пересчитывает» хэши сам.

---

## Сборка из исходников этого проекта

```sh
git clone <repo-url> oparysh && cd oparysh
cargo build --release
cargo test                       # интеграционные тесты работают с fixtures-репозиториями
# бинарник: target/release/oparysh
```

Структура workspace — в [DOCS.md, раздел 2](DOCS.md#2-структура-репозитория-проекта):
отдельные crate'ы для resolver, fetcher, verifier, builder, installer, db, repo.

---

## Документация

- **[DOCS.md](DOCS.md)** — полная спецификация: схема БД, формат рецепта, формат репозитория, resolver, коды возврата
- **[ROADMAP.md](ROADMAP.md)** — план разработки с микрошагами и вехами M1–M9
- **`oparysh.conf.example`** — пример конфига (будет добавлен)

## Сообщения о проблемах

В баг-репорте прикладывайте: `oparysh info pkg`, лог из `/var/lib/oparysh/logs/`,
`oparysh.conf` без секретов и вывод `oparysh --version`.

## Лицензия

[MIT](LICENSE) © 2026 oparysh contributors
