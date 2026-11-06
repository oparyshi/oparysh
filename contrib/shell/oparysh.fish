# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_oparysh_global_optspecs
    string join \n config= db-dir= cache-dir= root= jobs= v/verbose yes h/help V/version
end

function __fish_oparysh_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_oparysh_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_oparysh_using_subcommand
    set -l cmd (__fish_oparysh_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c oparysh -n "__fish_oparysh_needs_command" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_needs_command" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_needs_command" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_needs_command" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_needs_command" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_needs_command" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_needs_command" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_needs_command" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_needs_command" -s V -l version -d 'Print version'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "check" -d 'Сверить установленные файлы с БД (sha256): removed | modified | new (этап 11.1/11.6)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "verify" -d 'Verify a tarball\'s SHA-256 (stage 2)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "rebuild-db" -d 'Восстановить базу данных из резервной копии db.sqlite.bak (stage 3.6)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "sync" -d 'Скачать/обновить index.toml всех (или указанных) репозиториев (stage 4.4)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "search" -d 'Поиск по имени/описанию/тегам (glob или /regex/) (stage 4.6)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "info" -d 'Показать метаданные пакета: зависимости, provides, версию, репозиторий (stage 4.7)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "list" -d 'Список установленных/доступных пакетов (stage 4.7)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "plan" -d 'Показать план разрешения зависимостей и запросить подтверждение (stage 5.7)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "deps" -d 'Дерево или список зависимостей пакета (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "rdeps" -d 'Обратные зависимости среди установленных пакетов (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "why" -d 'Объяснить, зачем установлен пакет (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "build" -d 'Собрать пакет или рецепт из каталога, БЕЗ установки (stage 7.8)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "install" -d 'Разрешить зависимости, собрать и установить цели + build-зависимости (stage 8.6)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "remove" -d 'Удалить установленные пакеты по манифесту файлов (stage 9.1–9.4)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "autoremove" -d 'Удалить пакеты-«сироты»: стояли как зависимости, никому не нужны (stage 9.3)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "upgrade" -d 'Обновить все или указанные установленные пакеты (stage 10.1)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "downgrade" -d 'Откат пакета на конкретную версию из репо или кэша рецептов (stage 10.2)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "history" -d 'Журнал установок/обновлений/понижений/удалений (stage 10.3)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "repo" -d 'Управление репозиториями в конфиге (stage 4.5)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "clean" -d 'Очистить кэши исходников/сборки/логов (stage 6.6)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "completions" -d 'Генерировать скрипты автодополнения для оболочки (bash, zsh, fish, powershell)'
complete -c oparysh -n "__fish_oparysh_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand check" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -s e -l expected -d 'Ожидаемый SHA-256 (hex). Если не указан — вычисляется и печатается' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand verify" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l db-dir -d 'Директория базы (по умолчанию — из конфига/OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand rebuild-db" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand sync" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand search" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand info" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l installed -d 'Только установленные пакеты (по умолчанию)'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l available -d 'Все пакеты из репозиториев'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l outdated -d 'Установленные пакеты с более свежей версией в репозитории'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l orphaned -d 'Установленные пакеты, от которых никто не зависит'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand list" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand plan" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l tree -d 'Печатать дерево вместо плоского списка'
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand deps" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand rdeps" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand why" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l set -d 'Перекрытия флагов на одну сборку: `--set cflags=… --set cc=…` (stage 7.7)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l jobs -d 'Перекрыть число заданий сборки (stage 7.7)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l install -d 'Установить пакет после успешной сборки (stage 8.1–8.4): manifest scan → перенос в destroot → запись в БД'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l strict -d 'Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l keep-build -d 'Не удалять каталог сборки <db_dir>/build/ после установки'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand build" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l file -d 'Установить из локального рецепта, минуя репозиторий (stage 8.5)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l set -d 'Перекрытия флагов на одну сборку: `--set cflags=… --set cc=…`' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l jobs -d 'Перекрыть число заданий сборки' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l strict -d 'Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l keep-build -d 'Не удалять каталог сборки <db_dir>/build/ после установки'
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand install" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l force -d 'Удалить даже при установленных обратных зависимостях (код 9 без флага)'
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand remove" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand autoremove" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l set -d 'Перекрытия флагов на одну сборку: `--set cflags=… --set cc=…`' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l jobs -d 'Перекрыть число заданий сборки' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l strict -d 'Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l keep-build -d 'Не удалять каталог сборки <db_dir>/build/ после установки'
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand upgrade" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l set -d 'Перекрытия флагов на одну сборку: `--set cflags=… --set cc=…`' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l jobs -d 'Перекрыть число заданий сборки' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l strict -d 'Незаявленные файлы (вне install.prefix) = ошибка (stage 8.2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l keep-build -d 'Не удалять каталог сборки <db_dir>/build/ после установки'
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand downgrade" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand history" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -f -a "add" -d 'Добавить репозиторий в конфиг (url или путь к локальному каталогу)'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -f -a "list" -d 'Показать репозитории, кол-во пакетов, дату обновления'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -f -a "del" -d 'Убрать репозиторий из конфига'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and not __fish_seen_subcommand_from add list del help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from del" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from help" -f -a "add" -d 'Добавить репозиторий в конфиг (url или путь к локальному каталогу)'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from help" -f -a "list" -d 'Показать репозитории, кол-во пакетов, дату обновления'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from help" -f -a "del" -d 'Убрать репозиторий из конфига'
complete -c oparysh -n "__fish_oparysh_using_subcommand repo; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l src -d 'Очистить кэш исходников <db_dir>/src/'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l build -d 'Очистить рабочие каталоги сборки <db_dir>/build/'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l logs -d 'Очистить логи сборок <db_dir>/logs/'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l all -d 'Очистить всё: src + build + logs'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand clean" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -s o -l output -d 'Путь для вывода (по умолчанию stdout)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l config -d 'Путь к конфигу (по умолчанию /etc/oparysh/oparysh.conf или $OPARYSH_CONFIG)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l db-dir -d 'Перекрыть `db_dir` (как OPARYSH_DB_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l cache-dir -d 'Перекрыть `cache_dir` (как OPARYSH_CACHE_DIR)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l root -d 'Установить в другой корень — chroot-установка (как OPARYSH_ROOT)' -r -F
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l jobs -d 'Перекрыть число заданий сборки (как OPARYSH_JOBS)' -r
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -s v -l verbose -d 'Подробная диагностика в stderr'
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -l yes -d 'Не спрашивать подтверждение у команд, которые его поддерживают'
complete -c oparysh -n "__fish_oparysh_using_subcommand completions" -s h -l help -d 'Print help'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "check" -d 'Сверить установленные файлы с БД (sha256): removed | modified | new (этап 11.1/11.6)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "verify" -d 'Verify a tarball\'s SHA-256 (stage 2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "rebuild-db" -d 'Восстановить базу данных из резервной копии db.sqlite.bak (stage 3.6)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "sync" -d 'Скачать/обновить index.toml всех (или указанных) репозиториев (stage 4.4)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "search" -d 'Поиск по имени/описанию/тегам (glob или /regex/) (stage 4.6)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "info" -d 'Показать метаданные пакета: зависимости, provides, версию, репозиторий (stage 4.7)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "list" -d 'Список установленных/доступных пакетов (stage 4.7)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "plan" -d 'Показать план разрешения зависимостей и запросить подтверждение (stage 5.7)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "deps" -d 'Дерево или список зависимостей пакета (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "rdeps" -d 'Обратные зависимости среди установленных пакетов (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "why" -d 'Объяснить, зачем установлен пакет (stage 5.8)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "build" -d 'Собрать пакет или рецепт из каталога, БЕЗ установки (stage 7.8)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "install" -d 'Разрешить зависимости, собрать и установить цели + build-зависимости (stage 8.6)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "remove" -d 'Удалить установленные пакеты по манифесту файлов (stage 9.1–9.4)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "autoremove" -d 'Удалить пакеты-«сироты»: стояли как зависимости, никому не нужны (stage 9.3)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "upgrade" -d 'Обновить все или указанные установленные пакеты (stage 10.1)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "downgrade" -d 'Откат пакета на конкретную версию из репо или кэша рецептов (stage 10.2)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "history" -d 'Журнал установок/обновлений/понижений/удалений (stage 10.3)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "repo" -d 'Управление репозиториями в конфиге (stage 4.5)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "clean" -d 'Очистить кэши исходников/сборки/логов (stage 6.6)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "completions" -d 'Генерировать скрипты автодополнения для оболочки (bash, zsh, fish, powershell)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and not __fish_seen_subcommand_from check verify rebuild-db sync search info list plan deps rdeps why build install remove autoremove upgrade downgrade history repo clean completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and __fish_seen_subcommand_from repo" -f -a "add" -d 'Добавить репозиторий в конфиг (url или путь к локальному каталогу)'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and __fish_seen_subcommand_from repo" -f -a "list" -d 'Показать репозитории, кол-во пакетов, дату обновления'
complete -c oparysh -n "__fish_oparysh_using_subcommand help; and __fish_seen_subcommand_from repo" -f -a "del" -d 'Убрать репозиторий из конфига'
