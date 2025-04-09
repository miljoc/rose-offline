use bevy::prelude::Resource;
use crate::game::storage::StorageBackend;

#[derive(Clone, Resource)]
pub struct GameConfig {
    pub enable_npc_spawns: bool,
    pub enable_monster_spawns: bool,
    pub storage_backend: StorageBackend,
}

impl GameConfig {
    pub fn new() -> Self {
        Self {
            enable_npc_spawns: true,
            enable_monster_spawns: true,
            storage_backend: StorageBackend::default()
        }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self::new()
    }
}
