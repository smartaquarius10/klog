mod commands;
mod models;
pub mod utils;

use clap::{Parser, Subcommand, CommandFactory};
use colored::*;
use kube::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor; // The engine for history and arrows

#[derive(Parser)]
#[command(name = "klog", author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Tail logs from pods
    Log {
        /// If specified, fetch logs from this pod only
        pod: Option<String>,
        #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
        /// If specified, fetch logs from pods in this deployment only
        deployment: Option<Option<String>>,
        /// If specified, list pods from this namespace only. If pass -n only then list namespaces to choose from. IF not specified, use current context namespace.
        #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
        namespace: Option<Option<String>>,
        /// If specified, prompt to select containers within pods
        #[arg(short, long, default_value_t = false)]
        container_select: bool,
        /// Filter logs by this string (regex supported)
        #[arg(short, long)]
        filter: Option<String>,
        /// Exclude logs by this string (regex supported)
        #[arg(short, long)]
        exclude: Option<String>,
        /// If specified, fetch previous logs
        #[arg(short, long, default_value_t = false)]
        previous: bool,
        /// Number of lines from the end of the logs to show. * for all
        #[arg(short, long, default_value = "50")]
        tail: String,
    },
    /// Summarized diagnostic of a pod's health
    Describe {
        #[arg(short, long)]
        pod: Option<String>,
        #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
        namespace: Option<Option<String>>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    inquire::set_global_render_config(crate::utils::get_transparent_theme());
    rustls::crypto::ring::default_provider().install_default().ok();

    // 2. Initial connection (Zscaler tax paid here once)
    let pb = crate::utils::create_spinner("Connecting to Kubernetes...");
    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            pb.finish_and_clear();
            eprintln!("❌ Connection Error: {e}");
            return Ok(());
        }
    };
    pb.finish_and_clear();

    // 3. Enter the Shell
    run_shell(client).await?;

    Ok(())
}

async fn execute(client: Client, cmd: Commands) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match cmd {
        Commands::Log { pod, deployment, namespace, container_select, filter, exclude, previous, tail } => {
            commands::log::run(client, pod, deployment, namespace, container_select, filter, exclude, previous, tail).await
        }
        Commands::Describe { pod, namespace } => {
            commands::describe::run(client, pod, namespace).await
        }
    }
}

async fn run_shell(client: Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", "\n--- 🐚 klog interactive shell ---".bright_white().bold());
    println!("Commands: 'log', 'describe', 'help', 'exit'. Up/Down for history.");

    // Initialize the history editor
    let mut rl = DefaultEditor::new()?;
    
    loop {
        // PROMPT: This replaces inquire::Text
        let readline = rl.readline("klog> ");

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() { continue; }
                if line == "exit" || line == "quit" { break; }
                
                // Add to history (So up-arrow works)
                let _ = rl.add_history_entry(line);

                if line == "help" {
                    let mut cmd = Cli::command();
                    let _ = cmd.print_help();
                    println!();
                    continue;
                }

                if let Some(mut parts) = shlex::split(line) {
                    parts.insert(0, "klog".to_string());

                    match Cli::try_parse_from(parts) {
                        Ok(cli) => {
                            if let Some(cmd) = cli.command {
                                // We print errors if the command fails
                                if let Err(e) = execute(client.clone(), cmd).await {
                                    eprintln!("{} {}", "Error:".red(), e);
                                }
                            }
                        }
                        Err(e) => println!("{}", e),
                    }
                }
            }
            Err(ReadlineError::Interrupted) => break, // Ctrl-C
            Err(ReadlineError::Eof) => break,         // Ctrl-D
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    
    Ok(())
}