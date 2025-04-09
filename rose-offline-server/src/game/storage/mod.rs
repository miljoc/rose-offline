use std::path::PathBuf;
use std::sync::Arc;
use once_cell::sync::Lazy;

pub mod account;
pub mod bank;
pub mod clan;
pub mod character;
pub mod config;
pub mod storage_adapter;
pub mod json_adapter;
pub mod postgres_adapter;
pub mod storage_service;

pub use account::{AccountStorage, AccountStorageError};
pub use bank::BankStorage;
pub use character::CharacterStorage;
pub use clan::{ClanStorage, ClanStorageMember};
pub use config::StorageConfig;
pub use storage_service::StorageService;
pub use json_adapter::JsonStorageAdapter;
pub use postgres_adapter::PostgresStorageAdapter;
pub use storage_adapter::StorageAdapter;

#[derive(Clone, Debug)]
pub enum StorageBackend {
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
        StorageBackend::JsonStorageAdapter
    }
}

pub static ACCOUNT_STORAGE_DIR: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("data/accounts"));
pub static BANK_STORAGE_DIR: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("data/banks"));
pub static CLAN_STORAGE_DIR: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("data/clans"));
pub static CHARACTER_STORAGE_DIR: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("data/characters"));


