// We must globally allow dead_code because of modular-bitfield..
#![allow(dead_code)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::needless_option_as_deref)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

mod game;
mod irose;
mod protocol;

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use clap::{Arg, Command};
use log::debug;
use simplelog::*;
use tokio::net::TcpListener;
use tokio::runtime::Builder;

use rose_file_readers::{
    HostFilesystemDevice, VfsIndex, VirtualFilesystem, VirtualFilesystemDevice,
};

use crate::{
    game::{GameConfig, storage::StorageBackend},
    protocol::server::{GameServer, LoginServer, WorldServer},
};

pub enum ProtocolType {
    Irose,
}

impl Default for ProtocolType {
    fn default() -> Self {
        Self::Irose
    }
}

async fn async_main() {
    TermLogger::init(
        LevelFilter::Trace,
        ConfigBuilder::new()
            .set_location_level(LevelFilter::Trace)
            .add_filter_ignore_str("mio")
            .add_filter_ignore_str("npc_ai")
            .add_filter_ignore_str("packets")
            .add_filter_ignore_str("quest")
            .build(),
        TerminalMode::Stdout,
        ColorChoice::Auto,
    )
    .expect("Failed to initialise logging");

    let mut command = Command::new("rose-offline")
        .arg(
            Arg::new("data-idx")
                .long("data-idx")
                .help("Path to data.idx")
                .takes_value(true),
        )
        .arg(
            Arg::new("data-path")
                .long("data-path")
                .help("Optional path to extracted data, any files here override ones in data.idx")
                .takes_value(true),
        )
        .arg(
            Arg::new("ip")
                .long("ip")
                .help("Listen IP used for login, world, game servers")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("login-port")
                .long("login-port")
                .help("Port for login server")
                .takes_value(true)
                .default_value("29000"),
        )
        .arg(
            Arg::new("world-port")
                .long("world-port")
                .help("Port for world server")
                .takes_value(true)
                .default_value("29100"),
        )
        .arg(
            Arg::new("game-port")
                .long("game-port")
                .help("Port for login server")
                .takes_value(true)
                .default_value("29200"),
        )
        .arg(
            clap::Arg::new("protocol")
                .long("protocol")
                .takes_value(true)
                .value_parser(["irose"])
                .default_value("irose")
                .help("Select which protocol to use."),
        )
        .arg(
            clap::Arg::new("use-postgres")
                .long("use-postgres")
                .help("Use PostgreSQL for storage instead of JSON files")
                .takes_value(false),
        )
        .arg(
            clap::Arg::new("postgres-connection")
                .long("postgres-connection")
                .help("PostgreSQL connection string (postgresql://user:pass@host/dbname)")
                .takes_value(true)
                .default_value("postgresql://postgres:postgres@localhost/rose_offline"),
        );
    let data_path_error = command.error(
        clap::ErrorKind::ArgumentNotFound,
        "Must specify at least one of --data-idx or --data-path",
    );
    let matches = command.get_matches();
    let listen_ip = matches.value_of("ip").unwrap();
    let login_port = matches.value_of("login-port").unwrap();
    let world_port = matches.value_of("world-port").unwrap();
    let game_port = matches.value_of("game-port").unwrap();
    let protocol_type = match matches.value_of("protocol") {
        Some("irose") => ProtocolType::Irose,
        _ => ProtocolType::default(),
    };

    let (login_protocol, world_protocol, game_protocol) = match protocol_type {
        ProtocolType::Irose => (
            irose::login_protocol(),
            irose::world_protocol(),
            irose::game_protocol(),
        ),
    };

    let mut data_idx_path = matches.value_of("data-idx").map(Path::new);
    let data_extracted_path = matches.value_of("data-path").map(Path::new);
    if data_idx_path.is_none() && data_extracted_path.is_none() {
        if Path::new("data.idx").exists() {
            data_idx_path = Some(Path::new("data.idx"));
        } else {
            data_path_error.exit();
        }
    }

    // Setup storage backend
    let storage_backend = if matches.is_present("use-postgres") {
        let connection_string = matches.value_of("postgres-connection").unwrap().to_string();
        log::info!("Using PostgreSQL storage backend with connection: {}", connection_string);
        StorageBackend::from_postgres_connection_string(connection_string)
    } else {
        log::info!("Using JSON file storage backend");
        StorageBackend::default()
    };

    let mut vfs_devices: Vec<Box<dyn VirtualFilesystemDevice + Send + Sync>> = Vec::new();
    if let Some(data_extracted_path) = data_extracted_path {
        log::info!(
            "Loading game data from path {}",
            data_extracted_path.to_string_lossy()
        );
        vfs_devices.push(Box::new(HostFilesystemDevice::new(
            data_extracted_path.to_path_buf(),
        )));
    }

    if let Some(data_idx_path) = data_idx_path {
        log::info!(
            "Loading game data from vfs {}",
            data_idx_path.to_string_lossy()
        );
        vfs_devices.push(Box::new(VfsIndex::load(data_idx_path).unwrap_or_else(
            |_| panic!("Failed to load {}", data_idx_path.to_string_lossy()),
        )));

        let index_root_path = data_idx_path
            .parent()
            .map(|path| path.into())
            .unwrap_or_else(PathBuf::new);
        log::info!(
            "Loading game data from vfs root path {}",
            index_root_path.to_string_lossy()
        );
        vfs_devices.push(Box::new(HostFilesystemDevice::new(index_root_path)));
    }

    let virtual_filesystem = VirtualFilesystem::new(vfs_devices);

    let started_load = Instant::now();
    let game_data = irose::get_game_data(&virtual_filesystem);
    debug!("Time take to read game data {:?}", started_load.elapsed());

    let game_config = GameConfig {
        enable_npc_spawns: true,
        enable_monster_spawns: true,
        storage_backend,
    };

    debug!("Using StorageBackend: {:?}", game_config.storage_backend);

    let (game_control_tx, game_control_rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        game::GameWorld::new(game_control_rx).run(game_config, game_data);
    });

    let mut login_server = LoginServer::new(
        TcpListener::bind(format!("{}:{}", listen_ip, login_port))
            .await
            .unwrap(),
        login_protocol,
        game_control_tx.clone(),
    )
    .await
    .unwrap();

    let mut world_server = WorldServer::new(
        String::from("_WorldServer"),
        TcpListener::bind(format!("{}:{}", listen_ip, world_port))
            .await
            .unwrap(),
        world_protocol,
        game_control_tx.clone(),
    )
    .await
    .unwrap();

    let mut game_server = GameServer::new(
        String::from("GameServer"),
        world_server.get_entity(),
        TcpListener::bind(format!("{}:{}", listen_ip, game_port))
            .await
            .unwrap(),
        game_protocol,
        game_control_tx.clone(),
    )
    .await
    .unwrap();

    tokio::spawn(async move {
        game_server.run().await;
    });

    tokio::spawn(async move {
        world_server.run().await;
    });

    login_server.run().await;
}

fn main() {
    let rt = Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        async_main().await;
    });
}
