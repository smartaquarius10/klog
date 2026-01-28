use std::fmt;

#[derive(Clone)]
pub struct PodOption {
    pub name: String,
    pub namespace: String,
    pub containers: Vec<String>,
}

impl fmt::Display for PodOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.namespace)
    }
}

impl fmt::Display for LogMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}/{}] {}", self.pod_name, self.container_name, self.message)
    }
}

#[derive(Clone)]
pub struct LogMessage {
    pub pod_name: String,
    pub container_name: String,
    pub message: String,
}