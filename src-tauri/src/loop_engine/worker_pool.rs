use crate::api::llm_client::LlmClient;
use std::collections::HashMap;

pub struct WorkerPool {
    workers: HashMap<String, LlmClient>,
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerPool {
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
        }
    }

    pub fn add_worker(&mut self, id: String, client: LlmClient) {
        self.workers.insert(id, client);
    }

    pub fn get_worker(&self, id: &str) -> Option<&LlmClient> {
        self.workers.get(id)
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
}
