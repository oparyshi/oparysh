//! Сбор требований (этап 5.1 ROADMAP, DOCS §7 п.1).
//!
//! Начинаем с целей (`oparysh install nginx pcre2>=10.32`) и рекурсивно
//! раскрываем `deps.build` (нужны только при сборке) и `deps.run` (нужны
//! в рантайме). Опциональные зависимости (`deps.optional`) включаются,
//! если включена соответствующая фича (`oparysh install nginx +tls`).
//!
//! Версия каждого пакета выбирается стратегией **latest-satisfying**:
//! кандидаты берутся со всех включённых репозиториев в порядке приоритета
//! конфига; в первом репозитории, где есть подходящая версия, выбирается
//! максимальная (приоритет репозитория важнее свежести версии в другом
//! репозитории — DOCS §7 п.3). Более тонкая настройка — этап 5.3.
//!
//! Учёт уже установленных пакетов — этап 5.2, виртуальные пакеты/`provides`
//! — этап 5.4, обнаружение циклов и проверка `conflicts` — этап 5.5.
//! Здесь рекурсия просто **завершается** за счёт множества посещённых
//! узлов; рёбра цикла фиксируются как обычные зависимости.

use std::collections::{BTreeMap, VecDeque};

use oparysh_core::recipe::Recipe;
use oparysh_core::version::{DepReq, Version};

use crate::error::{ResolveError, ResolveResult};

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
    /// Кто и как требует этот пакет (отсортировано, без дубликатов).
    pub required_by: Vec<(Option<String>, DepKind)>,
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
            required_by: Vec::new(),
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
/// (`latest-satisfying`).
fn select<'a>(candidates: &'a [Candidate], req: &DepReq) -> Option<&'a Candidate> {
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
/// начиная с `targets`.
///
/// `features` — включённые фичи (`oparysh install nginx +tls`); включают
/// соответствующие `deps.optional`.
///
/// Ошибки: [`ResolveError::Unresolved`] — нет кандидата, удовлетворяющего
/// требованию; [`ResolveError::VersionConflict`] — ранее выбранная версия
/// не удовлетворяет новому требованию; ошибки источника — как есть.
pub fn collect_requirements(
    targets: &[DepReq],
    source: &dyn PackageSource,
    features: &[String],
) -> ResolveResult<RequirementSet> {
    let mut result = RequirementSet::default();
    // name -> индекс в порядке вставки не нужен; BTreeMap достаточен.
    let mut queue: VecDeque<(DepReq, DepKind, Option<String>)> = VecDeque::new();

    for target in targets {
        queue.push_back((target.clone(), DepKind::Run, None));
    }

    while let Some((req, kind, parent)) = queue.pop_front() {
        let edge = DepEdge {
            from: parent.clone(),
            req: req.clone(),
            kind,
        };
        result.edges.push(edge);

        let candidates = source.candidates(&req.name)?;
        if let Some(existing) = result.nodes.get_mut(&req.name) {
            // Узел уже выбран — версия обязана удовлетворять новому требованию.
            let ok = req
                .req
                .as_ref()
                .map_or(true, |r| existing.version.satisfies(r));
            if !ok {
                return Err(ResolveError::VersionConflict {
                    name: req.name.clone(),
                    selected: existing.version.to_string(),
                    required: req.to_string(),
                });
            }
            merge_edge(existing, kind, parent.as_deref());
            continue;
        }

        if candidates.is_empty() {
            return Err(ResolveError::Unresolved {
                req: req.to_string(),
                reason: "пакет не найден ни в одном репозитории".to_string(),
            });
        }
        let selected = select(&candidates, &req).ok_or_else(|| ResolveError::Unresolved {
            req: req.to_string(),
            reason: format!(
                "нет версии, удовлетворяющей требованию (доступны: {})",
                candidates
                    .iter()
                    .map(|c| c.version.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })?;

        let is_target = parent.is_none();
        let mut node = CollectedNode::new(selected.clone(), is_target);
        merge_edge(&mut node, kind, parent.as_deref());
        let name = node.name.clone();
        result.nodes.insert(name.clone(), node);

        // Раскрываем зависимости выбранного рецепта.
        let recipe = source.recipe(selected)?;
        let context = format!("{name} {}", selected.version_str);
        for raw in &recipe.deps.build {
            let dep = parse_dep(raw, &context)?;
            queue.push_back((dep, DepKind::Build, Some(name.clone())));
        }
        for raw in &recipe.deps.run {
            let dep = parse_dep(raw, &context)?;
            queue.push_back((dep, DepKind::Run, Some(name.clone())));
        }
        for (raw, feature) in &recipe.deps.optional {
            if features.iter().any(|f| f == feature) {
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

    Ok(result)
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
