use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{error, info};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres, Row};
use std::sync::Arc;

use crate::game::storage::{
    account::{AccountStorage, AccountStorageError},
    bank::BankStorage,
    character::CharacterStorage,
    clan::ClanStorage,
    storage_adapter::StorageAdapter,
};

#[derive(Debug)]
pub struct PostgresStorageAdapter {
    pool: Pool<Postgres>,
}

impl PostgresStorageAdapter {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(connection_string)
            .await
            .context("Failed to connect to PostgreSQL database")?;
        
        let adapter = Self { pool };
        adapter.init().await?;
        
        Ok(adapter)
    }
}

#[async_trait]
impl StorageAdapter for PostgresStorageAdapter {
    async fn init(&self) -> Result<()> {
        info!("Initializing PostgreSQL storage adapter");
        
        // Create tables if they don't exist
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS accounts (
                name TEXT PRIMARY KEY,
                password_md5_sha256 TEXT NOT NULL,
                character_names JSONB NOT NULL
            );"#
        )
        .execute(&self.pool)
        .await
        .context("Failed to create accounts table")?;
    
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS characters (
                name TEXT PRIMARY KEY,
                data JSONB NOT NULL
            );"#
        )
        .execute(&self.pool)
        .await
        .context("Failed to create characters table")?;
    
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS banks (
                account_name TEXT PRIMARY KEY,
                data JSONB NOT NULL
            );"#
        )
        .execute(&self.pool)
        .await
        .context("Failed to create banks table")?;
    
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS clans (
                name TEXT PRIMARY KEY,
                data JSONB NOT NULL
            );"#
        )
        .execute(&self.pool)
        .await
        .context("Failed to create clans table")?;
    
        info!("PostgreSQL storage adapter initialized successfully");
        Ok(())
    }

    // Account operations
    async fn create_account(&self, account: &AccountStorage) -> Result<()> {
        info!("STORAGE DEBUG: PostgreSQL adapter creating account {}", &account.name);
        sqlx::query(
            r#"
            INSERT INTO accounts (name, password_md5_sha256, character_names)
            VALUES ($1, $2, $3)
            "#
        )
        .bind(&account.name)
        .bind(&account.password_md5_sha256)
        .bind(json!(account.character_names))
        .execute(&self.pool)
        .await
        .context("Failed to create account")?;
        
        Ok(())
    }

    async fn load_account(&self, name: &str, password_hash: &str) -> Result<Option<AccountStorage>> {
        let result = sqlx::query(
            r#"
            SELECT name, password_md5_sha256, character_names
            FROM accounts
            WHERE name = $1
            "#
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load account")?;
        
        match result {
            Some(row) => {
                let db_name: String = row.try_get("name")?;
                let db_password: String = row.try_get("password_md5_sha256")?;
                if db_password != password_hash {
                    return Err(AccountStorageError::InvalidPassword.into());
                }
                
                let character_names: Vec<String> = serde_json::from_value(row.try_get("character_names")?)?;
                
                Ok(Some(AccountStorage {
                    name: db_name,
                    password_md5_sha256: db_password,
                    character_names,
                }))
            },
            None => Ok(None),
        }
    }

    async fn save_account(&self, account: &AccountStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO accounts (name, password_md5_sha256, character_names)
            VALUES ($1, $2, $3)
            ON CONFLICT (name)
            DO UPDATE SET
                password_md5_sha256 = $2,
                character_names = $3
            "#
        )
        .bind(&account.name)
        .bind(&account.password_md5_sha256)
        .bind(json!(account.character_names))
        .execute(&self.pool)
        .await
        .context("Failed to save account")?;
        
        Ok(())
    }

    // Character operations
    async fn create_character(&self, character: &CharacterStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO characters (name, data)
            VALUES ($1, $2)
            "#
        )
        .bind(&character.info.name)
        .bind(json!(character))
        .execute(&self.pool)
        .await
        .context("Failed to create character")?;
        
        Ok(())
    }

    async fn load_character(&self, name: &str) -> Result<Option<CharacterStorage>> {
        let result = sqlx::query(
            r#"
            SELECT data
            FROM characters
            WHERE name = $1
            "#
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load character")?;
        
        match result {
            Some(row) => {
                let data: serde_json::Value = row.try_get("data")?;
                let character: CharacterStorage = serde_json::from_value(data)?;
                Ok(Some(character))
            },
            None => Ok(None),
        }
    }

    async fn save_character(&self, character: &CharacterStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO characters (name, data)
            VALUES ($1, $2)
            ON CONFLICT (name)
            DO UPDATE SET data = $2
            "#
        )
        .bind(&character.info.name)
        .bind(json!(character))
        .execute(&self.pool)
        .await
        .context("Failed to save character")?;
        
        Ok(())
    }

    async fn delete_character(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM characters WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to delete character")?;
            
        Ok(())
    }

    async fn character_exists(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("SELECT EXISTS(SELECT 1 FROM characters WHERE name = $1) as exists")
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .context("Failed to check if character exists")?;
            
        let exists: bool = result.try_get("exists")?;
        Ok(exists)
    }

    // Bank operations
    async fn create_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO banks (account_name, data)
            VALUES ($1, $2)
            "#
        )
        .bind(account_name)
        .bind(json!(bank))
        .execute(&self.pool)
        .await
        .context("Failed to create bank")?;
        
        Ok(())
    }

    async fn load_bank(&self, account_name: &str) -> Result<Option<BankStorage>> {
        let result = sqlx::query(
            r#"
            SELECT data
            FROM banks
            WHERE account_name = $1
            "#
        )
        .bind(account_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load bank")?;
        
        match result {
            Some(row) => {
                let data: serde_json::Value = row.try_get("data")?;
                let bank: BankStorage = serde_json::from_value(data)?;
                Ok(Some(bank))
            },
            None => Ok(None),
        }
    }

    async fn save_bank(&self, account_name: &str, bank: &BankStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO banks (account_name, data)
            VALUES ($1, $2)
            ON CONFLICT (account_name)
            DO UPDATE SET data = $2
            "#
        )
        .bind(account_name)
        .bind(json!(bank))
        .execute(&self.pool)
        .await
        .context("Failed to save bank")?;
        
        Ok(())
    }

    // Clan operations
    async fn create_clan(&self, clan: &ClanStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO clans (name, data)
            VALUES ($1, $2)
            "#
        )
        .bind(&clan.name)
        .bind(json!(clan))
        .execute(&self.pool)
        .await
        .context("Failed to create clan")?;
        
        Ok(())
    }

    async fn load_clan(&self, name: &str) -> Result<Option<ClanStorage>> {
        let result = sqlx::query(
            r#"
            SELECT data
            FROM clans
            WHERE name = $1
            "#
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load clan")?;
        
        match result {
            Some(row) => {
                let data: serde_json::Value = row.try_get("data")?;
                let clan: ClanStorage = serde_json::from_value(data)?;
                Ok(Some(clan))
            },
            None => Ok(None),
        }
    }

    async fn save_clan(&self, clan: &ClanStorage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO clans (name, data)
            VALUES ($1, $2)
            ON CONFLICT (name)
            DO UPDATE SET data = $2
            "#
        )
        .bind(&clan.name)
        .bind(json!(clan))
        .execute(&self.pool)
        .await
        .context("Failed to save clan")?;
        
        Ok(())
    }

    async fn load_clan_list(&self) -> Result<Vec<ClanStorage>> {
        let rows = sqlx::query(
            r#"
            SELECT data
            FROM clans
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to load clan list")?;
        
        let mut clans = Vec::with_capacity(rows.len());
        for row in rows {
            let data: serde_json::Value = row.try_get("data")?;
            let clan: ClanStorage = serde_json::from_value(data)?;
            clans.push(clan);
        }
        
        Ok(clans)
    }

    async fn clan_exists(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("SELECT EXISTS(SELECT 1 FROM clans WHERE name = $1) as exists")
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .context("Failed to check if clan exists")?;
            
        let exists: bool = result.try_get("exists")?;
        Ok(exists)
    }
}