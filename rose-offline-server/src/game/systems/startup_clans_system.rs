use bevy::prelude::{Commands, Res};
use log::{error,info};
use tokio::runtime::Runtime;

use rose_data::QuestTriggerHash;
use rose_game_common::components::ClanUniqueId;

use crate::game::{
    components::{Clan, ClanMember, Level},
    storage::{StorageService},
};

pub fn startup_clans_system(mut commands: Commands, storage_service: Res<StorageService>) {
    // Create a static runtime for async operations
    static CLAN_RUNTIME: once_cell::sync::Lazy<Runtime> = 
        once_cell::sync::Lazy::new(|| Runtime::new().expect("Failed to create clan runtime"));

    // Load all clans using the StorageService
    let clans = CLAN_RUNTIME.block_on(async {
        match storage_service.load_clan_list().await {
            Ok(clans) => {
                info!("Successfully loaded {} clans", clans.len());
                clans
            },
            Err(err) => {
                error!("Failed to load clan list: {:?}", err);
                Vec::new()
            }
        }
    });

    for clan_storage in clans {
        info!("Loading clan: {}", clan_storage.name);
        let mut members = Vec::new();

        for member in clan_storage.members {
            // Load each character using the StorageService
            let character_result = CLAN_RUNTIME.block_on(async {
                storage_service.load_character(&member.name).await
            });

            match character_result {
                Ok(Some(character)) => {
                    members.push(ClanMember::Offline {
                        name: member.name,
                        position: member.position,
                        contribution: member.contribution,
                        level: Level::new(character.level.level),
                        job: character.info.job,
                    });
                }
                Ok(None) => {
                    error!("Character {} not found for clan {}", member.name, clan_storage.name);
                }
                Err(err) => {
                    error!("Failed to load character {} for clan {}: {:?}", 
                        member.name, clan_storage.name, err);
                }
            }
        }

        commands.spawn(Clan {
            unique_id: ClanUniqueId::new(QuestTriggerHash::from(clan_storage.name.as_str()).hash)
                .unwrap(),
            name: clan_storage.name,
            description: clan_storage.description,
            mark: clan_storage.mark,
            money: clan_storage.money,
            points: clan_storage.points,
            level: clan_storage.level,
            skills: clan_storage.skills,
            members,
        });
    }
}