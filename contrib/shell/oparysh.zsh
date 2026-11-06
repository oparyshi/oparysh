#compdef oparysh

autoload -U is-at-least

_oparysh() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_oparysh_commands" \
"*::: :->oparysh" \
&& ret=0
    case $state in
    (oparysh)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:oparysh-command-$line[1]:"
        case $line[1] in
            (check)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::pkgs -- Пакеты для проверки; пусто — все установленные (полный аудит):_default' \
&& ret=0
;;
(verify)
_arguments "${_arguments_options[@]}" : \
'-e+[Ожидаемый SHA-256 (hex). Если не указан — вычисляется и печатается]:EXPECTED:_default' \
'--expected=[Ожидаемый SHA-256 (hex). Если не указан — вычисляется и печатается]:EXPECTED:_default' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':tarball -- Путь к тарболу для проверки:_files' \
&& ret=0
;;
(rebuild-db)
_arguments "${_arguments_options[@]}" : \
'--db-dir=[Директория базы (по умолчанию — из конфига/OPARYSH_DB_DIR)]:PATH:_files' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(sync)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::repos -- Ограничить синхронизацию перечисленными репозиториями:_default' \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pattern -- Шаблон\: glob (`gcc*`, `*-git`) или регулярное выражение в слэшах (`/^lib.*/`):_default' \
&& ret=0
;;
(info)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pkg -- Пакет в виде `pkg` или `pkg@ver`:_default' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'--installed[Только установленные пакеты (по умолчанию)]' \
'--available[Все пакеты из репозиториев]' \
'--outdated[Установленные пакеты с более свежей версией в репозитории]' \
'--orphaned[Установленные пакеты, от которых никто не зависит]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(plan)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::specs -- Цели\: `pkg`, `pkg@ver`, необязательные зависимости через `+feature`:_default' \
&& ret=0
;;
(deps)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'--tree[Печатать дерево вместо плоского списка]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pkg -- Пакет в виде `pkg` или `pkg@ver`:_default' \
&& ret=0
;;
(rdeps)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pkg -- Пакет в виде `pkg` или `pkg@ver`:_default' \
&& ret=0
;;
(why)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pkg -- Пакет в виде `pkg` или `pkg@ver`:_default' \
&& ret=0
;;
(build)
_arguments "${_arguments_options[@]}" : \
'*--set=[Перекрытия флагов на одну сборку\: \`--set cflags=… --set cc=…\` (stage 7.7)]:K=V:_default' \
'--jobs=[Перекрыть число заданий сборки (stage 7.7)]:N:_default' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--install[Установить пакет после успешной сборки (stage 8.1–8.4)\: manifest scan → перенос в destroot → запись в БД]' \
'--strict[Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)]' \
'--keep-build[Не удалять каталог сборки <db_dir>/build/ после установки]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':target -- Пакет (`pkg`, `pkg@ver`) или каталог с рецептом (`pkg.toml`):_default' \
&& ret=0
;;
(install)
_arguments "${_arguments_options[@]}" : \
'--file=[Установить из локального рецепта, минуя репозиторий (stage 8.5)]:PKG_TOML:_files' \
'*--set=[Перекрытия флагов на одну сборку\: \`--set cflags=… --set cc=…\`]:K=V:_default' \
'--jobs=[Перекрыть число заданий сборки]:N:_default' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--strict[Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)]' \
'--keep-build[Не удалять каталог сборки <db_dir>/build/ после установки]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::specs -- Цели\: `pkg`, `pkg@ver`, необязательные зависимости через `+feature`; не требуется при `--file`:_default' \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'--force[Удалить даже при установленных обратных зависимостях (код 9 без флага)]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::pkgs -- Пакеты для удаления:_default' \
&& ret=0
;;
(autoremove)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(upgrade)
_arguments "${_arguments_options[@]}" : \
'*--set=[Перекрытия флагов на одну сборку\: \`--set cflags=… --set cc=…\`]:K=V:_default' \
'--jobs=[Перекрыть число заданий сборки]:N:_default' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--strict[Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)]' \
'--keep-build[Не удалять каталог сборки <db_dir>/build/ после установки]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'*::pkgs -- Пакеты для обновления; пусто — все установленные:_default' \
&& ret=0
;;
(downgrade)
_arguments "${_arguments_options[@]}" : \
'*--set=[Перекрытия флагов на одну сборку\: \`--set cflags=… --set cc=…\`]:K=V:_default' \
'--jobs=[Перекрыть число заданий сборки]:N:_default' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--strict[Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)]' \
'--keep-build[Не удалять каталог сборки <db_dir>/build/ после установки]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':pkg -- Пакет для отката:_default' \
':ver -- Целевая версия:_default' \
&& ret=0
;;
(history)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
'::pkg -- Ограничить вывод одним пакетом:_default' \
&& ret=0
;;
(repo)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
":: :_oparysh__subcmd__repo_commands" \
"*::: :->repo" \
&& ret=0

    case $state in
    (repo)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:oparysh-repo-command-$line[1]:"
        case $line[1] in
            (add)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':name -- Имя репозитория:_default' \
':target -- URL (https\://…) или путь к локальному каталогу-репозиторию:_default' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(del)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':name -- Имя репозитория:_default' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_oparysh__subcmd__repo__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:oparysh-repo-help-command-$line[1]:"
        case $line[1] in
            (add)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(del)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(clean)
_arguments "${_arguments_options[@]}" : \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'--src[Очистить кэш исходников <db_dir>/src/]' \
'--build[Очистить рабочие каталоги сборки <db_dir>/build/]' \
'--logs[Очистить логи сборок <db_dir>/logs/]' \
'--all[Очистить всё\: src + build + logs]' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" : \
'-o+[Путь для вывода (по умолчанию stdout)]:PATH:_files' \
'--output=[Путь для вывода (по умолчанию stdout)]:PATH:_files' \
'--config=[Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или \$OPARYSH_CONFIG)]:PATH:_files' \
'--db-dir=[Перекрыть \`db_dir\` (как OPARYSH_DB_DIR)]:PATH:_files' \
'--cache-dir=[Перекрыть \`cache_dir\` (как OPARYSH_CACHE_DIR)]:PATH:_files' \
'--root=[Установить в другой корень — chroot-установка (как OPARYSH_ROOT)]:PATH:_files' \
'--jobs=[Перекрыть число заданий сборки (как OPARYSH_JOBS)]:N:_default' \
'-v[Подробная диагностика в stderr]' \
'--verbose[Подробная диагностика в stderr]' \
'--yes[Не спрашивать подтверждение у команд, которые его поддерживают]' \
'-h[Print help]' \
'--help[Print help]' \
':shell -- Shell\: bash, zsh, fish, powershell:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_oparysh__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:oparysh-help-command-$line[1]:"
        case $line[1] in
            (check)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(verify)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(rebuild-db)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(sync)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(info)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(plan)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(deps)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(rdeps)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(why)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(build)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(install)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(autoremove)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(upgrade)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(downgrade)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(history)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(repo)
_arguments "${_arguments_options[@]}" : \
":: :_oparysh__subcmd__help__subcmd__repo_commands" \
"*::: :->repo" \
&& ret=0

    case $state in
    (repo)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:oparysh-help-repo-command-$line[1]:"
        case $line[1] in
            (add)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(del)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
(clean)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_oparysh_commands] )) ||
_oparysh_commands() {
    local commands; commands=(
'check:Сверить установленные файлы с БД (sha256)\: removed | modified | new (этап 11.1/11.6)' \
'verify:Verify a tarball'\''s SHA-256 (stage 2)' \
'rebuild-db:Восстановить базу данных из резервной копии db.sqlite.bak (stage 3.6)' \
'sync:Скачать/обновить index.toml всех (или указанных) репозиториев (stage 4.4)' \
'search:Поиск по имени/описанию/тегам (glob или /regex/) (stage 4.6)' \
'info:Показать метаданные пакета\: зависимости, provides, версию, репозиторий (stage 4.7)' \
'list:Список установленных/доступных пакетов (stage 4.7)' \
'plan:Показать план разрешения зависимостей и запросить подтверждение (stage 5.7)' \
'deps:Дерево или список зависимостей пакета (stage 5.8)' \
'rdeps:Обратные зависимости среди установленных пакетов (stage 5.8)' \
'why:Объяснить, зачем установлен пакет (stage 5.8)' \
'build:Собрать пакет или рецепт из каталога, БЕЗ установки (stage 7.8)' \
'install:Разрешить зависимости, собрать и установить цели + build-зависимости (stage 8.6)' \
'remove:Удалить установленные пакеты по манифесту файлов (stage 9.1–9.4)' \
'autoremove:Удалить пакеты-«сироты»\: стояли как зависимости, никому не нужны (stage 9.3)' \
'upgrade:Обновить все или указанные установленные пакеты (stage 10.1)' \
'downgrade:Откат пакета на конкретную версию из репо или кэша рецептов (stage 10.2)' \
'history:Журнал установок/обновлений/понижений/удалений (stage 10.3)' \
'repo:Управление репозиториями в конфиге (stage 4.5)' \
'clean:Очистить кэши исходников/сборки/логов (stage 6.6)' \
'completions:Генерировать скрипты автодополнения для оболочки (bash, zsh, fish, powershell)' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'oparysh commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__autoremove_commands] )) ||
_oparysh__subcmd__autoremove_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh autoremove commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__build_commands] )) ||
_oparysh__subcmd__build_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh build commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__check_commands] )) ||
_oparysh__subcmd__check_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh check commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__clean_commands] )) ||
_oparysh__subcmd__clean_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh clean commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__completions_commands] )) ||
_oparysh__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh completions commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__deps_commands] )) ||
_oparysh__subcmd__deps_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh deps commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__downgrade_commands] )) ||
_oparysh__subcmd__downgrade_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh downgrade commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help_commands] )) ||
_oparysh__subcmd__help_commands() {
    local commands; commands=(
'check:Сверить установленные файлы с БД (sha256)\: removed | modified | new (этап 11.1/11.6)' \
'verify:Verify a tarball'\''s SHA-256 (stage 2)' \
'rebuild-db:Восстановить базу данных из резервной копии db.sqlite.bak (stage 3.6)' \
'sync:Скачать/обновить index.toml всех (или указанных) репозиториев (stage 4.4)' \
'search:Поиск по имени/описанию/тегам (glob или /regex/) (stage 4.6)' \
'info:Показать метаданные пакета\: зависимости, provides, версию, репозиторий (stage 4.7)' \
'list:Список установленных/доступных пакетов (stage 4.7)' \
'plan:Показать план разрешения зависимостей и запросить подтверждение (stage 5.7)' \
'deps:Дерево или список зависимостей пакета (stage 5.8)' \
'rdeps:Обратные зависимости среди установленных пакетов (stage 5.8)' \
'why:Объяснить, зачем установлен пакет (stage 5.8)' \
'build:Собрать пакет или рецепт из каталога, БЕЗ установки (stage 7.8)' \
'install:Разрешить зависимости, собрать и установить цели + build-зависимости (stage 8.6)' \
'remove:Удалить установленные пакеты по манифесту файлов (stage 9.1–9.4)' \
'autoremove:Удалить пакеты-«сироты»\: стояли как зависимости, никому не нужны (stage 9.3)' \
'upgrade:Обновить все или указанные установленные пакеты (stage 10.1)' \
'downgrade:Откат пакета на конкретную версию из репо или кэша рецептов (stage 10.2)' \
'history:Журнал установок/обновлений/понижений/удалений (stage 10.3)' \
'repo:Управление репозиториями в конфиге (stage 4.5)' \
'clean:Очистить кэши исходников/сборки/логов (stage 6.6)' \
'completions:Генерировать скрипты автодополнения для оболочки (bash, zsh, fish, powershell)' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'oparysh help commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__autoremove_commands] )) ||
_oparysh__subcmd__help__subcmd__autoremove_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help autoremove commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__build_commands] )) ||
_oparysh__subcmd__help__subcmd__build_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help build commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__check_commands] )) ||
_oparysh__subcmd__help__subcmd__check_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help check commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__clean_commands] )) ||
_oparysh__subcmd__help__subcmd__clean_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help clean commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__completions_commands] )) ||
_oparysh__subcmd__help__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help completions commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__deps_commands] )) ||
_oparysh__subcmd__help__subcmd__deps_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help deps commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__downgrade_commands] )) ||
_oparysh__subcmd__help__subcmd__downgrade_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help downgrade commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__help_commands] )) ||
_oparysh__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help help commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__history_commands] )) ||
_oparysh__subcmd__help__subcmd__history_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help history commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__info_commands] )) ||
_oparysh__subcmd__help__subcmd__info_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help info commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__install_commands] )) ||
_oparysh__subcmd__help__subcmd__install_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help install commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__list_commands] )) ||
_oparysh__subcmd__help__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help list commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__plan_commands] )) ||
_oparysh__subcmd__help__subcmd__plan_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help plan commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__rdeps_commands] )) ||
_oparysh__subcmd__help__subcmd__rdeps_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help rdeps commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__rebuild-db_commands] )) ||
_oparysh__subcmd__help__subcmd__rebuild-db_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help rebuild-db commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__remove_commands] )) ||
_oparysh__subcmd__help__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help remove commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__repo_commands] )) ||
_oparysh__subcmd__help__subcmd__repo_commands() {
    local commands; commands=(
'add:Добавить репозиторий в конфиг (url или путь к локальному каталогу)' \
'list:Показать репозитории, кол-во пакетов, дату обновления' \
'del:Убрать репозиторий из конфига' \
    )
    _describe -t commands 'oparysh help repo commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__repo__subcmd__add_commands] )) ||
_oparysh__subcmd__help__subcmd__repo__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help repo add commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__repo__subcmd__del_commands] )) ||
_oparysh__subcmd__help__subcmd__repo__subcmd__del_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help repo del commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__repo__subcmd__list_commands] )) ||
_oparysh__subcmd__help__subcmd__repo__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help repo list commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__search_commands] )) ||
_oparysh__subcmd__help__subcmd__search_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help search commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__sync_commands] )) ||
_oparysh__subcmd__help__subcmd__sync_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help sync commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__upgrade_commands] )) ||
_oparysh__subcmd__help__subcmd__upgrade_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help upgrade commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__verify_commands] )) ||
_oparysh__subcmd__help__subcmd__verify_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help verify commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__help__subcmd__why_commands] )) ||
_oparysh__subcmd__help__subcmd__why_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh help why commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__history_commands] )) ||
_oparysh__subcmd__history_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh history commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__info_commands] )) ||
_oparysh__subcmd__info_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh info commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__install_commands] )) ||
_oparysh__subcmd__install_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh install commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__list_commands] )) ||
_oparysh__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh list commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__plan_commands] )) ||
_oparysh__subcmd__plan_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh plan commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__rdeps_commands] )) ||
_oparysh__subcmd__rdeps_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh rdeps commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__rebuild-db_commands] )) ||
_oparysh__subcmd__rebuild-db_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh rebuild-db commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__remove_commands] )) ||
_oparysh__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh remove commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo_commands] )) ||
_oparysh__subcmd__repo_commands() {
    local commands; commands=(
'add:Добавить репозиторий в конфиг (url или путь к локальному каталогу)' \
'list:Показать репозитории, кол-во пакетов, дату обновления' \
'del:Убрать репозиторий из конфига' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'oparysh repo commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__add_commands] )) ||
_oparysh__subcmd__repo__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo add commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__del_commands] )) ||
_oparysh__subcmd__repo__subcmd__del_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo del commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__help_commands] )) ||
_oparysh__subcmd__repo__subcmd__help_commands() {
    local commands; commands=(
'add:Добавить репозиторий в конфиг (url или путь к локальному каталогу)' \
'list:Показать репозитории, кол-во пакетов, дату обновления' \
'del:Убрать репозиторий из конфига' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'oparysh repo help commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__help__subcmd__add_commands] )) ||
_oparysh__subcmd__repo__subcmd__help__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo help add commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__help__subcmd__del_commands] )) ||
_oparysh__subcmd__repo__subcmd__help__subcmd__del_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo help del commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__help__subcmd__help_commands] )) ||
_oparysh__subcmd__repo__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo help help commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__help__subcmd__list_commands] )) ||
_oparysh__subcmd__repo__subcmd__help__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo help list commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__repo__subcmd__list_commands] )) ||
_oparysh__subcmd__repo__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh repo list commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__search_commands] )) ||
_oparysh__subcmd__search_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh search commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__sync_commands] )) ||
_oparysh__subcmd__sync_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh sync commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__upgrade_commands] )) ||
_oparysh__subcmd__upgrade_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh upgrade commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__verify_commands] )) ||
_oparysh__subcmd__verify_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh verify commands' commands "$@"
}
(( $+functions[_oparysh__subcmd__why_commands] )) ||
_oparysh__subcmd__why_commands() {
    local commands; commands=()
    _describe -t commands 'oparysh why commands' commands "$@"
}

if [ "$funcstack[1]" = "_oparysh" ]; then
    _oparysh "$@"
else
    compdef _oparysh oparysh
fi
