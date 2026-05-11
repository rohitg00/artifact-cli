use artifact_cli::{
    artifact_manifest, generate_worker, init_options, inspect_artifact, install_plan, plan_worker,
    register_artifact_primitives, verify_worker, worker_catalog, ArtifactInput, SourceType,
    VerifyWorkerInput,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

#[derive(Debug, Parser)]
#[command(name = "artifact")]
#[command(about = "Generate, install, and call narrow iii workers from artifacts")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Catalog,
    From(SourceArgs),
    Inspect(SourceArgs),
    Manifest(SourceArgs),
    Generate(GenerateArgs),
    Install {
        worker_dir: PathBuf,
    },
    Verify {
        worker_dir: PathBuf,
    },
    Call {
        function_id: String,
        #[arg(long, default_value = "{}")]
        json: String,
    },
    Serve {
        #[arg(long, env = "III_URL", default_value = "ws://localhost:49134")]
        iii_url: String,
    },
}

#[derive(Debug, Parser)]
struct SourceArgs {
    source: Option<String>,
    #[arg(long)]
    payload: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    goal: Option<String>,
    #[arg(long, default_value = "docs")]
    source_type: String,
    #[arg(long, value_delimiter = ',')]
    function: Vec<String>,
}

#[derive(Debug, Parser)]
struct GenerateArgs {
    source: Option<String>,
    #[arg(long)]
    payload: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    goal: Option<String>,
    #[arg(long, default_value = "docs")]
    source_type: String,
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    #[arg(long, value_delimiter = ',')]
    function: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Catalog => print_json(&worker_catalog())?,
        Command::From(args) => print_json(&plan_worker(args.input(None)?)?)?,
        Command::Inspect(args) => print_json(&inspect_artifact(args.input(None)?)?)?,
        Command::Manifest(args) => print_json(&artifact_manifest(args.input(None)?)?)?,
        Command::Generate(args) => print_json(&generate_worker(args.input()?)?)?,
        Command::Install { worker_dir } => print_json(&install_plan(worker_dir)?)?,
        Command::Verify { worker_dir } => print_json(&verify_worker(VerifyWorkerInput {
            output_dir: worker_dir,
        })?)?,
        Command::Call { function_id, json } => call_iii(&function_id, &json)?,
        Command::Serve { iii_url } => serve_artifact_worker(&iii_url),
    }
    Ok(())
}

impl SourceArgs {
    fn input(self, output_dir: Option<PathBuf>) -> anyhow::Result<ArtifactInput> {
        artifact_input(
            self.payload,
            self.name,
            self.goal,
            self.source,
            self.source_type,
            self.function,
            output_dir,
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
            self.out,
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

    let name = match (name, source.as_deref()) {
        (Some(name), _) => name,
        (None, Some(source)) => infer_name_from_source(source),
        (None, None) => anyhow::bail!("provide a source or set --payload"),
    };

    Ok(ArtifactInput {
        name,
        goal,
        source_type: Some(parse_source_type(&source_type)?),
        source,
        functions,
        output_dir,
    })
}

fn infer_name_from_source(source: &str) -> String {
    let no_scheme = source
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let no_query = no_scheme.split(['?', '#']).next().unwrap_or(no_scheme);
    let parts = no_query
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.len() >= 3 && parts[0].contains("github.com") {
        return format!("{}-{}", parts[1], parts[2]);
    }

    parts
        .last()
        .copied()
        .filter(|part| !part.is_empty())
        .unwrap_or("artifact")
        .trim_end_matches(".json")
        .trim_end_matches(".yaml")
        .trim_end_matches(".yml")
        .to_string()
}

fn parse_source_type(value: &str) -> anyhow::Result<SourceType> {
    match value {
        "openapi" | "open_api" => Ok(SourceType::OpenApi),
        "graphql" => Ok(SourceType::Graphql),
        "har" => Ok(SourceType::Har),
        "docs" => Ok(SourceType::Docs),
        "url" => Ok(SourceType::Url),
        "manual" => Ok(SourceType::Manual),
        other => Err(anyhow::anyhow!("unsupported --source-type: {other}")),
    }
}

fn call_iii(function_id: &str, payload: &str) -> anyhow::Result<()> {
    serde_json::from_str::<serde_json::Value>(payload)?;
    let output = ProcessCommand::new("iii")
        .arg("trigger")
        .arg(format!("--function-id={function_id}"))
        .arg(format!("--payload={payload}"))
        .output()?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        anyhow::bail!("iii trigger failed with status {}", output.status);
    }
    Ok(())
}

fn serve_artifact_worker(iii_url: &str) {
    let iii = iii_sdk::register_worker(iii_url, init_options());
    let refs = register_artifact_primitives(&iii);
    eprintln!(
        "artifact registered {} artifact::* iii functions against {}",
        refs.len(),
        iii_url
    );
    std::thread::park();
    iii.shutdown();
}

fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
