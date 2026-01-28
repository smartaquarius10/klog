mod models;
mod commands;
pub mod utils; 
use clap::{Parser, Subcommand};
use kube::Client; // Import Client
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser)]
// Add these attributes to enable auto-generation of help/version
#[command(name = "klog", about = "K8s Diagnostic Tool",author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Tail logs from pods
    Log {
        /// Pass target namespace.
        /// If -n is passed without a value, uses current context. 
        /// If -n is missing, shows interactive menu.
        #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
        namespace: Option<Option<String>>,
        /// If passed, will ask to select containers for each pod else defaults to first container
        #[arg(short, long, default_value_t = false)]
        container_select: bool,
        /// Only show lines matching this regex
        #[arg(short, long)]
        filter: Option<String>,
        /// Hide lines matching this regex (e.g. -e "healthz")
        #[arg(short, long)]
        exclude: Option<String>,
        /// If passed, will tail previous terminated container logs
        #[arg(short, long, default_value_t = false)]
        previous: bool,
    },
    /// Summarized diagnostic of a pod's health and recent events
    Describe {
        /// Target pod name (optional, will show menu if missing)
        #[arg(short, long)]
        pod: Option<String>,
        /// Target namespace.
        /// If -n is passed without a value, uses current context. 
        /// If -n is missing, shows interactive menu.
        #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
        namespace: Option<Option<String>>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    // 1. Initialize Crypto
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // 2. Start Spinner for Initialization
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);
    pb.set_message("Initializing Kubernetes client...");
    pb.enable_steady_tick(std::time::Duration::from_millis(120));

    // 3. Initialize Client ONCE
    let client = Client::try_default().await?;
    pb.finish_and_clear();    

    match cli.command {
        Commands::Log { namespace, container_select, filter, exclude,previous } => {
            commands::log::run(client.clone(), namespace, container_select, filter, exclude, previous).await?;
        }
        Commands::Describe { pod, namespace } => {
            // New command!
            commands::describe::run(client.clone(), pod, namespace).await?;
        }
    }
    Ok(())
}