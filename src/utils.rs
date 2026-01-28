use crate::models::PodOption;
use kube::{Client, Api, api::ListParams, config::Config};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use inquire::MultiSelect;
use indicatif::{ProgressBar, ProgressStyle};
use futures::future::join_all;
use colored::*;
use std::sync::Arc;
use tokio::sync::Semaphore;
use k8s_openapi::api::apps::v1::Deployment;


// --- SHARED SPINNER ---
pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb
}

// --- SHARED NAMESPACE LOGIC ---
pub async fn get_selected_namespaces(
    client: Client, 
    arg: Option<Option<String>>
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    match arg {
        None => {
            let config = Config::infer().await?;
            let current_ns = config.default_namespace.clone();
            println!("Using context namespace: {}", current_ns.cyan());
            Ok(vec![current_ns])            
        }
        Some(None) => {
            let pb = create_spinner("Fetching namespaces...");
            let ns_api: Api<Namespace> = Api::all(client);
            let ns_list = ns_api.list(&ListParams::default()).await?;
            pb.finish_and_clear();

            let ns_options: Vec<String> = ns_list.items.into_iter()
                .filter_map(|n| n.metadata.name).collect();
            Ok(MultiSelect::new("Select Namespaces:", ns_options).prompt()?)
        }
        Some(Some(ns)) => Ok(vec![ns]),
    }
}

// --- SHARED POD FETCHING (PARALLEL) ---
pub async fn fetch_all_pods(
    client: Client, 
    namespaces: Vec<String>
) -> Result<Vec<PodOption>, Box<dyn std::error::Error + Send + Sync>> {
    let pb = create_spinner("Fetching pods...");
    let mut tasks = Vec::new();
    let semaphore = Arc::new(Semaphore::new(8));

    for ns in namespaces {
        let c = client.clone();
        let sem = semaphore.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore closed");
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
    Ok(all_pods)
}

pub async fn fetch_all_deployments(
    client: Client,
    namespaces: Vec<String>,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let pb = create_spinner("Fetching deployments...");
    let mut all_deploys = Vec::new();

    for ns in namespaces {
        let api: Api<Deployment> = Api::namespaced(client.clone(), &ns);
        let list = api.list(&ListParams::default()).await?;
        for d in list.items {
            if let Some(name) = d.metadata.name {
                all_deploys.push(name);
            }
        }
    }
    pb.finish_and_clear();
    Ok(all_deploys)
}
