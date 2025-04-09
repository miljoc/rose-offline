use bevy::ecs::prelude::{Commands, Entity, Query, Res, ResMut, Without};
use log::{info, error, warn};
use tokio::runtime::Runtime;
use once_cell::sync::Lazy;

use crate::game::{
    components::{Account, LoginClient},
    messages::client::ClientMessage,
    messages::server::{ChannelListError, JoinServerError, LoginError, ServerMessage},
    resources::{LoginTokens, ServerList},
    storage::account::{AccountStorage, AccountStorageError},
    storage::StorageService,
};

// Create a static runtime for async calls
static LOGIN_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create login runtime")
});

pub fn login_server_authentication_system(
    mut commands: Commands,
    query: Query<(Entity, &LoginClient), Without<Account>>,
    login_tokens: Res<LoginTokens>,
    server_list: Res<ServerList>,
    storage_service: Res<StorageService>,
) {
    query.for_each(|(entity, login_client)| {
        if let Ok(message) = login_client.client_message_rx.try_recv() {
            match message {
                ClientMessage::ConnectionRequest { .. } => {
                    login_client
                        .server_message_tx
                        .send(ServerMessage::ConnectionRequestSuccess {
                            packet_sequence_id: 123,
                        })
                        .ok();
                }
                ClientMessage::LoginRequest { username, password } => {
                    let login_result = if login_tokens.find_username_token(&username).is_some() {
                        Err(LoginError::AlreadyLoggedIn)
                    } else {
                        // Calculate password hash for storage
                        let password_hash = {
                            use sha2::{Digest, Sha256};
                            let mut hasher = Sha256::new();
                            hasher.update(password.to_md5());
                            hex::encode(hasher.finalize())
                        };
                        
                        // Use storage_service for account operations
                        LOGIN_RUNTIME.block_on(async {
                            match storage_service.load_account(&username, &password_hash).await {
                                Ok(Some(account)) => {
                                    Ok(account)
                                },
                                Ok(None) => {
                                    // Account does not exist, create a new one
                                    let account = AccountStorage {
                                        name: username.clone(),
                                        password_md5_sha256: password_hash,
                                        character_names: Vec::new(),
                                    };
                                    
                                    match storage_service.create_account(&account).await {
                                        Ok(()) => {
                                            info!("Created account {}", &username);
                                            Ok(account)
                                        },
                                        Err(error) => {
                                            info!("Failed to create account {} with error {:?}", &username, error);
                                            Err(LoginError::InvalidAccount)
                                        }
                                    }
                                },
                                Err(error) => {
                                    error!("Failed to load account {} with error {:?}", &username, error);
                                    if let Some(AccountStorageError::InvalidPassword) = error.downcast_ref::<AccountStorageError>() {
                                        Err(LoginError::InvalidPassword)
                                    } else {
                                        Err(LoginError::Failed)
                                    }
                                }
                            }
                        })
                    };

                    let response = match login_result {
                        Ok(account) => {
                            commands.entity(entity).insert(Account::from(account));

                            ServerMessage::LoginSuccess {
                                server_list: server_list
                                    .world_servers
                                    .iter()
                                    .enumerate()
                                    .map(|(id, server)| (id as u32, server.name.clone()))
                                    .collect(),
                            }
                        }
                        Err(error) => ServerMessage::LoginError { error },
                    };

                    login_client.server_message_tx.send(response).ok();
                }
                _ => panic!("Received unexpected client message {:?}", message),
            }
        }
    });
}

pub fn login_server_system(
    mut query: Query<(Entity, &Account, &mut LoginClient)>,
    mut login_tokens: ResMut<LoginTokens>,
    server_list: Res<ServerList>,
) {
    query.for_each_mut(|(entity, account, mut login_client)| {
        if let Ok(message) = login_client.client_message_rx.try_recv() {
            match message {
                ClientMessage::GetChannelList { server_id } => {
                    let response = server_list.world_servers.get(server_id).map_or(
                        ServerMessage::ChannelListError {
                            error: ChannelListError::InvalidServerId { server_id },
                        },
                        |world_server| {
                            let mut channels = Vec::new();
                            for (id, channel) in world_server.channels.iter().enumerate() {
                                channels.push((id as u8, channel.name.clone()));
                            }
                            ServerMessage::ChannelList {
                                server_id,
                                channels,
                            }
                        },
                    );
                    login_client.server_message_tx.send(response).ok();
                }
                ClientMessage::JoinServer {
                    server_id,
                    channel_id,
                } => {
                    let response = server_list.world_servers.get(server_id).map_or(
                        ServerMessage::JoinServerError {
                            error: JoinServerError::InvalidServerId,
                        },
                        |world_server| {
                            world_server.channels.get(channel_id).map_or(
                                ServerMessage::JoinServerError {
                                    error: JoinServerError::InvalidChannelId,
                                },
                                |game_server| {
                                    login_client.login_token = login_tokens.generate(
                                        account.name.clone(),
                                        entity,
                                        world_server.entity,
                                        game_server.entity,
                                    );
                                    ServerMessage::JoinServerSuccess {
                                        login_token: login_client.login_token,
                                        packet_codec_seed: world_server.packet_codec_seed,
                                        ip: world_server.ip.clone(),
                                        port: world_server.port,
                                    }
                                },
                            )
                        },
                    );

                    login_client.server_message_tx.send(response).ok();
                }
                _ => warn!("[LS] Received unimplemented client message {:?}", message),
            }
        }
    });
}