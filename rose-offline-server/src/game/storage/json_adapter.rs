use anyhow::Result;
use async_trait::async_trait;
use log::info;
use std::{path::Path, sync::RwLock};

use crate::game::storage::{
    account::AccountStorage,
    bank::BankStorage,
    character::CharacterStorage,
    clan::ClanStorage,
    storage_adapter::StorageAdapter,
    ACCOUNT_STORAGE_DIR, BANK_STORAGE_DIR, CHARACTER_STORAGE_DIR, CLAN_STORAGE_DIR,
};

#[derive(Debug)]
pub struct JsonStorageAdapter {
    initialized: RwLock<bool>,
}

impl JsonStorageAdapter {
    pub fn new() -> Self {
        Self {
            initialized: RwLock::new(false),
        }
    }

    fn ensure_dir_exists(path: &Path) -> Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[async_trait]
impl StorageAdapter for JsonStorageAdapter {
    async fn init(&self) -> Result<()> {
        let mut initialized = self.initialized.write().unwrap();
        if *initialized {
            return Ok(());
        }

        info!("Initializing JSON storage adapter");
        
        Self::ensure_dir_exists(&*ACCOUNT_STORAGE_DIR)?;
        Self::ensure_dir_exists(&*BANK_STORAGE_DIR)?;
        Self::ensure_dir_exists(&*CHARACTER_STORAGE_DIR)?;
        Self::ensure_dir_exists(&*CLAN_STORAGE_DIR)?;
        
        *initialized = true;
        Ok(())
    }

    async fn create_account(&self, account: &AccountStorage) -> Result<()> {
        account.save()
    }

    async fn load_account(&self, name: &str, password_hash: &str) -> Result<Option<AccountStorage>> {
        let password = rose_game_common::data::Password::Md5(password_hash.to_string());
        match AccountStorage::try_load(name, &password) {
            Ok(account) => Ok(Some(account)),
            Err(_) => Ok(None),
        }
    }

    async fn save_account(&self, account: &AccountStorage) -> Result<()> {
        account.save()
    }

    async fn create_character(&self, character: &CharacterStorage) -> Result<()> {
        character.save()
    }

    async fn load_character(&self, name: &str) -> Result<Option<CharacterStorage>> {
        match CharacterStorage::try_load(name) {
            Ok(character) => Ok(Some(character)),
            Err(_) => Ok(None),
        }
    }

    async fn save_character(&self, character: &CharacterStorage) -> Result<()> {
        character.save()
    }

    async fn delete_character(&self, name: &str) -> Result<()> {
        let path = CHARACTER_STORAGE_DIR.join(format!("{}.json", name));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    async fn character_exists(&self, name: &str) -> Result<bool> {
        let path = CHARACTER_STORAGE_DIR.join(format!("{}.json", name));
        Ok(path.exists())
    }

    async fn create_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        bank.save(account_name)
    }

    async fn load_bank(&self, account_name: &str) -> Result<Option<BankStorage>> {
        match BankStorage::try_load(account_name) {
            Ok(bank) => Ok(Some(bank)),
            Err(_) => Ok(None),
        }
    }

    async fn save_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        bank.save(account_name)
    }

    async fn create_clan(&self, clan: &ClanStorage) -> Result<()> {
        clan.try_create()
    }

    async fn load_clan(&self, name: &str) -> Result<Option<ClanStorage>> {
        match ClanStorage::try_load(name) {
            Ok(clan) => Ok(Some(clan)),
            Err(_) => Ok(None),
        }
    }

    async fn save_clan(&self, clan: &ClanStorage) -> Result<()> {
        clan.save()
    }

    async fn load_clan_list(&self) -> Result<Vec<ClanStorage>> {
        ClanStorage::try_load_clan_list()
    }

    async fn clan_exists(&self, name: &str) -> Result<bool> {
        Ok(ClanStorage::exists(name))
    }
}