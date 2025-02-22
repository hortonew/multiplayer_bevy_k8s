use bevy::input::InputPlugin;
use bevy::log::LogPlugin;
use bevy::time::TimePlugin;
use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_renet::RenetServerPlugin;
use bevy_renet::netcode::{NetcodeServerPlugin, NetcodeServerTransport, NetcodeTransportError, ServerAuthentication, ServerConfig};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::SystemTime;
use std::{collections::HashMap, net::UdpSocket};

const PROTOCOL_ID: u64 = 7;

#[derive(Debug, Default, Serialize, Deserialize, Component, Resource)]
struct PlayerInput {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

#[derive(Debug, Component)]
struct Player {
    id: ClientId,
}

#[derive(Debug, Component)]
struct Disconnected {
    disconnect_time: f64,
}

#[derive(Debug, Default, Resource)]
struct Lobby {
    players: HashMap<ClientId, Entity>,
}

#[derive(Debug, Serialize, Deserialize, Component)]
enum ServerMessages {
    PlayerConnected { id: ClientId },
    PlayerDisconnected { id: ClientId },
}

// New resource holding server settings.
#[derive(Resource, Clone)]
struct ServerSettings {
    port: u16,
    max_clients: u32,
    player_move_speed: f32,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: env::var("SERVER_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(5000),
            max_clients: 64,
            player_move_speed: 1.0,
        }
    }
}

/// Run bevy server
fn main() {
    let mut app = App::new();
    app.add_plugins((
        TimePlugin,
        InputPlugin,
        TransformPlugin,
        TaskPoolPlugin {
            task_pool_options: Default::default(),
        },
        LogPlugin::default(),
    ));
    info!("Starting server...");
    let server_settings = ServerSettings::default();
    let (renet_server, renet_transport) = new_renet_server(&server_settings);
    app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)))
        .init_resource::<Lobby>()
        .add_plugins((RenetServerPlugin, NetcodeServerPlugin))
        .insert_resource(renet_server)
        .insert_resource(renet_transport)
        .insert_resource(server_settings)
        .add_systems(
            Update,
            (server_update_system, server_sync_players, move_players_system).run_if(resource_exists::<RenetServer>),
        )
        .add_systems(Update, (cleanup_disconnected_system, panic_on_error_system))
        .run();
}

/// Create a new Renet server and Netcode transport using settings from ServerSettings.
fn new_renet_server(settings: &ServerSettings) -> (RenetServer, NetcodeServerTransport) {
    let port = settings.port;
    info!("Server listening on port: {}", port);
    let public_addr = format!("0.0.0.0:{}", port).parse().unwrap();
    let socket = UdpSocket::bind(public_addr).unwrap();
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: settings.max_clients as usize,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![public_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    let server = RenetServer::new(ConnectionConfig::default());
    (server, transport)
}

/// System to handle server events and player input.
fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    mut commands: Commands,
    mut lobby: ResMut<Lobby>,
    mut server: ResMut<RenetServer>,
    time: Res<Time>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Player {} connected.", client_id);
                if let Some(&player_entity) = lobby.players.get(client_id) {
                    // If reconnecting, remove Disconnected marker if it exists.
                    commands.entity(player_entity).remove::<Disconnected>();
                    info!("Reattached client {} to existing entity.", client_id);
                } else {
                    // Spawn new player cube if it doesn't exist.
                    let player_entity = commands
                        .spawn((Transform::from_xyz(0.0, 0.5, 0.0),))
                        .insert(PlayerInput::default())
                        .insert(Player { id: *client_id })
                        .id();
                    lobby.players.insert(*client_id, player_entity);
                }
                // Broadcast connection info.
                let message = bincode::serialize(&ServerMessages::PlayerConnected { id: *client_id }).unwrap();
                server.broadcast_message(DefaultChannel::ReliableOrdered, message);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Player {} disconnected: {}", client_id, reason);
                if let Some(&player_entity) = lobby.players.get(client_id) {
                    // Mark as disconnected instead of despawning immediately.
                    commands.entity(player_entity).insert(Disconnected {
                        disconnect_time: time.elapsed_secs_f64(),
                    });
                    let message = bincode::serialize(&ServerMessages::PlayerDisconnected { id: *client_id }).unwrap();
                    server.broadcast_message(DefaultChannel::ReliableOrdered, message);
                }
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered) {
            let player_input: PlayerInput = bincode::deserialize(&message).unwrap();
            if let Some(&player_entity) = lobby.players.get(&client_id) {
                commands.entity(player_entity).insert(player_input);
            }
        }
    }
}

/// System to cleanup disconnected entities after a number of seconds.
fn cleanup_disconnected_system(mut commands: Commands, time: Res<Time>, mut lobby: ResMut<Lobby>, query: Query<(Entity, &Disconnected)>) {
    let grace_period = 20.0;
    for (entity, disconnected) in query.iter() {
        if time.elapsed_secs_f64() - disconnected.disconnect_time > grace_period {
            let client_id_opt = lobby.players.iter().find_map(|(id, &e)| if e == entity { Some(*id) } else { None });
            if let Some(client_id) = client_id_opt {
                lobby.players.remove(&client_id);
            }
            commands.entity(entity).despawn();
            info!("Cleaned up disconnected entity {:?}", entity);
        }
    }
}

/// System to sync player positions to clients.
fn server_sync_players(mut server: ResMut<RenetServer>, query: Query<(&Transform, &Player)>) {
    let mut players: HashMap<ClientId, [f32; 3]> = HashMap::new();
    for (transform, player) in query.iter() {
        players.insert(player.id, transform.translation.into());
    }
    let sync_message = bincode::serialize(&players).unwrap();
    server.broadcast_message(DefaultChannel::Unreliable, sync_message);
}

/// System to move player entities based on input.
fn move_players_system(mut query: Query<(&mut Transform, &PlayerInput)>, time: Res<Time>, server_settings: Res<ServerSettings>) {
    for (mut transform, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        transform.translation.x += x * server_settings.player_move_speed * time.delta().as_secs_f32();
        transform.translation.z += y * server_settings.player_move_speed * time.delta().as_secs_f32();
    }
}

/// If any error is found we just panic
#[allow(clippy::never_loop)]
fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
    for e in renet_error.read() {
        panic!("{}", e);
    }
}
