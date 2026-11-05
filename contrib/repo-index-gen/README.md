# repo-index-gen

Генератор `index.toml` по каталогу репозитория oparysh (основа будущей команды `oparysh repo gen-index`).

## Ожидаемая структура

```text
<root>/
├── recipes/<name>/<ver>/pkg.toml
└── dist/<name>/<ver>/<tarball>   # ровно один файл
```

## Использование

```sh
cargo run -p oparysh-repo-index-gen -- <dir> --name stable \
    --origin https://repo.example.com/stable [--output path] [--updated <unix-secs>]
```

По умолчанию записывает `<dir>/index.toml`. `recipe_sha` и `src_sha256`
вычисляются как sha256 соответствующих файлов. `deps` в индексе = `run` + `build`
(дубли убраны); `provides`/`conflicts` зеркалят рецепт. `tags` в рецепте
отсутствуют и в индекс не попадают (всегда пустой список).
