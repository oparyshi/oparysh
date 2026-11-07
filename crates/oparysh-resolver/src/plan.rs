//! Топологическая сортировка графа зависимостей (этап 5.6 ROADMAP,
//! DOCS §7 п.6).
//!
//! Порядок — «зависимости раньше зависящих» (порядок сборки). Используется
//! алгоритм Кана; при runtime-цикле оставшиеся узлы дописываются в
//! детерминированном порядке (runtime-цикл разрешён, build-цикл отсекается
//! на этапе сбора).

use std::collections::BTreeMap;
use std::fmt::Write;

use oparysh_core::i18n::{t_lang, Lang};

use crate::collect::RequirementSet;

/// Порядок сборки: имена узлов, зависимости идут раньше зависящих.
///
/// Установленные узлы (`installed = true`) включаются в результат, но
/// помечены — вызывающая сторона обычно пропускает их при выполнении.
pub fn build_order(set: &RequirementSet) -> Vec<String> {
    // in-degree = число зависимостей узла внутри множества.
    let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, node) in &set.nodes {
        let deps: Vec<&str> = node
            .dependencies
            .iter()
            .filter(|d| set.nodes.contains_key(d.as_str()))
            .map(String::as_str)
            .collect();
        in_degree.insert(name.as_str(), deps.len());
        for dep in deps {
            dependents.entry(dep).or_default().push(name.as_str());
        }
    }

    // Детерминизм: ready-множество как BTreeSet по имени.
    let mut ready: std::collections::BTreeSet<&str> = in_degree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(&n, _)| n)
        .collect();

    let mut order = Vec::with_capacity(set.nodes.len());
    while let Some(&node) = ready.iter().next() {
        ready.remove(node);
        order.push(node.to_string());
        if let Some(children) = dependents.get(node) {
            for &child in children {
                let d = in_degree.get_mut(child).expect("ребёнок существует");
                *d -= 1;
                if *d == 0 {
                    ready.insert(child);
                }
            }
        }
    }

    // Runtime-цикл: дописать оставшиеся узлы детерминированно.
    if order.len() != set.nodes.len() {
        let mut rest: Vec<&str> = set
            .nodes
            .keys()
            .map(String::as_str)
            .filter(|n| !order.iter().any(|o| o == n))
            .collect();
        rest.sort_unstable();
        order.extend(rest.into_iter().map(str::to_string));
    }

    order
}

/// Человекочитаемый размер для плана.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Построить текст плана (DOCS §7 п.6).
///
/// `sizes` — размер тарбола для каждого ожидающего сборки узла; `None`
/// означает «ещё неизвестен» и не включается в сумму. Возвращает текст,
/// который можно напечатать перед подтверждением.
pub fn render_plan(
    set: &RequirementSet,
    jobs: u32,
    sizes: &BTreeMap<String, Option<u64>>,
    lang: Lang,
) -> String {
    let pending = pending_build_order(set);
    let count = pending.len();
    let installed = set.nodes.values().filter(|n| n.installed).count();
    let mut out = String::new();
    let ru = lang.is_ru();
    let _ = writeln!(
        out,
        "{}",
        t_lang(lang, 
            &format!(
                "Будет собрано пакетов: {count}, уже установлено: {installed}"
            ),
            &format!(
                "The following {count} package(s) will be BUILT, {installed} already INSTALLED:"
            )
        )
    );
    if pending.is_empty() {
        let _ = writeln!(out, "  {}", t_lang(lang, "(ничего собирать не нужно)", "(nothing to build, all installed)"));
    } else {
        let _ = writeln!(out);
        for (index, name) in pending.iter().enumerate() {
            let node = &set.nodes[name];
            let reason = if node.is_target {
                t_lang(lang, "цель", "target")
            } else {
                let required_by = node
                    .required_by
                    .iter()
                    .filter_map(|(parent, _)| parent.as_deref())
                    .next()
                    .unwrap_or(if ru { "зависимость" } else { "dependency" });
                if ru {
                    format!("зависимость от {required_by}")
                } else {
                    format!("dep of {required_by}")
                }
            };
            let virtual_note = node
                .provided_for
                .as_deref()
                .map(|name| format!(", provides {name}"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  #{:<2} {}  {}  ({reason}{virtual_note})",
                index + 1,
                node.name,
                node.version_str
            );
        }
    }

    let total_known: u64 = pending
        .iter()
        .filter_map(|name| sizes.get(name).copied().flatten())
        .sum();
    let unknown = pending.iter().any(|name| sizes.get(name).copied().flatten().is_none());
    let total = if unknown {
        format!(
            "{}{}",
            format_bytes(total_known),
            t_lang(lang, "+ (некоторые размеры неизвестны)", "+ (some sizes unknown)")
        )
    } else {
        format_bytes(total_known)
    };
    let _ = writeln!(out);
    let _ = write!(out, "{}", t_lang(lang, &format!("К загрузке: {total}, заданий сборки: {jobs}."), &format!("Total DL: {total}, build jobs: {jobs}.")));
    out
}

/// Порядок сборки только для узлов, которые нужно собрать
/// (установленные пропускаются).
pub fn pending_build_order(set: &RequirementSet) -> Vec<String> {
    build_order(set)
        .into_iter()
        .filter(|name| !set.nodes[name].installed)
        .collect()
}
