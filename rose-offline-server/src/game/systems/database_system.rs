use bevy::prelude::*;

use crate::game::storage::StorageService;

pub fn database_system(
    mut commands: Commands, 
    storage_service: Res<StorageService>,
) {
    info!("Database system initialized");
    // The storage service is already initialized and registered as a resource
    // This system just ensures it's loaded and accessible to other systems
}