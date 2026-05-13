use artifact_worker::{init_options, register_artifact_primitives};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "artifact-worker")]
#[command(about = "Artifact iii worker")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve {
        #[arg(long, env = "III_URL", default_value = "ws://localhost:49134")]
        iii_url: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { iii_url } => {
            let iii = iii_sdk::register_worker(&iii_url, init_options());
            let refs = register_artifact_primitives(&iii);
            eprintln!(
                "artifact-worker registered {} artifact::* iii functions against {}",
                refs.len(),
                iii_url
            );
            std::thread::park();
            iii.shutdown();
        }
    }
    Ok(())
}
