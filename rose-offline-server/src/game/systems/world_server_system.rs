use bevy::{
    ecs::prelude::{Commands, Entity, Query, Res, ResMut, Without},
    prelude::EventWriter,
};
use log::warn;
use tokio::runtime::Runtime;
use once_cell::sync::Lazy;

use rose_game_common::data::Password;

use crate::game::{
    components::{Account, CharacterDeleteTime, CharacterList, ServerInfo, WorldClient},
    events::ClanEvent,
    messages::{
        client::ClientMessage,
        server::{CharacterListItem, ConnectionRequestError, CreateCharacterError, ServerMessage},
    },
    resources::{GameData, LoginTokens},
    storage::{
        account::{AccountStorage, AccountStorageError},
        character::CharacterStorage,
        StorageService,
    },
};

// Create a static runtime for async calls
static WORLD_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create world runtime")
});

fn handle_world_connection_request(
    commands: &mut Commands,
    login_tokens: &mut LoginTokens,
    entity: Entity,
    world_client: &mut WorldClient,
    token_id: u32,
    password: &Password,
    storage_service: &StorageService,
) -> Result<u32, ConnectionRequestError> {
    let login_token = login_tokens
        .get_token_mut(token_id)
        .ok_or(ConnectionRequestError::InvalidToken)?;
    if login_token.world_client.is_some() || login_token.game_client.is_some() {
        return Err(ConnectionRequestError::InvalidToken);
    }

    // Verify account password using StorageService
    let password_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(password.to_md5());
        hex::encode(hasher.finalize())
    };

    let account = WORLD_RUNTIME.block_on(async {
        match storage_service.load_account(&login_token.username, &password_hash).await {
            Ok(Some(account_storage)) => Ok(account_storage),
            Ok(None) => Err(ConnectionRequestError::InvalidPassword),
            Err(error) => {
                log::error!("Failed to load account {} with error {:?}", 
                    &login_token.username, error);
                
                // Check if it's specifically an invalid password error
                if let Some(AccountStorageError::InvalidPassword) = error.downcast_ref::<AccountStorageError>() {
                    Err(ConnectionRequestError::InvalidPassword)
                } else {
                    Err(ConnectionRequestError::Failed)
                }
            }
        }
    })?;

    // Load character list, deleting any characters ready for deletion
    let mut character_list = CharacterList::default();
    let mut valid_character_names = Vec::new();

    for name in &account.character_names {
        let character_result = WORLD_RUNTIME.block_on(async {
            storage_service.load_character(name).await
        });

        match character_result {
            Ok(Some(character)) => {
                if character
                    .delete_time
                    .as_ref()
                    .map(|x| x.get_time_until_delete())
                    .filter(|x| x.as_nanos() == 0)
                    .is_some()
                {
                    // Character delete time expired, delete it
                    match WORLD_RUNTIME.block_on(async {
                        storage_service.delete_character(&character.info.name).await
                    }) {
                        Ok(_) => log::info!("Deleted character {} as delete timer has expired.", &character.info.name),
                        Err(error) => log::error!("Failed to delete character {} with error {:?}", &character.info.name, error),
                    }
                } else {
                    character_list.push(character);
                    valid_character_names.push(name.clone());
                }
            }
            Ok(None) => {
                log::error!("Character {} not found", name);
            }
            Err(error) => {
                log::error!("Failed to load character {} with error {:?}", name, error);
            }
        }
    }

    // Update account character list if any characters were deleted
    if account.character_names.len() != valid_character_names.len() {
        let mut updated_account = account.clone();
        updated_account.character_names = valid_character_names;
        
        WORLD_RUNTIME.block_on(async {
            match storage_service.save_account(&updated_account).await {
                Ok(_) => {},
                Err(error) => log::error!("Failed to update account after character deletion: {:?}", error),
            }
        });
    }

    // Update entity
    commands
        .entity(entity)
        .insert(Account::from(account))
        .insert(character_list);

    // Update token
    login_token.world_client = Some(entity);
    world_client.login_token = login_token.token;
    world_client.selected_game_server = Some(login_token.selected_game_server);

    Ok(123)
}

pub fn world_server_authentication_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut WorldClient), Without<Account>>,
    mut login_tokens: ResMut<LoginTokens>,
    storage_service: Res<StorageService>,
) {
    query.for_each_mut(|(entity, mut world_client)| {
        if let Ok(message) = world_client.client_message_rx.try_recv() {
            match message {
                ClientMessage::ConnectionRequest {
                    login_token,
                    password,
                } => {
                    let response = match handle_world_connection_request(
                        &mut commands,
                        login_tokens.as_mut(),
                        entity,
                        world_client.as_mut(),
                        login_token,
                        &password,
                        &storage_service,
                    ) {
                        Ok(packet_sequence_id) => {
                            ServerMessage::ConnectionRequestSuccess { packet_sequence_id }
                        }
                        Err(error) => ServerMessage::ConnectionRequestError { error },
                    };
                    world_client.server_message_tx.send(response).ok();
                }
                _ => panic!("Received unexpected client message {:?}", message),
            }
        }
    });
}

pub fn world_server_system(
    mut world_client_query: Query<(&mut WorldClient, &mut Account, &mut CharacterList)>,
    server_info_query: Query<&ServerInfo>,
    mut login_tokens: ResMut<LoginTokens>,
    game_data: Res<GameData>,
    mut clan_events: EventWriter<ClanEvent>,
    storage_service: Res<StorageService>,
) {
    world_client_query.for_each_mut(|(world_client, mut account, mut character_list)| {
        if let Ok(message) = world_client.client_message_rx.try_recv() {
            match message {
                ClientMessage::GetCharacterList => {
                    world_client
                        .server_message_tx
                        .send(ServerMessage::CharacterList {
                            character_list: character_list
                                .iter()
                                .map(|character| CharacterListItem {
                                    info: character.info.clone(),
                                    level: character.level,
                                    delete_time: character.delete_time,
                                    equipment: character.equipment.clone(),
                                })
                                .collect(),
                        })
                        .ok();
                }
                ClientMessage::CreateCharacter {
                    gender,
                    hair,
                    face,
                    name,
                    birth_stone,
                    ..
                } => {
                    let response = if account.character_names.len() >= 5 {
                        ServerMessage::CreateCharacterError {
                            error: CreateCharacterError::NoMoreSlots,
                        }
                    } else if name.len() < 4 || name.len() > 20 {
                        ServerMessage::CreateCharacterError {
                            error: CreateCharacterError::InvalidValue,
                        }
                    } else {
                        // Check if character exists using the storage service
                        let char_exists = WORLD_RUNTIME.block_on(async {
                            storage_service.character_exists(&name).await
                        }).unwrap_or(true);  // Default to true on error to avoid name collision

                        if char_exists {
                            ServerMessage::CreateCharacterError {
                                error: CreateCharacterError::AlreadyExists,
                            }
                        } else {
                            match game_data.character_creator.create(
                                name.clone(),
                                gender,
                                birth_stone as u8,
                                face as u8,
                                hair as u8,
                            ) {
                                Ok(character) => {
                                    // Save character using storage service
                                    let save_result = WORLD_RUNTIME.block_on(async {
                                        storage_service.create_character(&character).await
                                    });

                                    if let Err(error) = save_result {
                                        log::error!(
                                            "Failed to create character {} with error {:?}",
                                            &name,
                                            error
                                        );
                                        ServerMessage::CreateCharacterError {
                                            error: CreateCharacterError::Failed,
                                        }
                                    } else {
                                        let character_slot = account.character_names.len();
                                        account.character_names.push(character.info.name.clone());
                                        
                                        // Save account using storage service
                                        WORLD_RUNTIME.block_on(async {
                                            let account_storage = AccountStorage::from(&*account);
                                            storage_service.save_account(&account_storage).await.ok()
                                        });
                                        
                                        character_list.push(character);
                                        ServerMessage::CreateCharacterSuccess { character_slot }
                                    }
                                }
                                Err(error) => {
                                    log::error!(
                                        "Failed to create character {} with error {:?}",
                                        &name,
                                        error
                                    );
                                    ServerMessage::CreateCharacterError {
                                        error: CreateCharacterError::InvalidValue,
                                    }
                                }
                            }
                        }
                    };

                    world_client.server_message_tx.send(response).ok();
                }
                ClientMessage::DeleteCharacter {
                    slot,
                    name,
                    is_delete,
                } => {
                    let response = character_list
                        .get_mut(slot as usize)
                        .filter(|character| character.info.name == name)
                        .map_or_else(
                            || ServerMessage::DeleteCharacterError { name: name.clone() },
                            |character| {
                                if is_delete {
                                    if character.delete_time.is_none() {
                                        character.delete_time = Some(CharacterDeleteTime::new());
                                    }
                                } else {
                                    character.delete_time = None;
                                }

                                // Save character using storage service
                                WORLD_RUNTIME.block_on(async {
                                    match storage_service.save_character(character).await {
                                        Ok(_) => log::info!("Saved character {}", character.info.name),
                                        Err(error) => log::error!(
                                            "Failed to save character {} with error {:?}",
                                            character.info.name,
                                            error
                                        ),
                                    }
                                });

                                if let Some(delete_time) = character.delete_time {
                                    ServerMessage::DeleteCharacterStart {
                                        name: name.clone(),
                                        delete_time,
                                    }
                                } else {
                                    ServerMessage::DeleteCharacterCancel { name: name.clone() }
                                }
                            },
                        );
                    world_client.server_message_tx.send(response).ok();
                }
                ClientMessage::SelectCharacter { slot, name } => {
                    let response = character_list
                        .get_mut(slot as usize)
                        .filter(|character| character.info.name == name)
                        .map_or(ServerMessage::SelectCharacterError, |selected_character| {
                            // Set the selected_character for the login token
                            if let Some(token) = login_tokens
                                .tokens
                                .iter_mut()
                                .find(|t| t.token == world_client.login_token)
                            {
                                token.selected_character = selected_character.info.name.clone()
                            }

                            // Find the selected game server details
                            if let Some(selected_game_server) = world_client.selected_game_server {
                                if let Ok(server_info) = server_info_query.get(selected_game_server)
                                {
                                    ServerMessage::SelectCharacterSuccess {
                                        login_token: world_client.login_token,
                                        packet_codec_seed: server_info.packet_codec_seed,
                                        ip: server_info.ip.clone(),
                                        port: server_info.port,
                                    }
                                } else {
                                    ServerMessage::SelectCharacterError
                                }
                            } else {
                                ServerMessage::SelectCharacterError
                            }
                        });
                    world_client.server_message_tx.send(response).ok();
                }
                ClientMessage::ClanGetMemberList => {
                    if let Some(game_client_entity) = world_client.game_client_entity {
                        clan_events.send(ClanEvent::GetMemberList {
                            entity: game_client_entity,
                        });
                    }
                }
                _ => warn!("[WS] Received unimplemented client message {:?}", message),
            }
        }
    });
}