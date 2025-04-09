use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

use crate::game::storage::{
    account::AccountStorage,
    bank::BankStorage,
    character::CharacterStorage,
    clan::ClanStorage,
};

/// Defines the interface for storage adapters
#[async_trait]
pub trait StorageAdapter: Send + Sync + Debug {
    /// Initialize the storage adapter
    async fn init(&self) -> Result<()>;

    // Account operations
    async fn create_account(&self, account: &AccountStorage) -> Result<()>;
    async fn load_account(&self, name: &str, password_hash: &str) -> Result<Option<AccountStorage>>;
    async fn save_account(&self, account: &AccountStorage) -> Result<()>;

    // Character operations
    async fn create_character(&self, character: &CharacterStorage) -> Result<()>;
    async fn load_character(&self, name: &str) -> Result<Option<CharacterStorage>>;
    async fn save_character(&self, character: &CharacterStorage) -> Result<()>;
    async fn delete_character(&self, name: &str) -> Result<()>;
    async fn character_exists(&self, name: &str) -> Result<bool>;

    // Bank operations
    async fn create_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()>;
    async fn load_bank(&self, account_name: &str) -> Result<Option<BankStorage>>;
    async fn save_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()>;

    // Clan operations
    async fn create_clan(&self, clan: &ClanStorage) -> Result<()>;
    async fn load_clan(&self, name: &str) -> Result<Option<ClanStorage>>;
    async fn save_clan(&self, clan: &ClanStorage) -> Result<()>;
    async fn load_clan_list(&self) -> Result<Vec<ClanStorage>>;
    async fn clan_exists(&self, name: &str) -> Result<bool>;
}