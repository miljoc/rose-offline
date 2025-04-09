use anyhow::Result;
use log::info;
use std::sync::Arc;

use rose_game_common::data::Password;

use crate::game::storage::{
    account::{AccountStorage, AccountStorageError},
    bank::{BankStorage, BankStorageError},
    character::{CharacterCreator, CharacterCreatorError, CharacterStorage},
    clan::ClanStorage,
    storage_adapter::StorageAdapter,
};

/// StorageService provides a unified interface to storage operations using the configured adapter
pub struct StorageService {
    adapter: Arc<dyn StorageAdapter>,
}

impl StorageService {
    pub fn new(adapter: Arc<dyn StorageAdapter>) -> Self {
        Self { adapter }
    }

    // Account operations
    pub async fn create_account(&self, name: &str, password: &Password) -> Result<AccountStorage> {
        let account = AccountStorage {
            name: String::from(name),
            password_md5_sha256: self.hash_password(password),
            character_names: Vec::new(),
        };
        
        self.adapter.create_account(&account).await?;
        Ok(account)
    }

    pub async fn load_account(&self, name: &str, password: &Password) -> Result<AccountStorage> {
        let password_hash = self.hash_password(password);
        match self.adapter.load_account(name, &password_hash).await? {
            Some(account) => Ok(account),
            None => Err(AccountStorageError::NotFound.into()),
        }
    }

    pub async fn save_account(&self, account: &AccountStorage) -> Result<()> {
        self.adapter.save_account(account).await
    }

    // Character operations
    pub async fn create_character<C: CharacterCreator>(
        &self,
        creator: &C,
        account: &mut AccountStorage,
        name: String,
        gender: rose_game_common::components::CharacterGender,
        birth_stone: u8,
        face: u8,
        hair: u8,
    ) -> Result<CharacterStorage, CharacterCreatorError> {
        // Check if character exists using the adapter
        let exists = self.adapter.character_exists(&name).await
            .map_err(|_| CharacterCreatorError::InvalidName)?;
            
        if exists {
            return Err(CharacterCreatorError::InvalidName);
        }

        // Create the character
        let character = creator.create(name.clone(), gender, birth_stone, face, hair)?;
        
        // Save the character through the adapter
        self.adapter.create_character(&character).await
            .map_err(|_| CharacterCreatorError::InvalidName)?;

        // Update and save the account
        account.character_names.push(name);
        self.save_account(account).await
            .map_err(|_| CharacterCreatorError::InvalidName)?;

        Ok(character)
    }

    pub async fn load_character(&self, name: &str) -> Result<CharacterStorage> {
        match self.adapter.load_character(name).await? {
            Some(character) => Ok(character),
            None => Err(anyhow::anyhow!("Character not found")),
        }
    }

    pub async fn save_character(&self, character: &CharacterStorage) -> Result<()> {
        self.adapter.save_character(character).await
    }

    pub async fn delete_character(&self, name: &str) -> Result<()> {
        self.adapter.delete_character(name).await
    }

    pub async fn character_exists(&self, name: &str) -> Result<bool> {
        self.adapter.character_exists(name).await
    }

    // Bank operations
    pub async fn load_bank(&self, account_name: &str) -> Result<BankStorage> {
        match self.adapter.load_bank(account_name).await? {
            Some(bank) => Ok(bank),
            None => {
                let bank = BankStorage::default();
                self.adapter.create_bank(account_name, &bank).await?;
                Ok(bank)
            },
        }
    }

    pub async fn save_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
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

    // Utility methods
    fn hash_password(&self, password: &Password) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(password.to_md5());
        hex::encode(hasher.finalize())
    }
}