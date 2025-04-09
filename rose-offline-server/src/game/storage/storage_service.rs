use std::sync::Arc;
use bevy::prelude::*;
use anyhow::Result;

use crate::game::storage::{
    account::AccountStorage,
    bank::BankStorage,
    character::CharacterStorage,
    clan::ClanStorage,
    storage_adapter::StorageAdapter,
};

#[derive(Resource)]
pub struct StorageService {
    adapter: Arc<dyn StorageAdapter>,
}

impl StorageService {
    pub fn new(adapter: Arc<dyn StorageAdapter>) -> Self {
        Self { adapter }
    }

    // Account operations
    pub async fn create_account(&self, account: &AccountStorage) -> Result<()> {
        info!("STORAGE SERVICE: Creating account {} using adapter {:?}", &account.name, self.adapter);
        self.adapter.create_account(account).await
    }

    pub async fn load_account(&self, name: &str, password_hash: &str) -> Result<Option<AccountStorage>> {
        self.adapter.load_account(name, password_hash).await
    }

    pub async fn save_account(&self, account: &AccountStorage) -> Result<()> {
        self.adapter.save_account(account).await
    }

    // Character operations
    pub async fn create_character(&self, character: &CharacterStorage) -> Result<()> {
        self.adapter.create_character(character).await
    }

    pub async fn load_character(&self, name: &str) -> Result<Option<CharacterStorage>> {
        self.adapter.load_character(name).await
    }

    pub async fn save_character(&self, character: &CharacterStorage) -> Result<()> {
        info!("STORAGE SERVICE: Saving character {} using adapter {:?}", &character.info.name, self.adapter);
        self.adapter.save_character(character).await
    }

    pub async fn delete_character(&self, name: &str) -> Result<()> {
        self.adapter.delete_character(name).await
    }

    pub async fn character_exists(&self, name: &str) -> Result<bool> {
        self.adapter.character_exists(name).await
    }

    // Bank operations
    pub async fn create_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        self.adapter.create_bank(account_name, bank).await
    }

    pub async fn load_bank(&self, account_name: &str) -> Result<Option<BankStorage>> {
        self.adapter.load_bank(account_name).await
    }

    pub async fn save_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        info!("STORAGE SERVICE: Saving bank for account {} using adapter {:?}", account_name, self.adapter);
        self.adapter.save_bank(account_name, bank).await
    }

    // Clan operations
    pub async fn create_clan(&self, clan: &ClanStorage) -> Result<()> {
        self.adapter.create_clan(clan).await
    }

    pub async fn load_clan(&self, name: &str) -> Result<Option<ClanStorage>> {
        self.adapter.load_clan(name).await
    }

    pub async fn save_clan(&self, clan: &ClanStorage) -> Result<()> {
        self.adapter.save_clan(clan).await
    }

    pub async fn load_clan_list(&self) -> Result<Vec<ClanStorage>> {
        self.adapter.load_clan_list().await
    }

    pub async fn clan_exists(&self, name: &str) -> Result<bool> {
        self.adapter.clan_exists(name).await
    }
}