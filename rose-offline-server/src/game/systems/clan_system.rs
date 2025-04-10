use std::num::{NonZeroU32, NonZeroUsize};
use log::info;
use tokio::runtime::Runtime;
use once_cell::sync::Lazy;

use bevy::{
    ecs::query::WorldQuery,
    prelude::{Changed, Commands, Entity, EventReader, Query, ResMut, Res},
};

use rose_data::{ClanMemberPosition, QuestTriggerHash};
use rose_game_common::{
    components::{ClanLevel, ClanPoints, ClanUniqueId},
    messages::server::{ClanCreateError, ClanMemberInfo, ServerMessage},
};

use crate::game::{
    components::{
        CharacterInfo, Clan, ClanMember, ClanMembership, ClientEntity, GameClient, Inventory,
        Level, Money,
    },
    events::ClanEvent,
    resources::ServerMessages,
    storage::{StorageService, ClanStorage, ClanStorageMember},
};

// Create a static runtime for async calls
static CLAN_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create clan runtime")
});

#[derive(WorldQuery)]
#[world_query(mutable)]
pub struct CreatorQuery<'w> {
    client_entity: &'w ClientEntity,
    character_info: &'w CharacterInfo,
    level: &'w Level,
    inventory: &'w mut Inventory,
    game_client: Option<&'w GameClient>,
    clan_membership: &'w ClanMembership,
}

#[derive(WorldQuery)]
pub struct MemberQuery<'w> {
    entity: Entity,
    character_info: &'w CharacterInfo,
    clan_membership: &'w ClanMembership,
    level: &'w Level,
    game_client: Option<&'w GameClient>,
}

fn send_update_clan_info(clan: &Clan, query_member: &Query<MemberQuery>) {
    for clan_member in clan.members.iter() {
        let &ClanMember::Online {
            entity: clan_member_entity,
            ..
        } = clan_member
        else {
            continue;
        };

        if let Ok(online_member) = query_member.get(clan_member_entity) {
            if let Some(online_member_game_client) = online_member.game_client {
                online_member_game_client
                    .server_message_tx
                    .send(ServerMessage::ClanUpdateInfo {
                        id: clan.unique_id,
                        mark: clan.mark,
                        level: clan.level,
                        points: clan.points,
                        money: clan.money,
                        skills: clan.skills.clone(),
                    })
                    .ok();
            }
        }
    }
}

pub fn clan_system(
    mut commands: Commands,
    mut clan_events: EventReader<ClanEvent>,
    query_member_connected: Query<MemberQuery, Changed<ClanMembership>>,
    query_member: Query<MemberQuery>,
    mut query_creator: Query<CreatorQuery>,
    mut query_clans: Query<(Entity, &mut Clan)>,
    mut server_messages: ResMut<ServerMessages>,
    storage_service: Res<StorageService>,
) {
    for event in clan_events.iter() {
        match event {
            ClanEvent::Create {
                creator: creator_entity,
                name,
                description,
                mark,
            } => {
                let Ok(mut creator) = query_creator.get_mut(*creator_entity) else {
                    continue;
                };

                // Cannot create a clan if already in one
                if creator.clan_membership.0.is_some() {
                    if let Some(game_client) = creator.game_client {
                        game_client
                            .server_message_tx
                            .send(ServerMessage::ClanCreateError {
                                error: ClanCreateError::Failed,
                            })
                            .ok();
                    }
                    continue;
                }

                if creator.level.level < 30 {
                    if let Some(game_client) = creator.game_client {
                        game_client
                            .server_message_tx
                            .send(ServerMessage::ClanCreateError {
                                error: ClanCreateError::UnmetCondition,
                            })
                            .ok();
                    }
                    continue;
                }

                // Check if clan name exists using StorageService
                let exists = CLAN_RUNTIME.block_on(async {
                    storage_service.clan_exists(name).await.unwrap_or(false)
                });
                if exists {
                    if let Some(game_client) = creator.game_client {
                        game_client
                            .server_message_tx
                            .send(ServerMessage::ClanCreateError {
                                error: ClanCreateError::NameExists,
                            })
                            .ok();
                    }
                    continue;
                }

                let Ok(money) = creator.inventory.try_take_money(Money(1000000)) else {
                    if let Some(game_client) = creator.game_client {
                        game_client
                            .server_message_tx
                            .send(ServerMessage::ClanCreateError {
                                error: ClanCreateError::UnmetCondition,
                            })
                            .ok();
                    }
                    continue;
                };

                // Create and save clan using StorageService
                let mut clan_storage = ClanStorage::new(name.clone(), description.clone(), *mark);
                clan_storage.members.push(ClanStorageMember::new(
                    creator.character_info.name.clone(),
                    ClanMemberPosition::Master,
                ));
                
                let create_result = CLAN_RUNTIME.block_on(async {
                    storage_service.create_clan(&clan_storage).await
                });
                
                if create_result.is_err() {
                    if let Some(game_client) = creator.game_client {
                        game_client
                            .server_message_tx
                            .send(ServerMessage::ClanCreateError {
                                error: ClanCreateError::Failed,
                            })
                            .ok();
                    }

                    creator.inventory.try_add_money(money).ok();
                    continue;
                }

                // Create clan entity
                let unique_id =
                    ClanUniqueId::new(QuestTriggerHash::from(name.as_str()).hash).unwrap();
                let members = vec![ClanMember::Online {
                    entity: *creator_entity,
                    position: ClanMemberPosition::Master,
                    contribution: ClanPoints(0),
                }];
                let clan_entity = commands
                    .spawn(Clan {
                        unique_id,
                        name: clan_storage.name.clone(),
                        description: clan_storage.description,
                        mark: clan_storage.mark,
                        money: clan_storage.money,
                        points: clan_storage.points,
                        level: clan_storage.level,
                        skills: clan_storage.skills,
                        members,
                    })
                    .id();

                // Add clan membership to creator
                commands
                    .entity(*creator_entity)
                    .insert(ClanMembership(Some(clan_entity)));

                // Update clan to nearby entities
                server_messages.send_entity_message(
                    creator.client_entity,
                    ServerMessage::CharacterUpdateClan {
                        client_entity_id: creator.client_entity.id,
                        id: unique_id,
                        mark: clan_storage.mark,
                        level: clan_storage.level,
                        name: clan_storage.name,
                        position: ClanMemberPosition::Master,
                    },
                );
            }
            &ClanEvent::MemberDisconnect {
                clan_entity,
                disconnect_entity,
                ref name,
                level,
                job,
            } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if let Some(clan_member) = clan.find_online_member_mut(disconnect_entity) {
                        let &mut ClanMember::Online {
                            position,
                            contribution,
                            ..
                        } = clan_member
                        else {
                            unreachable!()
                        };
                        *clan_member = ClanMember::Offline {
                            name: name.clone(),
                            position,
                            contribution,
                            level,
                            job,
                        };
                        
                        // Save the updated clan using StorageService
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after member disconnect: {:?}", err);
                            }
                        });

                        // Send message to other clan members that we have disconnected
                        for clan_member in clan.members.iter() {
                            let &ClanMember::Online {
                                entity: clan_member_entity,
                                ..
                            } = clan_member
                            else {
                                continue;
                            };

                            if let Ok(online_member) = query_member.get(clan_member_entity) {
                                if let Some(online_member_game_client) = online_member.game_client {
                                    online_member_game_client
                                        .server_message_tx
                                        .send(ServerMessage::ClanMemberDisconnected {
                                            name: name.clone(),
                                        })
                                        .ok();
                                }
                            }
                        }
                    }
                }
            }
            &ClanEvent::GetMemberList { entity } => {
                if let Ok(requestor) = query_member.get(entity) {
                    if let Some(clan_entity) = requestor.clan_membership.0 {
                        if let Ok((_, clan)) = query_clans.get(clan_entity) {
                            let mut members = Vec::new();

                            for member in clan.members.iter() {
                                match member {
                                    ClanMember::Online {
                                        entity: member_entity,
                                        position,
                                        contribution,
                                    } => {
                                        if let Ok(member_data) = query_member.get(*member_entity) {
                                            members.push(ClanMemberInfo {
                                                name: member_data.character_info.name.clone(),
                                                position: *position,
                                                contribution: *contribution,
                                                channel_id: NonZeroUsize::new(1),
                                                level: rose_game_common::components::Level::new(member_data.level.level),
                                                job: member_data.character_info.job,
                                            });
                                        }
                                    },
                                    ClanMember::Offline {
                                        name,
                                        position,
                                        contribution,
                                        level,
                                        job,
                                    } => {
                                        members.push(ClanMemberInfo {
                                            name: name.clone(),
                                            position: *position,
                                            contribution: *contribution,
                                            channel_id: None,
                                            level: rose_game_common::components::Level::new(level.level),
                                            job: *job,
                                        });
                                    }
                                }
                            }

                            if let Some(game_client) = requestor.game_client {
                                game_client
                                    .server_message_tx
                                    .send(ServerMessage::ClanMemberList { members })
                                    .ok();
                            }
                        }
                    }
                }
            }
            &ClanEvent::AddLevel { clan_entity, level } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if let Some(new_level) = clan
                        .level
                        .0
                        .get()
                        .checked_add_signed(level)
                        .and_then(NonZeroU32::new)
                    {
                        clan.level = ClanLevel(new_level);
                        
                        // Save clan changes
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after level change: {:?}", err);
                            }
                        });
                        
                        send_update_clan_info(&clan, &query_member);
                    }
                }
            }
            &ClanEvent::SetLevel { clan_entity, level } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    clan.level = level;
                    
                    // Save clan changes
                    let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                    
                    CLAN_RUNTIME.block_on(async {
                        if let Err(err) = storage_service.save_clan(&clan_storage).await {
                            log::error!("Failed to save clan after level set: {:?}", err);
                        }
                    });
                    
                    send_update_clan_info(&clan, &query_member);
                }
            }
            &ClanEvent::AddMoney { clan_entity, money } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if let Some(new_money) = clan.money.0.checked_add(money) {
                        clan.money = Money(new_money);
                        
                        // Save clan changes
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after money change: {:?}", err);
                            }
                        });
                        
                        send_update_clan_info(&clan, &query_member);
                    }
                }
            }
            &ClanEvent::SetMoney { clan_entity, money } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    clan.money = money;
                    
                    // Save clan changes
                    let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                    
                    CLAN_RUNTIME.block_on(async {
                        if let Err(err) = storage_service.save_clan(&clan_storage).await {
                            log::error!("Failed to save clan after money set: {:?}", err);
                        }
                    });
                    
                    send_update_clan_info(&clan, &query_member);
                }
            }
            &ClanEvent::AddPoints {
                clan_entity,
                points,
            } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if let Some(new_points) = clan.points.0.checked_add_signed(points) {
                        clan.points = ClanPoints(new_points);
                        
                        // Save clan changes
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after points change: {:?}", err);
                            }
                        });
                        
                        send_update_clan_info(&clan, &query_member);
                    }
                }
            }
            &ClanEvent::SetPoints {
                clan_entity,
                points,
            } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    clan.points = points;
                    
                    // Save clan changes
                    let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                    
                    CLAN_RUNTIME.block_on(async {
                        if let Err(err) = storage_service.save_clan(&clan_storage).await {
                            log::error!("Failed to save clan after points set: {:?}", err);
                        }
                    });
                    
                    send_update_clan_info(&clan, &query_member);
                }
            }
            &ClanEvent::AddSkill {
                clan_entity,
                skill_id,
            } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if !clan.skills.iter().any(|id| *id == skill_id) {
                        clan.skills.push(skill_id);
                        
                        // Save clan changes
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after skill addition: {:?}", err);
                            }
                        });
                        
                        send_update_clan_info(&clan, &query_member);
                    }
                }
            }
            &ClanEvent::RemoveSkill {
                clan_entity,
                skill_id,
            } => {
                if let Ok((_, mut clan)) = query_clans.get_mut(clan_entity) {
                    if clan.skills.iter().any(|id| *id == skill_id) {
                        clan.skills.retain(|id| *id != skill_id);
                        
                        // Save clan changes
                        let clan_storage = convert_clan_to_storage(&*clan, &query_member);
                        
                        CLAN_RUNTIME.block_on(async {
                            if let Err(err) = storage_service.save_clan(&clan_storage).await {
                                log::error!("Failed to save clan after skill removal: {:?}", err);
                            }
                        });
                        
                        send_update_clan_info(&clan, &query_member);
                    }
                }
            }
        }
    }

    for connected_member in query_member_connected.iter() {
        let Some(clan) = connected_member
            .clan_membership
            .0
            .and_then(|clan_entity| query_clans.get(clan_entity).ok().map(|(_, clan)| clan))
        else {
            continue;
        };

        let Some(&ClanMember::Online {
            position: connected_member_position,
            contribution: connected_member_contribution,
            ..
        }) = clan.find_online_member(connected_member.entity)
        else {
            continue;
        };

        if let Some(game_client) = connected_member.game_client {
            game_client
                .server_message_tx
                .send(ServerMessage::ClanInfo {
                    id: clan.unique_id,
                    name: clan.name.clone(),
                    description: clan.description.clone(),
                    mark: clan.mark,
                    level: clan.level,
                    points: clan.points,
                    money: clan.money,
                    skills: clan.skills.clone(),
                    position: connected_member_position,
                    contribution: connected_member_contribution,
                })
                .ok();
        }

        // Send message to other clan members that we have connected
        for clan_member in clan.members.iter() {
            let &ClanMember::Online {
                entity: clan_member_entity,
                ..
            } = clan_member
            else {
                continue;
            };

            if clan_member_entity == connected_member.entity {
                continue;
            }

            if let Ok(online_member) = query_member.get(clan_member_entity) {
                if let Some(online_member_game_client) = online_member.game_client {
                    online_member_game_client
                        .server_message_tx
                        .send(ServerMessage::ClanMemberConnected {
                            name: connected_member.character_info.name.clone(),
                            channel_id: NonZeroUsize::new(1).unwrap(),
                        })
                        .ok();
                }
            }
        }
    }
}

// Helper function to convert Clan to ClanStorage
fn convert_clan_to_storage(clan: &Clan, query_member: &Query<MemberQuery>) -> ClanStorage {
    let mut storage_members = Vec::new();
    
    for member in clan.members.iter() {
        match member {
            ClanMember::Online { 
                entity: member_entity, 
                position, 
                contribution 
            } => {
                // For online members, fetch the name from the query system
                if let Ok(member_data) = query_member.get(*member_entity) {
                    storage_members.push(ClanStorageMember {
                        name: member_data.character_info.name.clone(),
                        position: *position,
                        contribution: *contribution,
                    });
                }
            },
            ClanMember::Offline { name, position, contribution, .. } => {
                storage_members.push(ClanStorageMember {
                    name: name.clone(),
                    position: *position,
                    contribution: *contribution,
                });
            }
        }
    }
    
    ClanStorage {
        name: clan.name.clone(),
        description: clan.description.clone(),
        mark: clan.mark,
        money: clan.money,
        points: clan.points,
        level: clan.level,
        members: storage_members,
        skills: clan.skills.clone(),
    }
}