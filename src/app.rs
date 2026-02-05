use kube::Client;
use k8s_openapi::api::core::v1::Pod;

pub struct App {
    #[allow(dead_code)] 
    pub client: Client,
    pub namespace: String,
    pub should_quit: bool,
    pub pods: Vec<Pod>, // <--- NEW: Store the data here
}

impl App {
    pub async fn new() -> anyhow::Result<Self> {
        let config = kube::Config::infer().await?;
        let namespace = config.default_namespace.clone();
        let client = Client::try_from(config)?;
        
        // Initial Fetch (We will move this to background later)
        let pods = crate::k8s::get_pods(client.clone(), &namespace).await?;

        Ok(Self {
            client,
            namespace,
            should_quit: false,
            pods,
        })
    }
}