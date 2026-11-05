//! Топологическая сортировка графа зависимостей (этап 5.6 ROADMAP,
//! DOCS §7 п.6).
//!
//! Порядок — «зависимости раньше зависящих» (порядок сборки). Используется
//! алгоритм Кана; при runtime-цикле оставшиеся узлы дописываются в
//! детерминированном порядке (runtime-цикл разрешён, build-цикл отсекается
//! на этапе сбора).

use std::collections::BTreeMap;

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

/// Порядок сборки только для узлов, которые нужно собрать
/// (установленные пропускаются).
pub fn pending_build_order(set: &RequirementSet) -> Vec<String> {
    build_order(set)
        .into_iter()
        .filter(|name| !set.nodes[name].installed)
        .collect()
}
