use clap::Parser;
use colored::*;
use futures::{future::join_all, AsyncBufReadExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{api::ListParams, api::LogParams, Api, Client};
use std::{fmt};
use regex::Regex;
use kube::config::Config;


// --- 1. DATA STRUCTURES ---

#[derive(Parser, Debug)]
struct Args {
    /// If passed, will ask to select containers for each pod
    #[arg(short, default_value_t = false)]
    container_select: bool,

    /// Only show lines matching this regex
    #[arg(short, long)]
    filter: Option<String>,

    /// Hide lines matching this regex (e.g. -e "healthz")
    #[arg(short, long)]
    exclude: Option<String>,

    ///  Pass target namespace.
    ///  If -n is passed without a value, uses current context. 
    /// If -n is missing, shows interactive menu.
    #[arg(short, long, num_args = 0..=1, default_missing_value = None)]
    namespace: Option<Option<String>>,
}

#[derive(Clone)]
struct PodOption {
    name: String,
    namespace: String,
    containers: Vec<String>,
}

// How pods look in the menu
impl fmt::Display for PodOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.namespace)
    }
}

// Data sent from background workers to the screen
struct LogMessage {
    pod_name: String,
    container_name: String,
    message: String,
}

// --- 2. THE BACKGROUND WORKER (TAILER) ---

async fn tail_logs(
    client: Client,
    pod: PodOption,
    container: String,
    tx: tokio::sync::mpsc::Sender<LogMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pods: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let lp = LogParams {
        follow: true,
        tail_lines: Some(10),
        container: Some(container.clone()),
        ..LogParams::default()
    };

    let log_stream = pods.log_stream(&pod.name, &lp).await?;
    let mut lines = log_stream.lines();

    while let Some(line_result) = lines.next().await {
        if let Ok(line) = line_result {
            let msg = LogMessage {
                pod_name: pod.name.clone(),
                container_name: container.clone(),
                message: line,
            };
            if tx.send(msg).await.is_err() { break; }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let args = Args::parse();

    // 1. START SPINNER IMMEDIATELY
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);
    pb.set_message("Initializing Kubernetes client...");
    pb.enable_steady_tick(std::time::Duration::from_millis(120));

    // 2. NOW CONNECT (The spinner will be visible during this slow part)
    let client = Client::try_default().await?;

    // 3. FETCH NAMESPACES
    let selected_ns = match args.namespace {
        // CASE 1: User typed nothing (no -n) -> Show Menu
        None => {
            pb.set_message("Fetching namespaces...");
            let ns_api: Api<Namespace> = Api::all(client.clone());
            let ns_list = ns_api.list(&ListParams::default()).await?;
            pb.finish_and_clear();

            let ns_options: Vec<String> = ns_list.items.into_iter()
                .filter_map(|n| n.metadata.name)
                .collect();
            MultiSelect::new("Select Namespaces:", ns_options).prompt()?
        }
        
        // CASE 2: User typed -n but no value -> Pick from Kubeconfig (kubens)
        Some(None) => {
            pb.set_message("Selecting namespace in current context...");
            let config = Config::infer().await?;
            let current_ns = config.default_namespace.clone();
            pb.finish_and_clear();
            println!("Using default namespace from context: {}", current_ns);
            vec![current_ns]
        }

        // CASE 3: User typed -n my-ns -> Use provided value
        Some(Some(ns)) => {
            pb.finish_and_clear();
            vec![ns]
        }
    };

    // 4. FETCH PODS (Start a new spinner)
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);
    pb.set_message("Fetching pods in parallel...");
    pb.enable_steady_tick(std::time::Duration::from_millis(120));

    let mut tasks = Vec::new();
    for ns in selected_ns {
        let c = client.clone();
        tasks.push(tokio::spawn(async move {
            let api: Api<Pod> = Api::namespaced(c, &ns);
            (ns, api.list(&ListParams::default()).await)
        }));
    }

    let results = join_all(tasks).await;
    let mut all_pods = Vec::new();

    for res in results {
        let (ns, pod_list) = res?;
        for p in pod_list?.items {
            let name = p.metadata.name.clone().unwrap_or_default();
            let containers = p.spec.map(|s| s.containers.into_iter().map(|c| c.name).collect()).unwrap_or_default();
            all_pods.push(PodOption { name, namespace: ns.clone(), containers });
        }
    }
    pb.finish_and_clear();

    // C. SELECT PODS AND CONTAINERS
    let selected_pods = MultiSelect::new("Select Pods to tail:", all_pods).prompt()?;
    let mut final_targets = Vec::new();

    for p in selected_pods {
        let container = if args.container_select && p.containers.len() > 1 {
            Select::new(&format!("Select container for {}:", p.name), p.containers.clone()).prompt()?
        } else {
            p.containers.first().cloned().unwrap_or_else(|| "default".to_string())
        };
        final_targets.push((p, container));
    }

    // D. START STREAMING
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    for (pod, container) in final_targets {
        let tx_c = tx.clone();
        let client_c = client.clone();
        tokio::spawn(async move {
            let _ = tail_logs(client_c, pod, container, tx_c).await;
        });
    }
    drop(tx); // Close the original sender

    println!("\n--- Streaming Logs ---\n");
    let filter_regex = args.filter.as_ref().map(|f| Regex::new(f).unwrap());
    let exclude_regex = args.exclude.as_ref().map(|e| Regex::new(e).unwrap());
    while let Some(log) = rx.recv().await {
         if let Some(re) = &exclude_regex {
            if re.is_match(&log.message) {
                continue;
            }
        }
         if let Some(re) = &filter_regex {
            if !re.is_match(&log.message) {
                continue;
            }
        }
        let prefix_text = format!("[{}/{}]", log.pod_name, log.container_name);
        let prefix = match log.pod_name.len() % 4 {
            0 => prefix_text.cyan(),
            1 => prefix_text.green(),
            2 => prefix_text.magenta(),
            _ => prefix_text.yellow(),
        }.bold();

        println!("{} {}", prefix, log.message);
    }

    Ok(())
}