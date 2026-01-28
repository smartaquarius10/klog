use crate::models::PodOption;
use crate::utils;
use kube::{Client, Api, api::ListParams};
use k8s_openapi::api::core::v1::{Pod, Event};
use colored::*;
use comfy_table::Table;

pub async fn run(
    client: Client,
    pod_arg: Option<String>,
    namespace_arg: Option<Option<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    // 1. Get the target pod (reusing your utils)
    let target = match pod_arg {
        Some(name) => {
            let ns = utils::get_selected_namespaces(client.clone(), namespace_arg).await?;
            PodOption { name, namespace: ns[0].clone(), containers: vec![] }
        },
        None => {
            let ns = utils::get_selected_namespaces(client.clone(), namespace_arg).await?;
            let pods = utils::fetch_all_pods(client.clone(), ns).await?;
            inquire::Select::new("Select pod to describe:", pods).prompt()?
        }
    };

    // 2. Fetch Data (Pod + Events)
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), &target.namespace);
    let event_api: Api<Event> = Api::namespaced(client.clone(), &target.namespace);
    
    let p = pod_api.get(&target.name).await?;
    
    // Filter events specifically for this pod
    let lp = ListParams::default().fields(&format!("involvedObject.name={}", target.name));
    let events = event_api.list(&lp).await?;

    // --- 3. PRINT VITAL SIGNS ---
    println!("\n{}", "--- POD VITAL SIGNS ---".bold().bright_white());
    
    let mut vitals = Table::new();
    vitals.set_header(vec!["Property", "Value"]);

    // Status & Age
    let status = p.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_default();
    let color_status = if status == "Running" { status.green() } else { status.red() };
    vitals.add_row(vec!["Status", &color_status.to_string()]);
    
    if let Some(ip) = p.status.as_ref().and_then(|s| s.pod_ip.clone()) {
        vitals.add_row(vec!["Pod IP", &ip]);
    }

    // Resource Limits (CPU/Mem)
    if let Some(spec) = p.spec.as_ref() {
        if let Some(container) = spec.containers.first() {
            if let Some(resources) = &container.resources {
                let cpu = resources.limits.as_ref().and_then(|l| l.get("cpu")).map(|v| v.0.clone()).unwrap_or("None".into());
                let mem = resources.limits.as_ref().and_then(|l| l.get("memory")).map(|v| v.0.clone()).unwrap_or("None".into());
                vitals.add_row(vec!["Limits", &format!("CPU: {}, Mem: {}", cpu, mem)]);
            }
        }
    }
    println!("{vitals}");

    // --- 4. PRINT RECENT EVENTS (Clean Table) ---
    println!("\n{}", "--- RECENT EVENTS ---".bold().bright_white());
    if events.items.is_empty() {
        println!("   (No recent events)");
    } else {
        let mut event_table = Table::new();
        event_table.set_header(vec!["Type", "Reason", "Message"]);
        
        // Show only the last 5 events
        for e in events.items.iter().rev().take(5) {
            let row_type = if e.type_ == Some("Warning".into()) { "Warning".red() } else { "Normal".green() };
            event_table.add_row(vec![
                row_type.to_string(),
                e.reason.clone().unwrap_or_default(),
                e.message.clone().unwrap_or_default(),
            ]);
        }
        println!("{event_table}");
    }

    Ok(())
}