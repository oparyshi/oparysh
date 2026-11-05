//! Сбор требований (этап 5 ROADMAP, DOCS §7 п.1–5).
//!
//! Начинаем с целей (`oparysh install nginx pcre2>=10.32`) и рекурсивно
//! раскрываем `deps.build` (нужны только при сборке) и `deps.run` (нужны
//! в рантайме). Опциональные зависимости (`deps.optional`) включаются,
//! если включена соответствующая фича (`oparysh install nginx +tls`).
//!
//! - **5.2** — если установленный пакет удовлетворяет требованию, узел
//!   помечается `installed = true` и его зависимости не раскрываются;
//! - **5.3** — стратегия `latest-satisfying`: первый по приоритету
//!   репозиторий, в котором есть подходящая версия; внутри него —
//!   максимальная версия (в т.ч. `build`-ревизия);
//! - **5.4** — если пакета с запрошенным именем нет, выполняется поиск
//!   поставщика по `provides.virtual`; при нескольких поставщиках выбирается
//!   установленный, иначе — `options.provider`, иначе ошибка
//!   [`ResolveError::AmbiguousProvider`] (CLI предлагает выбор интерактивно);
//! - **5.5** — проверка `conflicts` и обнаружение циклов: runtime-цикл
//!   разрешён, build-цикл — ошибка [`ResolveError::BuildCycle`].
//!
//! Топологическая сортировка — в [`crate::plan`] (этап 5.6).

use std::collections::{BTreeMap, VecDeque};

use oparysh_core::recipe::Recipe;
use oparysh_core::version::{DepReq, Version};

use crate::error::{ResolveError, ResolveResult};
use crate::installed::InstalledSource;

/// Тип зависимости по секции рецепта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DepKind {
    /// `deps.build` — нужна только для сборки.
    Build,
    /// `deps.run` — нужна в рантайме.
    Run,
}

impl std::fmt::Display for DepKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepKind::Build => write!(f, "build"),
            DepKind::Run => write!(f, "run"),
        }
    }
}

/// Кандидат на установку из одного из репозиториев.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Имя пакета.
    pub name: String,
    /// Разобранная версия (для сравнения и выбора).
    pub version: Version,
    /// Исходная строка версии (пути рецептов, сообщения).
    pub version_str: String,
    /// Ревизия сборки при той же апстрим-версии.
    pub build: u32,
    /// Имя репозитория, которому принадлежит запись (приоритет — порядок возврата).
    pub repo: String,
}

/// Источник пакетов для резолвера.
///
/// Реализация обязана возвращать кандидатов **в порядке приоритета
/// репозиториев** (первым — самый приоритетный). Локальная реализация —
/// [`crate::repo_source::RepoPackageSource`]; тесты используют поддельный
/// источник в памяти.
pub trait PackageSource {
    /// Все кандидаты с данным именем, в порядке приоритета репозиториев.
    fn candidates(&self, name: &str) -> ResolveResult<Vec<Candidate>>;
    /// Рецепт конкретного кандидата (проверки хэшей — на стороне реализации).
    fn recipe(&self, candidate: &Candidate) -> ResolveResult<Recipe>;
    /// Кандидаты-поставщики виртуального пакета (`provides.virtual`).
    /// По умолчанию — пусто.
    fn providers(&self, _virtual_name: &str) -> ResolveResult<Vec<Candidate>> {
        Ok(Vec::new())
    }
}

/// Опции разрешения зависимостей.
#[derive(Default)]
pub struct ResolveOptions<'a> {
    /// Включённые фичи (`oparysh install nginx +tls`).
    pub features: Vec<String>,
    /// Источник сведений об установленных пакетах (этап 5.2). Без него
    /// установленные пакеты не учитываются.
    pub installed: Option<&'a dyn InstalledSource>,
    /// Предпочтительный поставщик виртуального пакета (выбор пользователя).
    pub provider: Option<String>,
}

/// Ребро графа зависимостей.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepEdge {
    /// Кто требует (`None` — цель из командной строки).
    pub from: Option<String>,
    /// Исходное требование, как в рецепте.
    pub req: DepReq,
    /// Вид требования (`build`/`run`).
    pub kind: DepKind,
}

/// Один собранный узел графа.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedNode {
    /// Имя пакета.
    pub name: String,
    /// Выбранная версия.
    pub version: Version,
    /// Исходная строка версии.
    pub version_str: String,
    /// Ревизия сборки.
    pub build: u32,
    /// Репозиторий-источник.
    pub repo: String,
    /// Пакет нужен для чьей-либо сборки.
    pub as_build: bool,
    /// Пакет нужен в чьём-либо рантайме.
    pub as_run: bool,
    /// Пакет является целью из командной строки.
    pub is_target: bool,
    /// Пакет уже установлен и удовлетворяет требованию (этап 5.2):
    /// узел «уже выполнено», пересобирать не нужно.
    pub installed: bool,
    /// Узел выбран как поставщик виртуального пакета с этим именем (этап 5.4).
    pub provided_for: Option<String>,
    /// Кто и как требует этот пакет (отсортировано, без дубликатов).
    pub required_by: Vec<(Option<String>, DepKind)>,
    /// Имена узлов, от которых узел зависит (в порядке обнаружения).
    pub dependencies: Vec<String>,
    /// Разобранные `conflicts.pkgs` рецепта (этап 5.5).
    pub conflicts: Vec<DepReq>,
}

impl CollectedNode {
    fn new(candidate: Candidate, is_target: bool) -> Self {
        CollectedNode {
            name: candidate.name,
            version: candidate.version,
            version_str: candidate.version_str,
            build: candidate.build,
            repo: candidate.repo,
            as_build: false,
            as_run: false,
            is_target,
            installed: false,
            provided_for: None,
            required_by: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
        }
    }
}

/// Результат сбора: узлы по имени + все рёбра.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequirementSet {
    /// Узлы графа, по имени пакета.
    pub nodes: BTreeMap<String, CollectedNode>,
    /// Все рёбра в порядке обнаружения.
    pub edges: Vec<DepEdge>,
}

impl RequirementSet {
    /// Узлы, нужные только для сборки (не для рантайма).
    pub fn build_only(&self) -> impl Iterator<Item = &CollectedNode> {
        self.nodes.values().filter(|n| n.as_build && !n.as_run)
    }

    /// Узлы, которые нужно собрать/установить (не `installed`).
    pub fn pending(&self) -> impl Iterator<Item = &CollectedNode> {
        self.nodes.values().filter(|n| !n.installed)
    }
}

/// Разобрать сырую зависимость из рецепта/индекса.
fn parse_dep(raw: &str, context: &str) -> ResolveResult<DepReq> {
    DepReq::parse(raw).map_err(|source| ResolveError::InvalidDep {
        raw: raw.to_string(),
        context: context.to_string(),
        source,
    })
}

/// Выбрать кандидата по требованию: первый по приоритету репозиторий,
/// в котором есть подходящая версия; внутри репозитория — максимальная
/// (`latest-satisfying`, этап 5.3).
pub fn select_latest_satisfying<'a>(
    candidates: &'a [Candidate],
    req: &DepReq,
) -> Option<&'a Candidate> {
    let mut seen_repos: Vec<&str> = Vec::new();
    for c in candidates {
        if !seen_repos.contains(&c.repo.as_str()) {
            seen_repos.push(&c.repo);
        }
    }

    for repo in seen_repos {
        let best = candidates
            .iter()
            .filter(|c| c.repo == repo)
            .filter(|c| req.req.as_ref().map_or(true, |r| c.version.satisfies(r)))
            .max_by(|a, b| a.version.cmp(&b.version).then(a.build.cmp(&b.build)));
        if let Some(best) = best {
            return Some(best);
        }
    }
    None
}

/// Собрать требования: рекурсивно раскрыть `deps.build` и `deps.run`
/// начиная с `targets`. Обёртка над [`resolve`] с опциями по умолчанию.
pub fn collect_requirements(
    targets: &[DepReq],
    source: &dyn PackageSource,
    features: &[String],
) -> ResolveResult<RequirementSet> {
    resolve(
        targets,
        source,
        &ResolveOptions {
            features: features.to_vec(),
            ..ResolveOptions::default()
        },
    )
}

/// Полное разрешение зависимостей с опциями (этапы 5.2–5.5).
pub fn resolve(
    targets: &[DepReq],
    source: &dyn PackageSource,
    options: &ResolveOptions<'_>,
) -> ResolveResult<RequirementSet> {
    let mut result = RequirementSet::default();
    let mut queue: VecDeque<(DepReq, DepKind, Option<String>)> = VecDeque::new();

    for target in targets {
        queue.push_back((target.clone(), DepKind::Run, None));
    }

    while let Some((req, kind, parent)) = queue.pop_front() {
        result.edges.push(DepEdge {
            from: parent.clone(),
            req: req.clone(),
            kind,
        });

        // 5.4: поставщик виртуального пакета, если пакета с таким именем нет.
        let (node_name, provided_for) = resolve_name(&req, source, options)?;

        if let Some(existing) = result.nodes.get_mut(&node_name) {
            // Узел уже выбран — версия обязана удовлетворять новому требованию.
            let ok = req
                .req
                .as_ref()
                .map_or(true, |r| existing.version.satisfies(r));
            if !ok {
                return Err(ResolveError::VersionConflict {
                    name: node_name.clone(),
                    selected: existing.version.to_string(),
                    required: req.to_string(),
                });
            }
            merge_edge(existing, kind, parent.as_deref());
            push_dependency(&mut result.nodes, parent.as_deref(), &node_name);
            continue;
        }

        // 5.2: установленный пакет, удовлетворяющий требованию.
        if let Some(installed) = options.installed.and_then(|i| i.installed(&node_name)) {
            let ok = req
                .req
                .as_ref()
                .map_or(true, |r| installed.version.satisfies(r));
            if ok {
                let is_target = parent.is_none();
                let mut node = CollectedNode {
                    name: installed.name.clone(),
                    version: installed.version,
                    version_str: installed.version_str,
                    build: installed.build_rev,
                    repo: String::new(),
                    as_build: false,
                    as_run: false,
                    is_target,
                    installed: true,
                    provided_for,
                    required_by: Vec::new(),
                    dependencies: Vec::new(),
                    conflicts: Vec::new(),
                };
                merge_edge(&mut node, kind, parent.as_deref());
                result.nodes.insert(installed.name, node);
                push_dependency(&mut result.nodes, parent.as_deref(), &node_name);
                continue;
            }
        }

        let candidates = source.candidates(&node_name)?;
        if candidates.is_empty() {
            return Err(ResolveError::Unresolved {
                req: req.to_string(),
                reason: "пакет не найден ни в одном репозитории".to_string(),
            });
        }
        let selected = select_latest_satisfying(&candidates, &req).ok_or_else(|| {
            ResolveError::Unresolved {
                req: req.to_string(),
                reason: format!(
                    "нет версии, удовлетворяющей требованию (доступны: {})",
                    candidates
                        .iter()
                        .map(|c| c.version.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        })?;

        let is_target = parent.is_none();
        let mut node = CollectedNode::new(selected.clone(), is_target);
        node.provided_for = provided_for;
        merge_edge(&mut node, kind, parent.as_deref());
        let name = node.name.clone();
        result.nodes.insert(name.clone(), node);
        push_dependency(&mut result.nodes, parent.as_deref(), &name);

        // Раскрываем зависимости выбранного рецепта.
        let recipe = source.recipe(selected)?;
        let context = format!("{name} {}", selected.version_str);
        // 5.5: собираем конфликты для последующей проверки.
        let conflicts = recipe
            .conflicts
            .pkgs
            .iter()
            .map(|raw| parse_dep(raw, &context))
            .collect::<ResolveResult<Vec<_>>>()?;
        result.nodes.get_mut(&name).expect("узел вставлен").conflicts = conflicts;

        for raw in &recipe.deps.build {
            let dep = parse_dep(raw, &context)?;
            queue.push_back((dep, DepKind::Build, Some(name.clone())));
        }
        for raw in &recipe.deps.run {
            let dep = parse_dep(raw, &context)?;
            queue.push_back((dep, DepKind::Run, Some(name.clone())));
        }
        for (raw, feature) in &recipe.deps.optional {
            if options.features.iter().any(|f| f == feature) {
                let dep = parse_dep(raw, &context)?;
                queue.push_back((dep, DepKind::Run, Some(name.clone())));
            }
        }
    }

    // Детерминированный порядок required_by.
    for node in result.nodes.values_mut() {
        node.required_by.sort();
        node.required_by.dedup();
    }

    // 5.5: проверка conflicts.
    check_conflicts(&result, options.installed)?;
    // 5.5: build-цикл — ошибка (runtime-цикл разрешён).
    check_build_cycles(&result)?;

    Ok(result)
}

/// Определить имя узла для требования: само имя, либо поставщик виртуального
/// пакета (этап 5.4). Возвращает (имя узла, имя виртуального пакета).
fn resolve_name(
    req: &DepReq,
    source: &dyn PackageSource,
    options: &ResolveOptions<'_>,
) -> ResolveResult<(String, Option<String>)> {
    // Явный пакет существует?
    if !source.candidates(&req.name)?.is_empty() {
        return Ok((req.name.clone(), None));
    }
    // Может, установлен?
    if options
        .installed
        .and_then(|i| i.installed(&req.name))
        .is_some()
    {
        return Ok((req.name.clone(), None));
    }

    // Поиск поставщиков по provides.virtual.
    let providers = source.providers(&req.name)?;
    if providers.is_empty() {
        // Пусть обработка ошибки произойдёт на этапе candidates (единый путь).
        return Ok((req.name.clone(), None));
    }
    let satisfying: Vec<Candidate> = providers
        .into_iter()
        .filter(|c| req.req.as_ref().map_or(true, |r| c.version.satisfies(r)))
        .collect();
    if satisfying.is_empty() {
        return Ok((req.name.clone(), None)); // дальше Unresolved с пустым списком
    }

    // Выбор поставщика: установленный > предпочтение пользователя > единственный.
    if let Some(installed) = options.installed {
        if let Some(found) = satisfying.iter().find(|c| installed.installed(&c.name).is_some()) {
            return Ok((found.name.clone(), Some(req.name.clone())));
        }
    }
    if let Some(preferred) = &options.provider {
        if let Some(found) = satisfying.iter().find(|c| &c.name == preferred) {
            return Ok((found.name.clone(), Some(req.name.clone())));
        }
    }
    if let Some(selected) = select_latest_satisfying(&satisfying, &DepReq {
        name: req.name.clone(),
        req: None,
    }) {
        let distinct: std::collections::BTreeSet<&str> =
            satisfying.iter().map(|c| c.name.as_str()).collect();
        if distinct.len() > 1 {
            // Разные поставщики с подходящей версией — нужен выбор пользователя.
            return Err(ResolveError::AmbiguousProvider {
                virtual_name: req.name.clone(),
                providers: distinct.into_iter().map(str::to_string).collect(),
            });
        }
        return Ok((selected.name.clone(), Some(req.name.clone())));
    }

    Ok((req.name.clone(), None))
}

fn push_dependency(
    nodes: &mut BTreeMap<String, CollectedNode>,
    parent: Option<&str>,
    dep: &str,
) {
    if let Some(parent) = parent {
        if let Some(node) = nodes.get_mut(parent) {
            if !node.dependencies.iter().any(|d| d == dep) {
                node.dependencies.push(dep.to_string());
            }
        }
    }
}

fn merge_edge(node: &mut CollectedNode, kind: DepKind, parent: Option<&str>) {
    match kind {
        DepKind::Build => node.as_build = true,
        DepKind::Run => node.as_run = true,
    }
    // Цели из командной строки не имеют parent.
    if parent.is_some() || !node.is_target {
        node.required_by.push((parent.map(str::to_string), kind));
    }
}

/// Проверка `conflicts` всех выбранных (и установленных) пакетов (этап 5.5).
fn check_conflicts(
    set: &RequirementSet,
    installed: Option<&dyn InstalledSource>,
) -> ResolveResult<()> {
    let names: Vec<String> = set.nodes.keys().cloned().collect();
    for name in &names {
        let node = &set.nodes[name];
        for conflict in &node.conflicts {
            // Против выбранных узлов.
            for (other_name, other) in &set.nodes {
                if other_name == name {
                    continue;
                }
                if conflict_matches(conflict, other_name, &other.version) {
                    return Err(ResolveError::Conflict {
                        pkg: name.clone(),
                        conflicting: other_name.clone(),
                        req: conflict.to_string(),
                    });
                }
            }
            // Против установленных пакетов, не входящих в план.
            if let Some(installed) = installed {
                if let Some(inst) = installed.installed(&conflict.name) {
                    let in_set = set.nodes.contains_key(&conflict.name);
                    if !in_set && conflict_matches(conflict, &inst.name, &inst.version) {
                        return Err(ResolveError::Conflict {
                            pkg: name.clone(),
                            conflicting: inst.name,
                            req: conflict.to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn conflict_matches(req: &DepReq, name: &str, version: &Version) -> bool {
    if req.name != name {
        return false;
    }
    req.req.as_ref().map_or(true, |r| version.satisfies(r))
}

/// Обнаружение циклов по `deps.build` (этап 5.5). Runtime-цикл разрешён.
fn check_build_cycles(set: &RequirementSet) -> ResolveResult<()> {
    // adjacency по build-рёбрам: parent -> build-зависимость.
    // Зависимость может указывать на виртуальное имя; фактический узел —
    // тот, у кого provided_for == этому имени.
    let mut build_adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &set.edges {
        if edge.kind != DepKind::Build {
            continue;
        }
        if let Some(from) = edge.from.as_deref() {
            // Зависимость может указывать на виртуальное имя; найдём фактический узел
            // как тот, у кого required_by содержит from:Build — или по имени.
            let target = if set.nodes.contains_key(&edge.req.name) {
                edge.req.name.as_str()
            } else {
                // поставщик виртуального пакета
                set.nodes
                    .values()
                    .find(|n| n.provided_for.as_deref() == Some(edge.req.name.as_str()))
                    .map(|n| n.name.as_str())
                    .unwrap_or(edge.req.name.as_str())
            };
            build_adj.entry(from).or_default().push(target);
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let mut color: BTreeMap<&str, Color> = set.nodes.keys().map(|k| (k.as_str(), Color::White)).collect();
    let mut stack: Vec<&str> = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a str>>,
        color: &mut BTreeMap<&'a str, Color>,
        stack: &mut Vec<&'a str>,
    ) -> Result<(), Vec<&'a str>> {
        color.insert(node, Color::Gray);
        stack.push(node);
        if let Some(children) = adj.get(node) {
            for &child in children {
                match color.get(child).copied().unwrap_or(Color::White) {
                    Color::White => {
                        if let Err(path) = dfs(child, adj, color, stack) {
                            return Err(path);
                        }
                    }
                    Color::Gray => {
                        let mut path: Vec<&str> = stack
                            .iter()
                            .skip_while(|&&n| n != child)
                            .copied()
                            .collect();
                        path.push(child);
                        return Err(path);
                    }
                    Color::Black => {}
                }
            }
        }
        stack.pop();
        color.insert(node, Color::Black);
        Ok(())
    }

    for node in set.nodes.keys().map(String::as_str).collect::<Vec<_>>() {
        if color[node] == Color::White {
            if let Err(path) = dfs(node, &build_adj, &mut color, &mut stack) {
                return Err(ResolveError::BuildCycle {
                    path: path.join(" -> "),
                });
            }
        }
    }
    Ok(())
}
