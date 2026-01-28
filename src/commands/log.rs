use crate::models::{PodOption, LogMessage};
use crate::utils;
use colored::*;
use futures::{AsyncBufReadExt, StreamExt};
use inquire::{MultiSelect, Select};
use k8s_openapi::api::core::v1::{Pod};
use kube::{api::LogParams, Api, Client};
use regex::Regex;

pub async fn run(
    client: Client,
    namespace_arg: Option<Option<String>>,
    container_select: bool,
    filter: Option<String>,
    exclude: Option<String>,
    previous: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let selected_ns = utils::get_selected_namespaces(client.clone(), namespace_arg).await?;

    let all_pods = utils::fetch_all_pods(client.clone(), selected_ns).await?;

    let targets = pick_pods_and_containers(all_pods, container_select).await?;

    start_log_stream(client, targets, filter, exclude, previous).await?;

    Ok(())
}

// USER SELECTION LOGIC 
async fn pick_pods_and_containers(
    all_pods: Vec<PodOption>, 
    force_container_select: bool
) -> Result<Vec<(PodOption, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let selected_pods = MultiSelect::new("Select Pods to tail:", all_pods).prompt()?;
    let mut final_targets = Vec::new();

    for p in selected_pods {
        let container = if force_container_select && p.containers.len() > 1 {
            Select::new(&format!("Select container for {}:", p.name), p.containers.clone()).prompt()?
        } else {
            p.containers.first().cloned().unwrap_or_else(|| "default".to_string())
        };
        final_targets.push((p, container));
    }
    Ok(final_targets)
}

// --- STEP 4: STREAMING LOGIC ---
async fn start_log_stream(
    client: Client,
    targets: Vec<(PodOption, String)>,
    filter: Option<String>,
    exclude: Option<String>,
    previous: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    for (pod, container) in targets {
        let tx_c = tx.clone();
        let client_c = client.clone();
        tokio::spawn(async move {
            let _ = tail_logs(client_c, pod, container, tx_c, previous).await;
        });
    }
    drop(tx);

    println!("\n--- Streaming Logs ---\n");
    let filter_regex = filter.as_ref().map(|f| Regex::new(f).unwrap());
    let exclude_regex = exclude.as_ref().map(|e| Regex::new(e).unwrap());

    while let Some(log) = rx.recv().await {
        if let Some(re) = &exclude_regex { if re.is_match(&log.message) { continue; } }
        if let Some(re) = &filter_regex { if !re.is_match(&log.message) { continue; } }
        
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

async fn tail_logs(
    client: Client,
    pod: PodOption,
    container: String,
    tx: tokio::sync::mpsc::Sender<LogMessage>,
    previous: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pods: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let lp = LogParams {
        follow: true,
        tail_lines: Some(10),
        container: Some(container.clone()),
        previous: previous,
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