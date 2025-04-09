use std::sync::Arc;

use anyhow::{Context, Result};

use crate::game::storage::{
    json_adapter::JsonStorageAdapter, 
    postgres_adapter::PostgresStorageAdapter, 
    storage_adapter::StorageAdapter
};

#[derive(Clone, Debug)]
pub enum StorageBackend {
    Json,
    Postgres(String),
    JsonStorageAdapter,
    PostgresStorageAdapter(String),
}

impl StorageBackend {
    pub fn from_postgres_connection_string(connection_string: String) -> Self {
        Self::PostgresStorageAdapter(connection_string)
    }
}

impl Default for StorageBackend {
    fn default() -> Self {
        Self::Json
    }
}

pub struct StorageConfig {
    pub backend: StorageBackend,
}

impl StorageConfig {
    pub fn new(backend: StorageBackend) -> Self {
        Self { backend }
    }

    pub async fn create_adapter(&self) -> Result<Arc<dyn StorageAdapter>> {
        match &self.backend {
            StorageBackend::Json => {
                let adapter = JsonStorageAdapter::new();
                adapter.init().await?;
                Ok(Arc::new(adapter))
            }
            StorageBackend::Postgres(connection_string) => {
                let adapter = PostgresStorageAdapter::new(connection_string)
                    .await
                    .context("Failed to create PostgreSQL adapter")?;
                Ok(Arc::new(adapter))
            }
            StorageBackend::JsonStorageAdapter => {
                let adapter = JsonStorageAdapter::new();
                adapter.init().await?;
                Ok(Arc::new(adapter))
            }
            StorageBackend::PostgresStorageAdapter(connection_string) => {
                let adapter = PostgresStorageAdapter::new(connection_string)
                    .await
                    .context("Failed to create PostgreSQL adapter")?;
                Ok(Arc::new(adapter))
            }
        }
    }
}