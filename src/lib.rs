use iii_sdk::{FunctionRef, InitOptions, RegisterFunction, RegisterServiceMessage, WorkerMetadata};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, ArtifactError>;

pub const WORKER_NAME: &str = "artifact-cli-worker";
pub const ARTIFACT_FUNCTION_IDS: [&str; 7] = [
    "artifact::catalog",
    "artifact::recipes",
    "artifact::inspect",
    "artifact::plan_worker",
    "artifact::generate_worker",
    "artifact::verify_worker",
    "artifact::manifest",
];

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    OpenApi,
    Graphql,
    Har,
    Docs,
    Url,
    #[default]
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactInput {
    pub name: String,
    pub goal: Option<String>,
    pub source_type: Option<SourceType>,
    pub source: Option<String>,
    #[serde(default)]
    pub functions: Vec<String>,
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyWorkerInput {
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SideEffects {
    Read,
    Write,
    Sync,
    ExternalCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReusableWorker {
    pub name: String,
    pub source: String,
    pub install: Option<String>,
    pub purpose: String,
    pub capabilities: Vec<String>,
    pub functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerCatalog {
    pub engine_builtins: Vec<ReusableWorker>,
    pub installable_workers: Vec<ReusableWorker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRecipe {
    pub name: String,
    pub category: String,
    pub stage: RecipeStage,
    pub priority: u8,
    pub integration: String,
    pub goal: String,
    pub source_hints: Vec<String>,
    pub default_functions: Vec<String>,
    pub research_links: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecipeStage {
    BuildNow,
    ResearchFirst,
    Later,
}

type WorkerRecipeDetails<'a> = (&'a str, &'a str, &'a str);
type WorkerRecipeSources<'a> = (&'a [&'a str], &'a [&'a str], &'a [&'a str]);

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReusePlan {
    pub engine_builtins: Vec<ReusableWorker>,
    pub installable_workers: Vec<ReusableWorker>,
    pub missing_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerFunctionPlan {
    pub function_id: String,
    pub purpose: String,
    pub side_effects: SideEffects,
    pub inputs: serde_json::Value,
    pub output: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerPlan {
    pub worker_name: String,
    pub namespace: String,
    pub source_type: SourceType,
    pub source: Option<String>,
    pub goal: String,
    pub functions: Vec<WorkerFunctionPlan>,
    pub uses_workers: Vec<String>,
    pub reuse_plan: ReusePlan,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectResult {
    pub name: String,
    pub namespace: String,
    pub source_type: SourceType,
    pub source: Option<String>,
    pub suggested_functions: Vec<String>,
    pub recommendation: String,
    pub existing_workers_to_use: Vec<String>,
    pub reuse_plan: ReusePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedWorker {
    pub output_dir: PathBuf,
    pub worker_path: PathBuf,
    pub manifest_path: PathBuf,
    pub plan: WorkerPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationReport {
    pub ok: bool,
    pub worker_path: PathBuf,
    pub function_count: usize,
    pub missing_registrations: Vec<String>,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifestPreview {
    pub schema: String,
    pub worker_name: String,
    pub namespace: String,
    pub functions: Vec<WorkerFunctionPlan>,
    pub uses_workers: Vec<String>,
    pub reuse_plan: ReusePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerInstallPlan {
    pub ok: bool,
    pub worker_name: String,
    pub worker_dir: PathBuf,
    pub dependencies: Vec<ReusableWorker>,
    pub commands: Vec<String>,
    pub verification: VerificationReport,
}

pub fn registered_function_ids() -> Vec<&'static str> {
    ARTIFACT_FUNCTION_IDS.to_vec()
}

pub fn worker_metadata() -> WorkerMetadata {
    WorkerMetadata {
        runtime: "rust".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        name: WORKER_NAME.into(),
        os: format!(
            "{} {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        ),
        pid: Some(std::process::id()),
        telemetry: None,
        isolation: None,
    }
}

pub fn init_options() -> InitOptions {
    InitOptions {
        metadata: Some(worker_metadata()),
        ..Default::default()
    }
}

pub fn register_artifact_primitives(iii: &iii_sdk::III) -> Vec<FunctionRef> {
    iii.register_service(RegisterServiceMessage {
        id: WORKER_NAME.into(),
        name: "Artifact CLI Worker".into(),
        description: Some("Plan, generate, verify, and manifest narrow Rust iii workers.".into()),
        parent_service_id: None,
    });

    vec![
        iii.register_function(
            RegisterFunction::new(
                "artifact::catalog",
                |_payload: serde_json::Value| -> Result<WorkerCatalog> { Ok(worker_catalog()) },
            )
            .description(
                "List iii engine builtins and iii-hq/workers modules artifact-cli can reuse.",
            ),
        ),
        iii.register_function(
            RegisterFunction::new(
                "artifact::recipes",
                |_payload: serde_json::Value| -> Result<Vec<WorkerRecipe>> { Ok(worker_recipes()) },
            )
            .description("List narrow worker recipes artifact-cli can generate."),
        ),
        iii.register_function(
            RegisterFunction::new("artifact::inspect", inspect_artifact)
                .description("Inspect an artifact source and suggest narrow iii worker functions."),
        ),
        iii.register_function(
            RegisterFunction::new("artifact::plan_worker", plan_worker)
                .description("Create a narrow iii worker plan from an artifact description."),
        ),
        iii.register_function(
            RegisterFunction::new("artifact::generate_worker", generate_worker)
                .description("Generate a Rust iii worker scaffold from an artifact plan."),
        ),
        iii.register_function(
            RegisterFunction::new("artifact::verify_worker", verify_worker)
                .description("Run structural verification on a generated artifact worker."),
        ),
        iii.register_function(
            RegisterFunction::new("artifact::manifest", artifact_manifest)
                .description("Create a manifest preview for a generated artifact worker."),
        ),
    ]
}

pub fn worker_catalog() -> WorkerCatalog {
    WorkerCatalog {
        engine_builtins: engine_builtin_catalog(),
        installable_workers: installable_worker_catalog(),
    }
}

pub fn worker_recipes() -> Vec<WorkerRecipe> {
    vec![
        worker_recipe(
            "digg",
            "media",
            RecipeStage::BuildNow,
            95,
            (
                "public web dataset",
                "Answer top stories, AI 1000 rank lookup, story search, highlights, and pipeline status.",
                "Small read-only surface with obvious agent questions and no account setup requirement.",
            ),
            (
                &["digg", "di.gg", "ai 1000"],
                &[
                    "top_stories",
                    "author_rank",
                    "search_stories",
                    "story_highlights",
                    "pipeline_status",
                ],
                &["https://di.gg/ai"],
            ),
        ),
        worker_recipe(
            "hackernews",
            "media",
            RecipeStage::BuildNow,
            100,
            (
                "public read-only Firebase JSON",
                "Give agents focused access to top stories, item lookup, and cached story search.",
                "Canonical public API, no auth, stable repeated workflows, and a tiny worker surface.",
            ),
            (
                &["hackernews", "hacker news", "news.ycombinator", "hn.algolia"],
                &["top_stories", "get_item", "search_cached_stories"],
                &["https://github.com/HackerNews/API"],
            ),
        ),
        worker_recipe(
            "producthunt",
            "marketing",
            RecipeStage::ResearchFirst,
            90,
            (
                "official GraphQL API with OAuth",
                "Track launches, maker profiles, topic search, and launch momentum.",
                "High demo value, but GraphQL auth and query shape should be researched before live handlers.",
            ),
            (
                &["producthunt", "product hunt"],
                &[
                    "top_launches",
                    "launch_details",
                    "maker_lookup",
                    "topic_search",
                    "launch_metrics",
                ],
                &["https://www.producthunt.com/v2/docs"],
            ),
        ),
        worker_recipe(
            "linear",
            "project_management",
            RecipeStage::ResearchFirst,
            88,
            (
                "official GraphQL API with API key or OAuth",
                "Summarize blocked work, stale issues, cycle risk, issue search, and team load.",
                "Strong agent use case, but schema, scopes, and workspace auth should be planned first.",
            ),
            (
                &["linear", "linear.app"],
                &[
                    "blocked_issues",
                    "stale_issues",
                    "cycle_risk",
                    "issue_search",
                    "team_workload",
                ],
                &["https://linear.app/docs/api/"],
            ),
        ),
        worker_recipe(
            "github_repo",
            "developer_tools",
            RecipeStage::ResearchFirst,
            86,
            (
                "GitHub API and local git metadata",
                "Summarize repo health, stale PRs, open issues, releases, and failing checks.",
                "Useful across nearly every project, but should be scoped around repo-risk jobs not full GitHub coverage.",
            ),
            (
                &["github repo", "pull request", "github.com"],
                &[
                    "repo_summary",
                    "stale_prs",
                    "open_issues",
                    "release_notes",
                    "ci_failures",
                ],
                &["https://docs.github.com/en/rest"],
            ),
        ),
        worker_recipe(
            "stripe",
            "payments",
            RecipeStage::ResearchFirst,
            78,
            (
                "official REST API with account-scoped keys",
                "Inspect customer health, subscription risk, failed payments, invoices, and revenue snapshots.",
                "High-value business workflows, but money movement and account permissions make this research-first.",
            ),
            (
                &["stripe"],
                &[
                    "customer_summary",
                    "subscription_risk",
                    "failed_payments",
                    "invoice_lookup",
                    "revenue_snapshot",
                ],
                &["https://docs.stripe.com/api"],
            ),
        ),
        worker_recipe(
            "arxiv",
            "research",
            RecipeStage::BuildNow,
            82,
            (
                "public Atom query API",
                "Search papers, summarize findings, inspect author trends, and build citation packs.",
                "Public read-only metadata with a natural cache/search worker shape.",
            ),
            (
                &["arxiv", "arxiv.org"],
                &[
                    "search_papers",
                    "paper_summary",
                    "author_trends",
                    "related_papers",
                    "citation_pack",
                ],
                &["https://arxiv.org/help/api/user-manual"],
            ),
        ),
        worker_recipe(
            "wikipedia",
            "knowledge",
            RecipeStage::BuildNow,
            80,
            (
                "public MediaWiki REST and action APIs",
                "Summarize pages, search topics, read sections, cite sources, and compare pages.",
                "Public knowledge source where narrow citation and compare functions are more useful than raw API access.",
            ),
            (
                &["wikipedia", "wikimedia"],
                &[
                    "article_summary",
                    "topic_search",
                    "page_sections",
                    "citations",
                    "compare_pages",
                ],
                &["https://www.mediawiki.org/wiki/API:REST_API"],
            ),
        ),
        worker_recipe(
            "sentry",
            "monitoring",
            RecipeStage::ResearchFirst,
            76,
            (
                "official REST API with org/project auth",
                "Summarize production issues, release regressions, trends, suspect commits, and alerts.",
                "Good operational value, but org scopes, rate limits, and alert semantics need validation.",
            ),
            (
                &["sentry"],
                &[
                    "issue_summary",
                    "release_regressions",
                    "error_trends",
                    "suspect_commits",
                    "alert_digest",
                ],
                &["https://docs.sentry.io/api/"],
            ),
        ),
        worker_recipe(
            "slack",
            "productivity",
            RecipeStage::ResearchFirst,
            72,
            (
                "Slack Web API with app tokens and scopes",
                "Search channels, summarize threads, extract decisions, and prepare follow-ups.",
                "Compelling agent memory workflows, but workspace scopes and data retention rules need deliberate design.",
            ),
            (
                &["slack"],
                &[
                    "channel_search",
                    "thread_summary",
                    "decision_digest",
                    "followups",
                    "user_context",
                ],
                &["https://api.slack.com/web"],
            ),
        ),
        worker_recipe(
            "notion",
            "productivity",
            RecipeStage::ResearchFirst,
            70,
            (
                "official REST API with integration permissions",
                "Search workspace knowledge, summarize pages, inspect databases, and create update briefs.",
                "Knowledge workflows are strong, but page/database capability gaps should be mapped before implementation.",
            ),
            (
                &["notion"],
                &[
                    "workspace_search",
                    "page_summary",
                    "database_query",
                    "decision_log",
                    "update_brief",
                ],
                &["https://developers.notion.com/guides/get-started"],
            ),
        ),
        worker_recipe(
            "openrouter",
            "ai",
            RecipeStage::ResearchFirst,
            68,
            (
                "OpenAI-compatible API with model registry",
                "Compare model availability, pricing, capabilities, and routing fit.",
                "Useful for model routing, but needs current model/pricing research and cache policy before live calls.",
            ),
            (
                &["openrouter"],
                &[
                    "model_search",
                    "model_compare",
                    "pricing_lookup",
                    "capability_filter",
                    "routing_recommendation",
                ],
                &["https://openrouter.ai/docs/api-reference/overview/"],
            ),
        ),
    ]
}

pub fn inspect_artifact(input: ArtifactInput) -> Result<InspectResult> {
    let namespace = slugify(&input.name);
    let source_type = input.source_type.clone().unwrap_or_default();
    let functions = infer_functions(&input);
    let reuse_plan = plan_reuse(&input, &functions);
    let existing_workers_to_use = reuse_worker_names(&reuse_plan);
    Ok(InspectResult {
        name: input.name.clone(),
        namespace: namespace.clone(),
        source_type,
        source: input.source.clone(),
        suggested_functions: functions
            .iter()
            .map(|function| format!("{}::{}", namespace, slugify(function)))
            .collect(),
        recommendation:
            "Generate a narrow iii worker around the specific job, not a generic full API wrapper."
                .into(),
        existing_workers_to_use,
        reuse_plan,
    })
}

pub fn plan_worker(input: ArtifactInput) -> Result<WorkerPlan> {
    let namespace = slugify(&input.name);
    let source_type = input.source_type.clone().unwrap_or_default();
    let inferred_functions = infer_functions(&input);
    let functions = inferred_functions
        .iter()
        .map(|function| plan_function(&namespace, function))
        .collect::<Vec<_>>();
    let reuse_plan = plan_reuse(&input, &inferred_functions);
    let uses_workers = reuse_worker_names(&reuse_plan);

    Ok(WorkerPlan {
        worker_name: format!("{}-worker", namespace.replace('_', "-")),
        namespace: namespace.clone(),
        source_type,
        source: input.source.clone(),
        goal: input
            .goal
            .clone()
            .unwrap_or_else(|| format!("Expose focused agent-operable functions for {}.", input.name)),
        functions,
        uses_workers,
        reuse_plan,
        notes: vec![
            "Keep function count small and job-specific.".into(),
            "Prefer read-only functions unless the worker explicitly syncs or mutates external state.".into(),
            "Persist manifests and source fingerprints through iii-state.".into(),
            "Run generated code checks inside iii-sandbox before publishing.".into(),
        ],
    })
}

pub fn artifact_manifest(input: ArtifactInput) -> Result<ArtifactManifestPreview> {
    let plan = plan_worker(input)?;
    Ok(ArtifactManifestPreview {
        schema: "artifact-cli.manifest.preview.v1".into(),
        worker_name: plan.worker_name,
        namespace: plan.namespace,
        functions: plan.functions,
        uses_workers: plan.uses_workers,
        reuse_plan: plan.reuse_plan,
    })
}

pub fn generate_worker(input: ArtifactInput) -> Result<GeneratedWorker> {
    let plan = plan_worker(input.clone())?;
    let output_dir = input
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("generated").join(&plan.worker_name));
    let src_dir = output_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    let manifest_path = output_dir.join("artifact.manifest.json");
    let worker_path = src_dir.join("main.rs");

    fs::write(&manifest_path, serde_json::to_string_pretty(&plan)? + "\n")?;
    fs::write(&worker_path, render_worker_source(&plan))?;
    fs::write(output_dir.join("Cargo.toml"), render_worker_cargo(&plan))?;
    fs::write(output_dir.join("README.md"), render_worker_readme(&plan))?;
    fs::write(
        output_dir.join("iii.worker.yaml"),
        render_worker_yaml(&plan),
    )?;

    Ok(GeneratedWorker {
        output_dir,
        worker_path,
        manifest_path,
        plan,
    })
}

pub fn verify_worker(input: VerifyWorkerInput) -> Result<VerificationReport> {
    verify_worker_dir(input.output_dir)
}

pub fn install_plan(output_dir: impl AsRef<Path>) -> Result<WorkerInstallPlan> {
    let output_dir = output_dir.as_ref();
    let verification = verify_worker_dir(output_dir)?;
    let manifest_path = output_dir.join("artifact.manifest.json");
    let plan = if manifest_path.exists() {
        serde_json::from_str::<WorkerPlan>(&fs::read_to_string(&manifest_path)?)?
    } else {
        WorkerPlan {
            worker_name: "unknown-worker".into(),
            namespace: "unknown".into(),
            source_type: SourceType::Manual,
            source: None,
            goal: "Unknown generated worker.".into(),
            functions: Vec::new(),
            uses_workers: Vec::new(),
            reuse_plan: ReusePlan::default(),
            notes: Vec::new(),
        }
    };
    let mut commands = plan
        .reuse_plan
        .installable_workers
        .iter()
        .filter_map(|worker| worker.install.clone())
        .collect::<Vec<_>>();
    commands.push(format!(
        "cd {} && cargo build --release",
        output_dir.display()
    ));
    commands.push(format!(
        "III_URL=ws://localhost:49134 {}/target/release/{}",
        output_dir.display(),
        plan.worker_name
    ));

    Ok(WorkerInstallPlan {
        ok: verification.ok,
        worker_name: plan.worker_name,
        worker_dir: output_dir.to_path_buf(),
        dependencies: plan.reuse_plan.installable_workers,
        commands,
        verification,
    })
}

pub fn verify_worker_dir(output_dir: impl AsRef<Path>) -> Result<VerificationReport> {
    let output_dir = output_dir.as_ref();
    let manifest_path = output_dir.join("artifact.manifest.json");
    let worker_path = output_dir.join("src/main.rs");
    let required_files = [
        manifest_path.clone(),
        worker_path.clone(),
        output_dir.join("Cargo.toml"),
        output_dir.join("README.md"),
        output_dir.join("iii.worker.yaml"),
    ];
    let missing_files = required_files
        .iter()
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();

    if !missing_files.is_empty() {
        return Ok(VerificationReport {
            ok: false,
            worker_path,
            function_count: 0,
            missing_registrations: Vec::new(),
            missing_files,
        });
    }

    let plan: WorkerPlan = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let worker_source = fs::read_to_string(&worker_path)?;
    let missing_registrations = plan
        .functions
        .iter()
        .filter_map(|function| {
            let has_id = worker_source.contains(&function.function_id);
            let has_iii_registration = worker_source.contains("iii.register_function");
            if has_id && has_iii_registration {
                None
            } else {
                Some(function.function_id.clone())
            }
        })
        .collect::<Vec<_>>();

    Ok(VerificationReport {
        ok: missing_registrations.is_empty(),
        worker_path,
        function_count: plan.functions.len(),
        missing_registrations,
        missing_files,
    })
}

fn engine_builtin_catalog() -> Vec<ReusableWorker> {
    vec![
        reusable_worker(
            "iii-state",
            "iii-hq/iii",
            None,
            "Persist manifests, source fingerprints, cache metadata, and generated worker state.",
            &["state", "cache", "manifest", "fingerprint"],
            &[
                "state::get",
                "state::set",
                "state::delete",
                "state::list",
                "state::update",
            ],
        ),
        reusable_worker(
            "iii-queue",
            "iii-hq/iii",
            None,
            "Run generation, verification, sync, and publish jobs asynchronously with retries.",
            &[
                "queue",
                "async",
                "retry",
                "dlq",
                "generation",
                "verification",
            ],
            &["queue trigger", "durable subscriber"],
        ),
        reusable_worker(
            "iii-cron",
            "iii-hq/iii",
            None,
            "Refresh synced artifacts and local mirrors on schedules.",
            &["cron", "schedule", "refresh"],
            &["cron trigger"],
        ),
        reusable_worker(
            "iii-rest",
            "iii-hq/iii",
            None,
            "Expose generated worker functions as HTTP endpoints when needed.",
            &["http", "api", "endpoint", "rest"],
            &["http trigger"],
        ),
        reusable_worker(
            "iii-stream",
            "iii-hq/iii",
            None,
            "Stream long-running generation progress, sync results, and runtime events.",
            &["stream", "realtime", "events"],
            &["stream::get", "stream::set"],
        ),
        reusable_worker(
            "iii-sandbox",
            "iii-hq/iii",
            None,
            "Build, test, and execute generated code in an isolated engine sandbox.",
            &["sandbox", "build", "test", "verification", "shell"],
            &["sandbox::exec"],
        ),
        reusable_worker(
            "iii-observability",
            "iii-hq/iii",
            None,
            "Record traces, logs, rollups, and debug telemetry for generation and runtime calls.",
            &["observability", "trace", "log", "metric", "debug"],
            &[
                "engine::traces::list",
                "engine::traces::tree",
                "engine::alerts::*",
            ],
        ),
    ]
}

fn installable_worker_catalog() -> Vec<ReusableWorker> {
    vec![
        reusable_worker(
            "auth-credentials",
            "iii-hq/workers",
            Some("iii worker add auth-credentials"),
            "Store API keys and OAuth tokens for generated workers.",
            &["credentials", "auth", "oauth"],
            &["auth::set", "auth::get", "auth::list", "auth::clear", "auth::resolve"],
        ),
        reusable_worker(
            "shell-bash",
            "iii-hq/workers",
            Some("iii worker add shell-bash"),
            "Run sandboxed CLI, git, build, and smoke-test commands through the iii bus.",
            &["shell", "cli", "git", "build", "test", "verification"],
            &["shell::bash::exec", "shell::bash::which", "shell::bash::detect_clis"],
        ),
        reusable_worker(
            "shell-filesystem",
            "iii-hq/workers",
            Some("iii worker add shell-filesystem"),
            "Read, write, list, grep, and edit files for artifact ingestion and generated output.",
            &["filesystem", "docs", "source", "file", "grep"],
            &[
                "shell::filesystem::read",
                "shell::filesystem::write",
                "shell::filesystem::ls",
                "shell::filesystem::grep",
                "shell::filesystem::edit",
            ],
        ),
        reusable_worker(
            "iii-database",
            "iii-hq/workers",
            Some("iii worker add iii-database"),
            "Back generated workers with SQLite, Postgres, MySQL, query polling, and local mirrors.",
            &["database", "sqlite", "postgres", "mysql", "cache", "search", "sync"],
            &[
                "iii-database::query",
                "iii-database::execute",
                "iii-database::prepareStatement",
                "iii-database::runStatement",
                "iii-database::transaction",
            ],
        ),
        reusable_worker(
            "mcp",
            "iii-hq/workers",
            Some("iii worker add mcp"),
            "Expose selected iii functions as MCP tools for IDE and agent clients.",
            &["mcp", "tool", "agent", "ide", "publish"],
            &["mcp::handler"],
        ),
        reusable_worker(
            "skills",
            "iii-hq/workers",
            Some("iii worker add skills"),
            "Publish generated worker usage notes as resources, slash commands, and MCP prompts.",
            &["skills", "docs", "agent", "mcp", "publish"],
            &["skills::resources-list", "skills::resources-read", "prompts::mcp-list"],
        ),
        reusable_worker(
            "proof",
            "iii-hq/workers",
            Some("iii worker add proof"),
            "Verify web-facing generated workers with browser automation and replayable flows.",
            &["browser", "ui", "playwright", "test", "verification"],
            &[
                "proof::scan",
                "proof::browser::launch",
                "proof::browser::navigate",
                "proof::browser::snapshot",
                "proof::browser::click",
                "proof::report",
            ],
        ),
        reusable_worker(
            "provider-router",
            "iii-hq/workers",
            Some("iii worker add provider-router"),
            "Route assistant calls through installed model providers and session controls.",
            &["llm", "assistant", "provider", "router"],
            &[
                "router::stream_assistant",
                "router::abort",
                "router::push_steering",
                "router::push_followup",
            ],
        ),
        reusable_worker(
            "models-catalog",
            "iii-hq/workers",
            Some("iii worker add models-catalog"),
            "Query model capabilities when a generated worker needs model selection.",
            &["llm", "model", "capability", "provider"],
            &["models::list", "models::get", "models::supports", "models::register"],
        ),
        reusable_worker(
            "provider-openai",
            "iii-hq/workers",
            Some("iii worker add provider-openai"),
            "Call OpenAI model APIs as an iii provider.",
            &["llm", "model", "provider", "openai"],
            &["provider::openai::complete"],
        ),
        reusable_worker(
            "provider-anthropic",
            "iii-hq/workers",
            Some("iii worker add provider-anthropic"),
            "Call Anthropic Messages API as an iii provider.",
            &["llm", "model", "provider", "anthropic"],
            &["provider::anthropic::complete"],
        ),
        reusable_worker(
            "hook-fanout",
            "iii-hq/workers",
            Some("iii worker add hook-fanout"),
            "Publish events to subscribers and merge replies for extensible generated workflows.",
            &["hooks", "events", "fanout", "extension"],
            &["hooks::publish_collect"],
        ),
        reusable_worker(
            "session-tree",
            "iii-hq/workers",
            Some("iii worker add session-tree"),
            "Store agent sessions as typed parent-id trees when generated workers manage conversations.",
            &["session", "conversation", "tree"],
            &["session::*"],
        ),
        reusable_worker(
            "session-inbox",
            "iii-hq/workers",
            Some("iii worker add session-inbox"),
            "Buffer per-session steering and follow-up messages.",
            &["session", "inbox"],
            &["inbox::push", "inbox::drain", "inbox::peek"],
        ),
        reusable_worker(
            "policy-denylist",
            "iii-hq/workers",
            Some("iii worker add policy-denylist"),
            "Block unsafe tool calls before generated workers invoke external tools.",
            &["policy", "security", "guard"],
            &["policy::denylist::check"],
        ),
    ]
}

fn reusable_worker(
    name: &str,
    source: &str,
    install: Option<&str>,
    purpose: &str,
    capabilities: &[&str],
    functions: &[&str],
) -> ReusableWorker {
    ReusableWorker {
        name: name.into(),
        source: source.into(),
        install: install.map(str::to_string),
        purpose: purpose.into(),
        capabilities: capabilities.iter().map(|value| (*value).into()).collect(),
        functions: functions.iter().map(|value| (*value).into()).collect(),
    }
}

fn worker_recipe(
    name: &str,
    category: &str,
    stage: RecipeStage,
    priority: u8,
    details: WorkerRecipeDetails<'_>,
    sources: WorkerRecipeSources<'_>,
) -> WorkerRecipe {
    let (integration, goal, rationale) = details;
    let (source_hints, default_functions, research_links) = sources;
    WorkerRecipe {
        name: name.into(),
        category: category.into(),
        stage,
        priority,
        integration: integration.into(),
        goal: goal.into(),
        source_hints: source_hints.iter().map(|value| (*value).into()).collect(),
        default_functions: default_functions
            .iter()
            .map(|value| (*value).into())
            .collect(),
        research_links: research_links.iter().map(|value| (*value).into()).collect(),
        rationale: rationale.into(),
    }
}

fn plan_reuse(input: &ArtifactInput, functions: &[String]) -> ReusePlan {
    let capabilities = infer_capabilities(input, functions);
    let engine_builtins = engine_builtin_catalog()
        .into_iter()
        .filter(|worker| worker_matches(worker, &capabilities))
        .collect::<Vec<_>>();
    let installable_workers = installable_worker_catalog()
        .into_iter()
        .filter(|worker| worker_matches(worker, &capabilities))
        .collect::<Vec<_>>();
    let mut covered = Vec::new();
    for worker in engine_builtins.iter().chain(installable_workers.iter()) {
        for capability in &worker.capabilities {
            push_unique(&mut covered, capability);
        }
    }
    let missing_capabilities = capabilities
        .into_iter()
        .filter(|capability| !covered.iter().any(|covered| covered == capability))
        .collect();

    ReusePlan {
        engine_builtins,
        installable_workers,
        missing_capabilities,
    }
}

fn infer_capabilities(input: &ArtifactInput, functions: &[String]) -> Vec<String> {
    let mut capabilities = Vec::new();
    push_unique(&mut capabilities, "state");
    push_unique(&mut capabilities, "observability");
    push_unique(&mut capabilities, "sandbox");

    let source_type = input.source_type.clone().unwrap_or_default();
    let haystack = format!(
        "{} {} {} {}",
        input.name,
        input.goal.as_deref().unwrap_or_default(),
        input.source.as_deref().unwrap_or_default(),
        functions.join(" ")
    )
    .to_lowercase();

    let public_digg = contains_any(
        &haystack,
        &[
            "digg",
            "di.gg",
            "ai 1000",
            "leaderboard",
            "pipeline status",
            "top stories",
        ],
    );

    match source_type {
        SourceType::OpenApi | SourceType::Graphql | SourceType::Har | SourceType::Url => {
            push_unique(&mut capabilities, "http");
        }
        SourceType::Docs | SourceType::Manual => {
            push_unique(&mut capabilities, "docs");
            push_unique(&mut capabilities, "filesystem");
        }
    }

    if contains_any(
        &haystack,
        &[
            "search", "cache", "cached", "sync", "mirror", "history", "sqlite", "postgres",
            "mysql", "sql",
        ],
    ) {
        push_unique(&mut capabilities, "database");
    }
    if contains_any(
        &haystack,
        &[
            "sync",
            "refresh",
            "generate",
            "verify",
            "background",
            "queue",
            "retry",
        ],
    ) {
        push_unique(&mut capabilities, "queue");
    }
    if contains_any(
        &haystack,
        &[
            "cron",
            "schedule",
            "scheduled",
            "daily",
            "hourly",
            "refresh",
        ],
    ) {
        push_unique(&mut capabilities, "cron");
    }
    if contains_any(
        &haystack,
        &["mcp", "tool", "agent", "codex", "claude", "ide", "publish"],
    ) {
        push_unique(&mut capabilities, "mcp");
    }
    if contains_any(
        &haystack,
        &["llm", "model", "assistant", "prompt", "completion"],
    ) {
        push_unique(&mut capabilities, "llm");
    }
    if contains_any(
        &haystack,
        &["browser", "ui", "playwright", "web app", "screenshot"],
    ) {
        push_unique(&mut capabilities, "browser");
        push_unique(&mut capabilities, "test");
    }
    if contains_any(
        &haystack,
        &["shell", "cli", "build", "cargo", "npm", "pnpm", "test"],
    ) {
        push_unique(&mut capabilities, "shell");
    }
    if contains_any(&haystack, &["event", "hook", "fanout", "subscriber"]) {
        push_unique(&mut capabilities, "events");
    }
    if contains_any(
        &haystack,
        &["policy", "security", "deny", "guard", "unsafe"],
    ) {
        push_unique(&mut capabilities, "policy");
    }
    if !public_digg
        && contains_any(
            &haystack,
            &[
                "oauth",
                "token",
                "api key",
                "credential",
                "github",
                "linear",
                "jira",
            ],
        )
    {
        push_unique(&mut capabilities, "credentials");
    }
    if public_digg {
        push_unique(&mut capabilities, "http");
        push_unique(&mut capabilities, "database");
    }

    capabilities
}

fn worker_matches(worker: &ReusableWorker, capabilities: &[String]) -> bool {
    worker
        .capabilities
        .iter()
        .any(|capability| capabilities.iter().any(|needed| needed == capability))
}

fn reuse_worker_names(reuse_plan: &ReusePlan) -> Vec<String> {
    reuse_plan
        .engine_builtins
        .iter()
        .chain(reuse_plan.installable_workers.iter())
        .map(|worker| worker.name.clone())
        .collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.into());
    }
}

fn infer_functions(input: &ArtifactInput) -> Vec<String> {
    if !input.functions.is_empty() {
        return input.functions.clone();
    }
    let haystack = format!(
        "{} {} {}",
        input.goal.as_deref().unwrap_or_default(),
        input.source.as_deref().unwrap_or_default(),
        input.name
    )
    .to_lowercase();

    let name = input.name.to_lowercase();
    if let Some(recipe) = matching_recipe(&name, &haystack) {
        return recipe.default_functions;
    }
    if name.contains("hackernews") || name == "hn" || haystack.contains("top stories") {
        return vec![
            "top_stories".into(),
            "get_item".into(),
            "search_cached_stories".into(),
        ];
    }
    if haystack.contains("issue") || haystack.contains("linear") || haystack.contains("jira") {
        return vec![
            "list_items".into(),
            "blocked_items".into(),
            "risk_summary".into(),
        ];
    }
    if haystack.contains("search") || haystack.contains("docs") {
        return vec![
            "search".into(),
            "get_document".into(),
            "answer_with_sources".into(),
        ];
    }
    if haystack.contains("github") || haystack.contains("repo") || haystack.contains("pull request")
    {
        return vec![
            "repo_summary".into(),
            "stale_prs".into(),
            "open_issues".into(),
        ];
    }
    vec!["inspect".into(), "list".into(), "get".into()]
}

fn matching_recipe(name: &str, haystack: &str) -> Option<WorkerRecipe> {
    worker_recipes().into_iter().find(|recipe| {
        recipe.name == name
            || recipe.source_hints.iter().any(|hint| {
                let hint = hint.to_lowercase();
                name.contains(&hint) || haystack.contains(&hint)
            })
    })
}

fn plan_function(namespace: &str, function: &str) -> WorkerFunctionPlan {
    let clean = slugify(function);
    let sync_like = clean.contains("sync") || clean.contains("refresh");
    if namespace == "digg" {
        return plan_digg_function(namespace, &clean);
    }
    WorkerFunctionPlan {
        function_id: format!("{}::{}", namespace, clean),
        purpose: format!("{} for the {} worker", titleize(&clean), namespace),
        side_effects: if sync_like {
            SideEffects::Sync
        } else {
            SideEffects::ExternalCall
        },
        inputs: if sync_like {
            serde_json::json!({ "force": "boolean optional; bypass cache when true" })
        } else {
            serde_json::json!({ "query": "string/object; focused request payload for this function" })
        },
        output: serde_json::json!({
            "ok": "boolean success flag",
            "data": "function-specific result payload",
            "sources": "optional source/provenance list"
        }),
    }
}

fn plan_digg_function(namespace: &str, function: &str) -> WorkerFunctionPlan {
    let (purpose, inputs, output) = match function {
        "top_stories" => (
            "Return the current top Digg AI story clusters for agent summaries.",
            serde_json::json!({ "limit": "number optional; default 10", "window": "string optional; today|24h|7d" }),
            serde_json::json!({ "stories": "ranked clusters with title, rank, url id, authors, and citation links" }),
        ),
        "author_rank" => (
            "Look up a person or X handle in the Digg AI 1000 and explain rank or off-list gap.",
            serde_json::json!({ "handle": "string optional; X handle", "name": "string optional; person name" }),
            serde_json::json!({ "author": "rank, handle, category, peer-follow count, nearest rank anchor, and gap when off-list" }),
        ),
        "search_stories" => (
            "Search Digg AI story clusters by topic with citations.",
            serde_json::json!({ "query": "string topic", "since": "string optional duration like 24h or 7d", "limit": "number optional" }),
            serde_json::json!({ "matches": "ranked clusters with title, rank, post count, authors, and cluster url id" }),
        ),
        "story_highlights" => (
            "Summarize notable AI 1000 posts and replies for a story or post URL.",
            serde_json::json!({ "clusterUrlId": "string optional", "postUrl": "string optional", "handle": "string optional" }),
            serde_json::json!({ "highlights": "quoted or paraphrased notable posts with author rank and source URLs" }),
        ),
        "pipeline_status" => (
            "Read public Digg AI ingestion status and recent pipeline events.",
            serde_json::json!({ "watch": "boolean optional", "since": "string optional duration" }),
            serde_json::json!({ "status": "isFetching, nextFetchAt, storiesToday, clustersToday, recent events" }),
        ),
        _ => (
            "Handle a focused Digg AI read-only query.",
            serde_json::json!({ "query": "string/object; focused request payload" }),
            serde_json::json!({ "ok": "boolean", "data": "Digg AI result payload", "sources": "citations" }),
        ),
    };

    WorkerFunctionPlan {
        function_id: format!("{}::{}", namespace, function),
        purpose: purpose.into(),
        side_effects: SideEffects::ExternalCall,
        inputs,
        output,
    }
}

fn render_worker_source(plan: &WorkerPlan) -> String {
    let reused_workers = json_string_array(&plan.uses_workers);
    let registrations = plan
        .functions
        .iter()
        .map(|function| {
            format!(
                r#"    iii.register_function(RegisterFunction::new("{function_id}", |payload: serde_json::Value| -> Result<serde_json::Value, String> {{
        Ok(serde_json::json!({{
            "ok": true,
            "functionId": "{function_id}",
            "payload": payload,
            "reusedWorkers": {reused_workers},
            "todo": "implement {purpose}"
        }}))
    }}).description("{purpose}"));
"#,
                function_id = function.function_id,
                purpose = function.purpose,
                reused_workers = reused_workers
            )
        })
        .collect::<String>();

    format!(
        r#"use iii_sdk::{{register_worker, InitOptions, RegisterFunction, RegisterServiceMessage}};

fn main() {{
    let engine_url = std::env::var("III_URL").unwrap_or_else(|_| "ws://localhost:49134".to_string());
    let iii = register_worker(&engine_url, InitOptions::default());
    iii.register_service(RegisterServiceMessage {{
        id: "{worker_name}".into(),
        name: "{worker_name}".into(),
        description: Some("Generated artifact-cli Rust iii worker".into()),
        parent_service_id: None,
    }});
{registrations}    println!("{worker_name} registered functions against {{engine_url}}");
    std::thread::park();
    iii.shutdown();
}}
"#,
        worker_name = plan.worker_name,
        registrations = registrations
    )
}

fn render_worker_cargo(plan: &WorkerPlan) -> String {
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
iii-sdk = "0.11.6"
schemars = "0.8"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#,
        plan.worker_name
    )
}

fn render_worker_readme(plan: &WorkerPlan) -> String {
    let functions = plan
        .functions
        .iter()
        .map(|function| format!("- `{}` — {}", function.function_id, function.purpose))
        .collect::<Vec<_>>()
        .join("\n");
    let reuse = render_reuse_markdown(plan);
    format!(
        "# {}\n\nGenerated by artifact-cli as a narrow Rust iii worker.\n\n## Goal\n\n{}\n\n## Functions\n\n{}\n\n## Reused iii Workers\n\n{}\n",
        plan.worker_name, plan.goal, functions, reuse
    )
}

fn render_reuse_markdown(plan: &WorkerPlan) -> String {
    let rows = plan
        .reuse_plan
        .engine_builtins
        .iter()
        .chain(plan.reuse_plan.installable_workers.iter())
        .map(|worker| {
            let install = worker.install.as_deref().unwrap_or("built into iii engine");
            format!(
                "| `{}` | {} | {} | `{}` |",
                worker.name, worker.source, worker.purpose, install
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    if rows.is_empty() {
        "No reusable iii workers selected.".into()
    } else {
        format!(
            "| Worker | Source | Why | Install |\n|---|---|---|---|\n{}",
            rows
        )
    }
}

fn render_worker_yaml(plan: &WorkerPlan) -> String {
    let functions = plan
        .functions
        .iter()
        .map(|function| {
            format!(
                "  - id: {}\n    sideEffects: {}\n",
                function.function_id,
                side_effects_label(&function.side_effects)
            )
        })
        .collect::<String>();
    let dependencies = plan
        .reuse_plan
        .installable_workers
        .iter()
        .map(|worker| {
            format!(
                "  - name: {}\n    source: {}\n    install: {}\n",
                worker.name,
                worker.source,
                worker.install.as_deref().unwrap_or("iii worker add")
            )
        })
        .collect::<String>();
    let dependencies = if dependencies.is_empty() {
        "[]\n".into()
    } else {
        format!("\n{}", dependencies)
    };

    format!(
        "name: {}\nversion: 0.1.0\nruntime: rust\ndescription: Narrow artifact worker generated by artifact-cli.\nfunctions:\n{}dependencies: {}",
        plan.worker_name, functions, dependencies
    )
}

fn side_effects_label(side_effects: &SideEffects) -> &'static str {
    match side_effects {
        SideEffects::Read => "read",
        SideEffects::Write => "write",
        SideEffects::Sync => "sync",
        SideEffects::ExternalCall => "external-call",
    }
}

fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", values)
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_sep = false;
        } else if !last_was_sep && !out.is_empty() {
            out.push('_');
            last_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "artifact".into()
    } else {
        out
    }
}

fn titleize(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
