use artifact_cli::{
    artifact_manifest, generate_worker, inspect_artifact, install_plan, plan_worker,
    registered_function_ids, verify_worker, worker_catalog, worker_metadata, worker_recipes,
    ArtifactInput, RecipeStage, SourceType, VerifyWorkerInput,
};

#[test]
fn plans_narrow_worker_functions_from_explicit_artifact_input() {
    let input = ArtifactInput {
        name: "hackernews".into(),
        goal: Some("give agents focused access to top stories and item lookup".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://github.com/HackerNews/API".into()),
        functions: vec![
            "top_stories".into(),
            "get_item".into(),
            "search_cached_stories".into(),
        ],
        output_dir: None,
    };

    let plan = plan_worker(input.clone()).unwrap();

    assert_eq!(plan.worker_name, "hackernews-worker");
    assert_eq!(plan.namespace, "hackernews");
    assert_eq!(plan.functions.len(), 3);
    assert_eq!(plan.functions[0].function_id, "hackernews::top_stories");
    assert!(plan.uses_workers.contains(&"iii-sandbox".to_string()));
    assert!(plan.uses_workers.contains(&"iii-database".to_string()));
    assert!(plan
        .reuse_plan
        .installable_workers
        .iter()
        .any(|worker| worker.name == "mcp"));
}

#[test]
fn inspect_artifact_recommends_narrow_not_generic_wrapper() {
    let input = ArtifactInput {
        name: "github repo".into(),
        goal: Some("repo and pull request risk checks".into()),
        source_type: Some(SourceType::OpenApi),
        source: None,
        functions: vec![],
        output_dir: None,
    };

    let inspected = inspect_artifact(input).unwrap();

    assert_eq!(inspected.namespace, "github_repo");
    assert!(inspected.recommendation.contains("narrow iii worker"));
    assert!(inspected
        .suggested_functions
        .iter()
        .any(|id| id == "github_repo::stale_prs"));
    assert!(inspected
        .reuse_plan
        .installable_workers
        .iter()
        .any(|worker| worker.name == "auth-credentials"));
}

#[test]
fn infers_hackernews_functions_from_name_before_source_url_noise() {
    let input = ArtifactInput {
        name: "hackernews".into(),
        goal: Some("give agents focused access to top stories and item lookup".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://github.com/HackerNews/API".into()),
        functions: vec![],
        output_dir: None,
    };

    let plan = plan_worker(input).unwrap();

    assert_eq!(plan.functions[0].function_id, "hackernews::top_stories");
    assert!(plan
        .functions
        .iter()
        .any(|function| function.function_id == "hackernews::get_item"));
}

#[test]
fn infers_narrow_digg_worker_from_artifact_source() {
    let input = ArtifactInput {
        name: "digg".into(),
        goal: Some("rank lookup, top stories, story highlights, and pipeline status".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://di.gg/ai".into()),
        functions: vec![],
        output_dir: None,
    };

    let plan = plan_worker(input).unwrap();

    assert_eq!(plan.worker_name, "digg-worker");
    assert_eq!(plan.functions.len(), 5);
    assert!(plan
        .functions
        .iter()
        .any(|function| function.function_id == "digg::author_rank"
            && function.purpose.contains("AI 1000")));
    assert!(plan
        .functions
        .iter()
        .any(|function| function.function_id == "digg::pipeline_status"));
    assert!(plan.uses_workers.contains(&"iii-database".to_string()));
    assert!(plan.uses_workers.contains(&"iii-rest".to_string()));
    assert!(!plan.uses_workers.contains(&"auth-credentials".to_string()));
}

#[test]
fn recipe_catalog_seeds_core_worker_targets() {
    let recipes = worker_recipes();

    for name in [
        "digg",
        "hackernews",
        "producthunt",
        "linear",
        "github_repo",
        "stripe",
        "arxiv",
        "wikipedia",
        "sentry",
        "slack",
        "notion",
        "openrouter",
    ] {
        assert!(recipes.iter().any(|recipe| recipe.name == name), "{name}");
    }
}

#[test]
fn recipe_catalog_is_prioritized_by_readiness() {
    let recipes = worker_recipes();
    let hackernews = recipes
        .iter()
        .find(|recipe| recipe.name == "hackernews")
        .unwrap();
    assert_eq!(hackernews.stage, RecipeStage::BuildNow);
    assert!(hackernews.priority >= 90);
    assert!(hackernews.integration.contains("public read-only"));
    assert!(hackernews
        .research_links
        .iter()
        .any(|link| link == "https://github.com/HackerNews/API"));

    let producthunt = recipes
        .iter()
        .find(|recipe| recipe.name == "producthunt")
        .unwrap();
    assert_eq!(producthunt.stage, RecipeStage::ResearchFirst);
    assert!(producthunt.integration.contains("GraphQL"));
    assert!(producthunt.rationale.contains("research"));

    let linear = recipes
        .iter()
        .find(|recipe| recipe.name == "linear")
        .unwrap();
    assert_eq!(linear.stage, RecipeStage::ResearchFirst);
    assert!(linear
        .research_links
        .iter()
        .any(|link| link == "https://linear.app/docs/api/"));
}

#[test]
fn infers_producthunt_and_linear_from_recipe_catalog() {
    let producthunt = plan_worker(ArtifactInput {
        name: "producthunt".into(),
        goal: Some("daily launch tracking and maker lookup".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://www.producthunt.com".into()),
        functions: vec![],
        output_dir: None,
    })
    .unwrap();
    assert!(producthunt
        .functions
        .iter()
        .any(|function| function.function_id == "producthunt::launch_metrics"));

    let linear = plan_worker(ArtifactInput {
        name: "linear".into(),
        goal: Some("blocked issues and cycle risk".into()),
        source_type: Some(SourceType::OpenApi),
        source: Some("https://linear.app".into()),
        functions: vec![],
        output_dir: None,
    })
    .unwrap();
    assert!(linear
        .functions
        .iter()
        .any(|function| function.function_id == "linear::cycle_risk"));
}

#[test]
fn manifest_matches_old_artifact_manifest_function_surface() {
    let input = ArtifactInput {
        name: "hackernews".into(),
        goal: Some("focused agent access to top stories".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://github.com/HackerNews/API".into()),
        functions: vec!["top_stories".into(), "get_item".into()],
        output_dir: None,
    };

    let manifest = artifact_manifest(input).unwrap();

    assert_eq!(manifest.schema, "artifact-cli.manifest.preview.v1");
    assert_eq!(manifest.worker_name, "hackernews-worker");
    assert_eq!(manifest.functions.len(), 2);
    assert!(manifest.uses_workers.contains(&"iii-sandbox".to_string()));
    assert!(manifest
        .reuse_plan
        .engine_builtins
        .iter()
        .any(|worker| worker.name == "iii-state"));
}

#[test]
fn exposes_the_same_artifact_function_ids_as_iii_primitives() {
    assert_eq!(
        registered_function_ids(),
        vec![
            "artifact::catalog",
            "artifact::recipes",
            "artifact::inspect",
            "artifact::plan_worker",
            "artifact::generate_worker",
            "artifact::verify_worker",
            "artifact::manifest",
        ]
    );

    let metadata = worker_metadata();
    assert_eq!(metadata.runtime, "rust");
    assert_eq!(metadata.name, "artifact-cli-worker");
}

#[test]
fn catalog_exposes_engine_builtins_and_installable_workers() {
    let catalog = worker_catalog();

    assert!(catalog
        .engine_builtins
        .iter()
        .any(|worker| worker.name == "iii-state"));
    assert!(catalog
        .installable_workers
        .iter()
        .any(|worker| worker.name == "shell-bash"
            && worker.functions.contains(&"shell::bash::exec".to_string())));
    assert!(catalog
        .installable_workers
        .iter()
        .any(|worker| worker.name == "iii-database"
            && worker.install.as_deref() == Some("iii worker add iii-database")));
}

#[test]
fn generates_and_verifies_rust_worker_scaffold_using_iii_sdk_apis() {
    let tmp = tempfile::tempdir().unwrap();
    let input = ArtifactInput {
        name: "hackernews".into(),
        goal: Some("focused agent access to top stories".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://github.com/HackerNews/API".into()),
        functions: vec!["top_stories".into(), "get_item".into()],
        output_dir: Some(tmp.path().to_path_buf()),
    };

    let generated = generate_worker(input).unwrap();
    assert!(generated.worker_path.ends_with("src/main.rs"));
    assert!(generated.manifest_path.ends_with("artifact.manifest.json"));
    assert!(tmp.path().join("iii.worker.yaml").exists());

    let worker_source = std::fs::read_to_string(&generated.worker_path).unwrap();
    assert!(worker_source.contains("use iii_sdk::{register_worker, InitOptions, RegisterFunction"));
    assert!(worker_source.contains("iii.register_function(RegisterFunction::new"));
    assert!(!worker_source.contains("// iii.register_function"));
    assert!(!worker_source.contains("//!"));
    assert!(worker_source.contains("\"reusedWorkers\""));

    let worker_yaml = std::fs::read_to_string(tmp.path().join("iii.worker.yaml")).unwrap();
    assert!(worker_yaml.contains("dependencies:"));
    assert!(worker_yaml.contains("iii worker add shell-filesystem"));
    assert!(worker_yaml.contains("iii worker add mcp"));

    let worker_readme = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
    assert!(worker_readme.contains("Reused iii Workers"));
    assert!(worker_readme.contains("iii-hq/workers"));

    let verified = verify_worker(VerifyWorkerInput {
        output_dir: tmp.path().to_path_buf(),
    })
    .unwrap();
    assert!(verified.ok, "missing: {:?}", verified.missing_registrations);
    assert_eq!(verified.function_count, 2);
    assert!(verified.missing_files.is_empty());

    let install = install_plan(tmp.path()).unwrap();
    assert!(install.ok);
    assert_eq!(install.worker_name, "hackernews-worker");
    assert!(install
        .commands
        .iter()
        .any(|command| command == "iii worker add shell-filesystem"));
    assert!(install
        .commands
        .iter()
        .any(|command| command.contains("cargo build --release")));
}

#[test]
fn generated_digg_worker_has_live_highlight_handlers() {
    let tmp = tempfile::tempdir().unwrap();
    let input = ArtifactInput {
        name: "digg".into(),
        goal: Some("focused Digg AI story highlights".into()),
        source_type: Some(SourceType::Docs),
        source: Some("https://di.gg/ai".into()),
        functions: vec!["story_highlights".into(), "top_stories".into()],
        output_dir: Some(tmp.path().to_path_buf()),
    };

    let generated = generate_worker(input).unwrap();
    let worker_source = std::fs::read_to_string(&generated.worker_path).unwrap();
    assert!(worker_source.contains("format_digg_thread_highlights"));
    assert!(worker_source.contains("readable_text"));
    assert!(worker_source.contains("extract_status_links"));
    assert!(worker_source.contains("fetch_text"));
    assert!(worker_source.contains("std::time::Duration::from_secs(12)"));
    assert!(!worker_source.contains("\"todo\": \"implement"));

    let worker_cargo = std::fs::read_to_string(tmp.path().join("Cargo.toml")).unwrap();
    assert!(worker_cargo.contains("ureq"));

    let verified = verify_worker(VerifyWorkerInput {
        output_dir: tmp.path().to_path_buf(),
    })
    .unwrap();
    assert!(verified.ok);
}
