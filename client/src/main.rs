use once_cell::sync::Lazy;
use std::env;
use std::time::SystemTime;

use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetClient};
use bevy_renet::{RenetClientPlugin, client_connected};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::UdpSocket};
use std::{thread, time::Duration};

const PROTOCOL_ID: u64 = 7;

pub static CLIENT_ID: Lazy<u64> = Lazy::new(|| {
    env::var("CLIENT_ID")
        .ok()
        .and_then(|id_str| id_str.parse().ok())
        .unwrap_or_else(|| {
            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
            current_time.as_millis() as u64
        })
});

// Updated ClientSettings with additional fields.
#[derive(Resource, Clone)]
struct ClientSettings {
    max_retries: u32,
    initial_delay: Duration,
    server_ip: String,
    server_port: String,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            server_ip: env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string()),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Component, Resource)]
struct PlayerInput {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
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

/// Run bevy client
fn main() {
    let client_settings = ClientSettings::default();
    let (renet_client, renet_transport) = new_renet_client(&client_settings);

    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .init_resource::<Lobby>()
        .add_plugins((RenetClientPlugin, NetcodeClientPlugin))
        .init_resource::<PlayerInput>()
        .insert_resource(renet_client)
        .insert_resource(renet_transport)
        .insert_resource(client_settings)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (player_input, client_send_input, client_sync_players).run_if(client_connected),
        )
        .add_systems(Update, (reconnect_on_error_system, reconnect_check_system))
        .run();
}

/// Create a new RenetClient and NetcodeClientTransport using settings from ClientSettings.
fn new_renet_client(settings: &ClientSettings) -> (RenetClient, NetcodeClientTransport) {
    let server_addr = format!("{}:{}", settings.server_ip, settings.server_port).parse().unwrap();

    info!("Connecting to server at: {}", server_addr);
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let client_id = *CLIENT_ID;
    info!("Using CLIENT_ID={}", client_id);

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let mut retries = 0;
    let mut delay = settings.initial_delay;

    while retries < settings.max_retries {
        match NetcodeClientTransport::new(current_time, authentication.clone(), socket.try_clone().unwrap()) {
            Ok(transport) => {
                println!("✅ Connected to server on attempt {}", retries + 1);
                let client = RenetClient::new(ConnectionConfig::default());
                return (client, transport);
            }
            Err(e) => {
                println!(
                    "⚠️ Connection attempt {} failed: {}. Retrying in {}s...",
                    retries + 1,
                    e,
                    delay.as_secs()
                );
                thread::sleep(delay);
                delay *= 2; // Exponential backoff (1s, 2s, 4s, 8s...)
                retries += 1;
            }
        }
    }

    panic!("❌ Failed to connect to server after {} attempts.", settings.max_retries);
}

/// Sync player with the server
fn client_sync_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    mut lobby: ResMut<Lobby>,
) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerConnected { id } => {
                info!("Player {} connected.", id);
                let player_entity = commands
                    .spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(1.0)))),
                        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
                        Transform::from_xyz(0.0, 0.5, 0.0),
                    ))
                    .id();

                lobby.players.insert(id, player_entity);
            }
            ServerMessages::PlayerDisconnected { id } => {
                info!("Player {} disconnected.", id);
                if let Some(player_entity) = lobby.players.remove(&id) {
                    commands.entity(player_entity).despawn();
                }
            }
        }
    }

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        let players: HashMap<ClientId, [f32; 3]> = bincode::deserialize(&message).unwrap();
        for (player_id, translation) in players.iter() {
            if let Some(player_entity) = lobby.players.get(player_id) {
                let transform = Transform {
                    translation: (*translation).into(),
                    ..Default::default()
                };
                commands.entity(*player_entity).insert(transform);
            }
        }
    }
}

/// Setup the scene
fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // plane
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(PlaneMeshBuilder::from_size(Vec2::splat(5.0))))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Update the player input
fn player_input(keyboard_input: Res<ButtonInput<KeyCode>>, mut player_input: ResMut<PlayerInput>) {
    player_input.left = keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    player_input.right = keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    player_input.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    player_input.down = keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);
}

/// Send the player input to the server
fn client_send_input(player_input: Res<PlayerInput>, mut client: ResMut<RenetClient>) {
    let input_message = bincode::serialize(&*player_input).unwrap();
    client.send_message(DefaultChannel::ReliableOrdered, input_message);
}

/// Reconnect to the server if an error occurs
#[allow(clippy::never_loop)]
fn reconnect_on_error_system(
    mut commands: Commands,
    mut renet_error: EventReader<NetcodeTransportError>,
    time: Res<Time>,
    mut last_check: Local<f64>,
) {
    // Only check once per second
    if time.elapsed_secs_f64() - *last_check < 1.0 {
        return;
    }
    *last_check = time.elapsed_secs_f64();

    for e in renet_error.read() {
        error!("⚠️ Connection lost: {:?}", e);

        // Attempt to reconnect
        println!("🔄 Attempting to reconnect...");

        // Remove the old client resources
        commands.remove_resource::<RenetClient>();
        commands.remove_resource::<NetcodeClientTransport>();

        // Create a new client and transport
        let (new_client, new_transport) = new_renet_client(&ClientSettings {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            server_ip: env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string()),
        });

        // Re-insert the new client resources
        commands.insert_resource(new_client);
        commands.insert_resource(new_transport);

        println!("✅ Reconnection attempt completed.");
    }
}

/// Check the system to see if the client is connected
fn reconnect_check_system(mut commands: Commands, client: Res<RenetClient>, time: Res<Time>, mut last_check: Local<f64>) {
    // Only check once per second
    if time.elapsed_secs_f64() - *last_check < 1.0 {
        return;
    }
    *last_check = time.elapsed_secs_f64();

    // 🔹 Prevent reconnecting if already connected
    if client.is_connected() {
        return;
    }

    println!("⚠️ Connection lost. Attempting to reconnect...");

    // Remove old client
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();

    // Create a new client and transport
    let (new_client, new_transport) = new_renet_client(&ClientSettings {
        max_retries: 10,
        initial_delay: Duration::from_secs(1),
        server_ip: env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
        server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string()),
    });

    // Reinsert the new client
    commands.insert_resource(new_client);
    commands.insert_resource(new_transport);

    println!("✅ Reconnected to server!");
}
