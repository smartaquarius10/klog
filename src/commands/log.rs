use std::collections::VecDeque;

use crate::models::{LogMessage, PodOption};
use crate::utils;
use colored::*;
use futures::{AsyncBufReadExt, StreamExt};
use inquire::{MultiSelect, Select};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, api::LogParams};
use regex::Regex;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{self},
};
use std::io::{stdout, Write};
use std::time::Duration;

pub async fn run(
    client: Client,
    pod_arg: Option<String>,
    deploy_arg: Option<Option<String>>,
    namespace_arg: Option<Option<String>>,
    container_select: bool,
    filter: Option<String>,
    exclude: Option<String>,
    previous: bool,
    tail: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // 1. Resolve Namespaces
    let selected_ns = utils::get_selected_namespaces(client.clone(), namespace_arg).await?;

    // 2. Resolve PodOptions (All paths lead to a Vec<PodOption>)
    let mut pod_options: Vec<PodOption> = Vec::new();

    if let Some(pod_name) = pod_arg {
        // --- Path A: Direct Pod Name ---
        // We fetch the pod specifically to get its container list for the -c logic
        let pods_api: Api<Pod> = Api::namespaced(client.clone(), &selected_ns[0]);
        let p = pods_api.get(&pod_name).await?;
        let containers = p.spec.map(|s| s.containers.into_iter().map(|c| c.name).collect()).unwrap_or_default();
        
        pod_options.push(PodOption { 
            name: pod_name, 
            namespace: selected_ns[0].clone(), 
            containers 
        });
    } 
    else if let Some(deploy_opt) = deploy_arg {
        // --- Path B: Deployment Mode ---
        let deploy_name = match deploy_opt {
            Some(name) => name,
            None => {
                let deploys = utils::fetch_all_deployments(client.clone(), selected_ns.clone()).await?;
                inquire::Select::new("Select deployment to tail:", deploys).prompt()?
            }
        };
        
        // Find pods by label (app=name)
        let lp = kube::api::ListParams::default().labels(&format!("app={}", deploy_name));
        for ns in &selected_ns {
            let api: Api<Pod> = Api::namespaced(client.clone(), ns);
            let pods = api.list(&lp).await?;
            for p in pods.items {
                let name = p.metadata.name.clone().unwrap_or_default();
                let containers = p.spec.map(|s| s.containers.into_iter().map(|c| c.name).collect()).unwrap_or_default();
                pod_options.push(PodOption { name, namespace: ns.clone(), containers });
            }
        }
    } 
    else {
        // --- Path C: Standard Interactive Menu ---
        let available_pods = utils::fetch_all_pods(client.clone(), selected_ns).await?;
        pod_options = MultiSelect::new("Select Pods to tail:", available_pods).prompt()?;
    }

    if pod_options.is_empty() {
        println!("{}", "No pods found matching your selection.".yellow());
        return Ok(());
    }

    // 3. Resolve Containers (This converts Vec<PodOption> -> Vec<(PodOption, String)>)
    // This handles your -c logic for ALL paths automatically.
    let final_targets = pick_pods_and_containers(pod_options, container_select).await?;

    // 4. Start Streaming
    start_log_stream(client, final_targets, filter, exclude, previous, tail).await?;

    Ok(())
}

// USER SELECTION LOGIC
async fn pick_pods_and_containers(
    selected_pods: Vec<PodOption>,
    force_container_select: bool,
) -> Result<Vec<(PodOption, String)>, Box<dyn std::error::Error + Send + Sync>> {
    // let selected_pods = MultiSelect::new("Select Pods to tail:", all_pods).prompt()?;
    let mut final_targets = Vec::new();

    for p in selected_pods {
        let container = if force_container_select && p.containers.len() > 1 {
            Select::new(
                &format!("Select container for {}:", p.name),
                p.containers.clone(),
            )
            .prompt()?
        } else {
            p.containers
                .first()
                .cloned()
                .unwrap_or_else(|| "default".to_string())
        };
        final_targets.push((p, container));
    }
    Ok(final_targets)
}

fn draw_footer() {
    // Get the current terminal size
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    
    // 1. Prepare the text we want to show
    let footer_text = " [s] Search History | [q] Quit ";
    
    // 2. Calculate how much space is left to fill the whole line
    // We use .chars().count() because emojis like 🔍 count as 1 char but multiple bytes
    let text_len = footer_text.chars().count();
    let padding = if cols as usize > text_len {
        " ".repeat(cols as usize - text_len)
    } else {
        "".to_string()
    };

    // 3. Draw the bar
    execute!(
        stdout(),
        cursor::SavePosition,               // Remember where the log was
        cursor::MoveTo(0, rows - 1),        // Jump to the very last line
    ).unwrap();

    // Print the text + the padding to fill the background to the end of the screen
    print!("{}{}", 
        footer_text.on_white().black(), 
        padding.on_white()
    );
    
    execute!(stdout(), cursor::RestorePosition).unwrap(); // Jump back to the log line
    let _ = stdout().flush();
}

async fn start_log_stream(
    client: Client,
    targets: Vec<(PodOption, String)>,
    filter: Option<String>,
    exclude: Option<String>,
    previous: bool,
    tail: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let mut history: VecDeque<LogMessage> = VecDeque::with_capacity(1000);

    // Spawn workers (Same as before)
    for (pod, container) in targets {
        let (tx_c, client_c, tail_c) = (tx.clone(), client.clone(), tail.clone());
        tokio::spawn(async move {
            let _ = tail_logs(client_c, pod, container, tx_c, previous, tail_c).await;
        });
    }
    drop(tx);

    // --- 1. ENTER RAW MODE ---
    terminal::enable_raw_mode()?;
    draw_footer();

    let filter_regex = filter.as_ref().map(|f| Regex::new(f).unwrap());
    let exclude_regex = exclude.as_ref().map(|e| Regex::new(e).unwrap());

    loop {
        tokio::select! {
            Some(log) = rx.recv() => {
                if history.len() >= 1000 { history.pop_front(); }
                history.push_back(log.clone());

                if let Some(re) = &exclude_regex { if re.is_match(&log.message) { continue; } }
                if let Some(re) = &filter_regex { if !re.is_match(&log.message) { continue; } }

                // --- 2. PRINT LOG WITH CARRIAGE RETURN ---
                print_log_line(&log);
                draw_footer(); // Keep the footer at the bottom
            }

            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        // Only handle Press events (ignores release events on Windows)
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') => break,
                                
                                KeyCode::Char('s') => {
                                    // --- 3. TEMPORARILY EXIT RAW MODE FOR SEARCH ---
                                    terminal::disable_raw_mode()?;
                                    println!("\n{}", " --- ⏸️  STREAM PAUSED --- ".on_yellow().black());

                                    let query = inquire::Text::new("Search history:").prompt()?;
                                    let matches: Vec<LogMessage> = history.iter()
                                        .filter(|h| h.message.to_lowercase().contains(&query.to_lowercase()))
                                        .cloned().collect();

                                    if !matches.is_empty() {
                                        // Use our custom help message here
                                        let _ = Select::new("Search Results:", matches)
                                            .with_help_message("↑↓ to scroll through history, Enter to return to live logs")
                                            .prompt();
                                    }

                                    println!("{}", " ---  RESUMING --- ".on_green().black());
                                    
                                    // RE-ENTER RAW MODE
                                    terminal::enable_raw_mode()?;
                                    draw_footer();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // --- 4. CLEANUP ---
    cleanup_terminal();
    Ok(())
}

fn print_log_line(log: &LogMessage) {
    let prefix_text = format!("[{}/{}]", log.pod_name, log.container_name);
    let prefix = match log.pod_name.len() % 4 {
        0 => prefix_text.cyan(),
        1 => prefix_text.green(),
        2 => prefix_text.magenta(),
        _ => prefix_text.yellow(),
    }.bold();
    
    // In RAW mode, we need \r\n to start at the beginning of the next line
    print!("\r{} {}\n", prefix, log.message);
    let _ = stdout().flush();
}

async fn tail_logs(
    client: Client,
    pod: PodOption,
    container: String,
    tx: tokio::sync::mpsc::Sender<LogMessage>,
    previous: bool,
    tail: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pods: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let tail_setting: Option<i64> = if tail == "*" {
        None
    } else {
        match tail.parse::<i64>() {
            Ok(num) => Some(num),
            Err(_) => {
                eprintln!("⚠️  Invalid tail value '{}', defaulting to 50", tail);
                Some(50)
            }
        }
    };
    let lp = LogParams {
        follow: true,
        tail_lines: tail_setting,
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
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn cleanup_terminal() {
    let _ = terminal::disable_raw_mode();
    let (_, rows) = terminal::size().unwrap_or((80, 24));
    
    // Jump to the bottom line and clear it entirely
    execute!(
        stdout(),
        cursor::MoveTo(0, rows - 1),
        terminal::Clear(terminal::ClearType::CurrentLine)
    ).unwrap();
    
    // Ensure the cursor is visible and moved to a new line so the prompt is clean
    println!("\r"); 
}