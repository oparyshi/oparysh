//! Валидатор рецепта (`pkg.toml`) — шаг 1.2.
//!
//! Проверяет обязательные поля, отсутствием которых нельзя «предупредить»:
//! секции `[pkg]` и `[build]`, `sha256` источника. Отсутствие `sha256` —
//! «жирная ошибка» по DOCS §9, сборка не должна начинаться.

use crate::recipe::Recipe;

/// Набор известных сборочных систем (DOCS §5, `build.system`).
const KNOWN_BUILD_SYSTEMS: [&str; 6] = [
    "autoconf-make",
    "cmake-ninja",
    "cargo",
    "meson",
    "make-only",
    "custom",
];

/// Проверить рецепт. Возвращает список ошибок; пустой список = рецепт корректен.
pub fn validate_recipe(recipe: &Recipe) -> Vec<String> {
    let mut errors = Vec::new();

    // [pkg]
    if recipe.pkg.name.trim().is_empty() {
        errors.push("[pkg]: обязательное поле `name` пустое или отсутствует".into());
    }
    if recipe.pkg.version.trim().is_empty() {
        errors.push(format!(
            "[pkg]: обязательное поле `version` пустое или отсутствует (пакет `{}`)",
            recipe.pkg.name
        ));
    }

    // [source] — sha256 обязателен всегда: отсутствие = жирная ошибка.
    let source = &recipe.source;
    if source.sha256.is_none() {
        errors.push(
            "[source]: обязательное поле `sha256` отсутствует — источник без контрольной \
             суммы не допускается (ошибка, а не предупреждение)"
                .into(),
        );
    } else if let Some(sha) = &source.sha256 {
        if let Err(e) = check_sha256(sha) {
            errors.push(format!("[source]: `sha256`: {e}"));
        }
    }
    if source.url.is_none() && source.file.is_none() {
        errors.push("[source]: укажите `url` или `file` — что именно берём как источник".into());
    } else if source.url.is_some() && source.file.is_some() {
        errors.push(
            "[source]: `url` и `file` взаимоисключающие — укажите что-то одно".into(),
        );
    }

    // [[patches]] — sha256 обязателен у каждого патча (же логика, что для источника).
    for (i, patch) in recipe.patches.iter().enumerate() {
        if let Err(e) = check_sha256(&patch.sha256) {
            errors.push(format!("[[patches]] №{i} ({}): `sha256`: {e}", patch.file));
        }
    }

    // [build]
    if !KNOWN_BUILD_SYSTEMS.contains(&recipe.build.system.as_str()) {
        errors.push(format!(
            "[build]: неизвестная сборочная система `{}` (допустимо: {})",
            recipe.build.system,
            KNOWN_BUILD_SYSTEMS.join(", ")
        ));
    }
    if recipe.build.system == "custom" {
        let steps = &recipe.build.steps;
        if steps.prepare.is_none() && steps.compile.is_none() && steps.install.is_none() {
            errors.push(
                "[build]: `system = \"custom\"` требует заполнить `[build.steps]` \
                 (prepare/compile/install)"
                    .into(),
            );
        }
    }
    if recipe.build.install.prefix.trim().is_empty() {
        errors.push("[build.install]: обязательное поле `prefix` пустое".into());
    }

    errors
}

/// Валидный sha256: 64 hex-символа в нижнем регистре.
fn check_sha256(sha: &str) -> Result<(), String> {
    let valid = sha.len() == 64
        && sha
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if valid {
        Ok(())
    } else if sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
        Err(format!(
            "ожидается нижний регистр hex (64 символа), получен `{sha}`"
        ))
    } else {
        Err(format!("ожидается 64 hex-символа SHA-256, получен `{sha}`"))
    }
}

/// Проверить рецепт и получить одну строку со всеми ошибками.
pub fn validate(recipe: &Recipe) -> Result<(), String> {
    let errors = validate_recipe(recipe);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "рецепт `{}` не прошёл валидацию:\n  {}",
            recipe.pkg.name,
            errors.join("\n  ")
        ))
    }
}
