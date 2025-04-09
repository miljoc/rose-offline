use std::time::Duration;
use std::sync::Arc;
use tokio::runtime::Runtime;

use bevy::{
    app::ScheduleRunnerPlugin,
    prelude::{
        apply_deferred, App, IntoSystemConfigs, Last, PluginGroup, PostUpdate, PreUpdate, Startup,
        Update, Resource,
    },
    MinimalPlugins,
};
use crossbeam_channel::Receiver;

use crate::game::{
    bots::BotPlugin,
    events::{
        BankEvent, ChatCommandEvent, ClanEvent, DamageEvent, EquipmentEvent, ItemLifeEvent,
        NpcStoreEvent, PartyEvent, PartyMemberEvent, PersonalStoreEvent, PickupItemEvent,
        QuestTriggerEvent, ReviveEvent, RewardItemEvent, RewardXpEvent, SaveEvent, SkillEvent,
        UseAmmoEvent, UseItemEvent,
    },
    messages::control::ControlMessage,
    resources::{
        BotList, ClientEntityList, ControlChannel, GameConfig, GameData, LoginTokens, ServerList,
        ServerMessages, WorldRates, WorldTime, ZoneList,
    },
    systems::{
        database_system, ability_values_changed_system, ability_values_update_character_system,
        ability_values_update_npc_system, bank_system, chat_commands_system, clan_system,
        client_entity_visibility_system, command_system, control_server_system, damage_system,
        driving_time_system, equipment_event_system, experience_points_system, expire_time_system,
        game_server_authentication_system, game_server_join_system, game_server_main_system,
        item_life_system, login_server_authentication_system, login_server_system,
        monster_spawn_system, npc_ai_system, npc_store_system, party_member_event_system,
        party_member_update_info_system, party_system, party_update_average_level_system,
        passive_recovery_system, personal_store_system, pickup_item_system, quest_system,
        revive_event_system, reward_item_system, save_system, server_messages_system,
        skill_effect_system, startup_clans_system, startup_zones_system, status_effect_system,
        update_character_motion_data_system, update_npc_motion_data_system, update_position_system,
        use_ammo_system, use_item_system, weight_system, world_server_authentication_system,
        world_server_system, world_time_system,
    },
    storage::{
        storage_adapter, storage_service,
    }
};

use crate::game::storage::config::StorageConfig;
use crate::game::storage::StorageService;
use crate::game::storage::StorageAdapter;
use crate::game::storage::JsonStorageAdapter;
use crate::game::storage::PostgresStorageAdapter;
use crate::game::storage::StorageBackend;
use crate::game::storage::StorageService as StorageServiceType;
use crate::game::storage::StorageBackend as StorageBackendType;

pub struct GameWorld {
    control_rx: Receiver<ControlMessage>,
}

impl GameWorld {
    pub fn new(control_rx: Receiver<ControlMessage>) -> Self {
        Self { control_rx }
    }

    pub fn run(&mut self, game_config: GameConfig, game_data: GameData) {
        // Create a tokio runtime for async operations
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        
        // Convert the StorageBackend from game_config to config::StorageBackend
        let backend = match &game_config.storage_backend {
            StorageBackendType::JsonStorageAdapter => crate::game::storage::config::StorageBackend::JsonStorageAdapter,
            StorageBackendType::PostgresStorageAdapter(conn_string) => 
                crate::game::storage::config::StorageBackend::PostgresStorageAdapter(conn_string.clone()),
            _ => crate::game::storage::config::StorageBackend::Json,
        };
        
        // Create the storage config and adapter
        let storage_config = StorageConfig::new(backend);
        
        // Create the appropriate adapter based on the storage backend
        let adapter: Arc<dyn StorageAdapter> = match &game_config.storage_backend {
            StorageBackendType::JsonStorageAdapter => {
                let adapter = JsonStorageAdapter::new();
                runtime.block_on(async {
                    adapter.init().await.expect("Failed to initialize JSON storage adapter");
                });
                Arc::new(adapter)
            },
            StorageBackendType::PostgresStorageAdapter(conn_string) => {
                let adapter = runtime.block_on(async {
                    PostgresStorageAdapter::new(conn_string)
                        .await
                        .expect("Failed to create PostgreSQL adapter")
                });
                
                // Initialize the adapter after creating it
                runtime.block_on(async {
                    adapter.init().await.expect("Failed to initialize PostgreSQL storage adapter");
                });
                
                Arc::new(adapter)
            }
        };

        // Use the storage_service::StorageService 
        let storage_service = StorageService::new(adapter);
        
        // Rest of your application setup...
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f64(1.0 / 60.0),
        )));
        app.add_plugins(BotPlugin);

        app.insert_resource(BotList::new());
        app.insert_resource(ClientEntityList::new(&game_data.zones));
        app.insert_resource(ControlChannel::new(self.control_rx.clone()));
        app.insert_resource(LoginTokens::new());
        app.insert_resource(ServerList::new());
        app.insert_resource(ServerMessages::new());
        app.insert_resource(WorldRates::new());
        app.insert_resource(WorldTime::new());
        app.insert_resource(ZoneList::new());
        app.insert_resource(game_config);
        app.insert_resource(game_data);
        app.insert_resource(storage_service);

        app.add_event::<BankEvent>()
            .add_event::<ChatCommandEvent>()
            .add_event::<ClanEvent>()
            .add_event::<DamageEvent>()
            .add_event::<EquipmentEvent>()
            .add_event::<ItemLifeEvent>()
            .add_event::<NpcStoreEvent>()
            .add_event::<PartyEvent>()
            .add_event::<PartyMemberEvent>()
            .add_event::<PersonalStoreEvent>()
            .add_event::<PickupItemEvent>()
            .add_event::<QuestTriggerEvent>()
            .add_event::<ReviveEvent>()
            .add_event::<RewardItemEvent>()
            .add_event::<RewardXpEvent>()
            .add_event::<SaveEvent>()
            .add_event::<SkillEvent>()
            .add_event::<UseAmmoEvent>()
            .add_event::<UseItemEvent>();

        /*
        Stage order:
        - CoreSet::First
        - CoreSet::PreUpdate
        - GameStages::Input
        - CoreSet::Update
        - CoreSet::PostUpdate
        - CoreSet::Last
        */
        app.add_systems(Startup, (
            startup_clans_system,
            startup_zones_system,
            database_system
        ));

        app.add_systems(
            PreUpdate,
            (
                (
                    world_time_system,
                    control_server_system,
                    login_server_authentication_system,
                    login_server_system,
                    world_server_authentication_system,
                    world_server_system,
                    game_server_authentication_system,
                    game_server_join_system,
                    (game_server_main_system, revive_event_system).chain(),
                    chat_commands_system,
                    monster_spawn_system,
                    npc_ai_system,
                    expire_time_system,
                    status_effect_system,
                    passive_recovery_system,
                    driving_time_system,
                ),
                apply_deferred,
                (
                    (
                        (
                            update_character_motion_data_system,
                            update_npc_motion_data_system,
                            update_position_system,
                        ),
                        command_system,
                        (use_ammo_system, pickup_item_system),
                    )
                        .chain(),
                    (
                        party_member_event_system,
                        party_system,
                        party_member_update_info_system,
                    )
                        .chain(),
                    clan_system,
                ),
            )
                .chain(),
        );

        app.add_systems(
            Update,
            (
                bank_system,
                personal_store_system,
                npc_store_system,
                quest_system,
                use_item_system,
                reward_item_system,
                damage_system.before(item_life_system),
                skill_effect_system.before(item_life_system),
                item_life_system,
                equipment_event_system.after(item_life_system),
            ),
        );

        app.add_systems(
            PostUpdate,
            (
                weight_system,
                experience_points_system,
                party_update_average_level_system.after(experience_points_system),
                client_entity_visibility_system,
            ),
        );

        app.add_systems(
            Last,
            (
                ability_values_update_character_system.before(ability_values_changed_system),
                ability_values_update_npc_system.before(ability_values_changed_system),
                ability_values_changed_system,
                server_messages_system,
                save_system,
            ),
        );

        app.run();
    }
}
