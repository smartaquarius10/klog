use kube::{Client, Api, api::ListParams};
use k8s_openapi::api::core::v1::Pod;

pub async fn get_pods(client: Client, namespace: &str) -> anyhow::Result<Vec<Pod>> {
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}