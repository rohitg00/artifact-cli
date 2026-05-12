use artifact_cli::{
    artifact_manifest, generate_worker, init_options, inspect_artifact, plan_worker,
    register_artifact_primitives, verify_worker, worker_catalog, worker_recipes, ArtifactInput,
    SourceType, VerifyWorkerInput,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "artifact-cli-worker")]
#[command(about = "Rust-first artifact-cli iii worker utility")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Catalog,
    Recipes,
    Inspect(ArtifactArgs),
    Plan(ArtifactArgs),
    Manifest(ArtifactArgs),
    Generate(GenerateArgs),
    Verify {
        #[arg(long)]
        output_dir: PathBuf,
    },
    Serve {
        #[arg(long, env = "III_URL", default_value = "ws://localhost:49134")]
        iii_url: String,
    },
}

#[derive(Debug, Parser)]
struct ArtifactArgs {
    #[arg(long)]
    payload: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    goal: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long, default_value = "docs")]
    source_type: String,
    #[arg(long, value_delimiter = ',')]
    function: Vec<String>,
}

#[derive(Debug, Parser)]
struct GenerateArgs {
    #[arg(long)]
    payload: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    goal: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long, default_value = "docs")]
    source_type: String,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long, value_delimiter = ',')]
    function: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Catalog => {
            println!("{}", serde_json::to_string_pretty(&worker_catalog())?);
        }
        Command::Recipes => {
            println!("{}", serde_json::to_string_pretty(&worker_recipes())?);
        }
        Command::Inspect(args) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&inspect_artifact(args.input()?)?)?
            );
        }
        Command::Plan(args) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&plan_worker(args.input()?)?)?
            );
        }
        Command::Manifest(args) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&artifact_manifest(args.input()?)?)?
            );
        }
        Command::Generate(args) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&generate_worker(args.input()?)?)?
            );
        }
        Command::Verify { output_dir } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&verify_worker(VerifyWorkerInput { output_dir })?)?
            );
        }
        Command::Serve { iii_url } => {
            let iii = iii_sdk::register_worker(&iii_url, init_options());
            let refs = register_artifact_primitives(&iii);
            eprintln!(
                "artifact-cli-worker registered {} artifact::* iii functions against {}",
                refs.len(),
                iii_url
            );
            std::thread::park();
            iii.shutdown();
        }
    }
    Ok(())
}

impl ArtifactArgs {
    fn input(self) -> anyhow::Result<ArtifactInput> {
        artifact_input(
            self.payload,
            self.name,
            self.goal,
            self.source,
            self.source_type,
            self.function,
            None,
        )
    }
}

impl GenerateArgs {
    fn input(self) -> anyhow::Result<ArtifactInput> {
        artifact_input(
            self.payload,
            self.name,
            self.goal,
            self.source,
            self.source_type,
            self.function,
            self.output_dir,
        )
    }
}

fn artifact_input(
    payload: Option<PathBuf>,
    name: Option<String>,
    goal: Option<String>,
    source: Option<String>,
    source_type: String,
    functions: Vec<String>,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<ArtifactInput> {
    if let Some(path) = payload {
        let mut input: ArtifactInput = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        if output_dir.is_some() {
            input.output_dir = output_dir;
        }
        return Ok(input);
    }

    Ok(ArtifactInput {
        name: name.ok_or_else(|| anyhow::anyhow!("--name is required unless --payload is set"))?,
        goal,
        source_type: Some(parse_source_type(&source_type)?),
        source,
        functions,
        output_dir,
    })
}

fn parse_source_type(value: &str) -> anyhow::Result<SourceType> {
    match value {
        "openapi" | "open_api" => Ok(SourceType::OpenApi),
        "graphql" => Ok(SourceType::Graphql),
        "har" => Ok(SourceType::Har),
        "mcp" => Ok(SourceType::Mcp),
        "docs" => Ok(SourceType::Docs),
        "url" => Ok(SourceType::Url),
        "manual" => Ok(SourceType::Manual),
        other => Err(anyhow::anyhow!("unsupported --source-type: {other}")),
    }
}
