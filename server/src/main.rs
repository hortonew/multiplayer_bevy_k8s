use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use bevy_renet::RenetServerPlugin;
use bevy_renet::netcode::{NetcodeServerPlugin, NetcodeServerTransport, NetcodeTransportError, ServerAuthentication, ServerConfig};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};

use std::time::SystemTime;
use std::{collections::HashMap, net::UdpSocket};

use serde::{Deserialize, Serialize};

const PROTOCOL_ID: u64 = 7;

const PLAYER_MOVE_SPEED: f32 = 1.0;

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

#[derive(Debug, Default, Resource)]
struct Lobby {
    players: HashMap<ClientId, Entity>,
}

#[derive(Debug, Serialize, Deserialize, Component)]
enum ServerMessages {
    PlayerConnected { id: ClientId },
    PlayerDisconnected { id: ClientId },
}

fn new_renet_server() -> (RenetServer, NetcodeServerTransport) {
    let public_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind(public_addr).unwrap();
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: 64,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![public_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    let server = RenetServer::new(ConnectionConfig::default());

    (server, transport)
}

fn main() {
    println!("Usage: run with \"server\" or \"client\" argument");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_resource::<Lobby>();

    app.add_plugins(RenetServerPlugin);
    app.add_plugins(NetcodeServerPlugin);
    let (server, transport) = new_renet_server();
    app.insert_resource(server);
    app.insert_resource(transport);

    app.add_systems(
        Update,
        (server_update_system, server_sync_players, move_players_system).run_if(resource_exists::<RenetServer>),
    );

    app.add_systems(Startup, setup);
    app.add_systems(Update, panic_on_error_system);

    app.run();
}

fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<Lobby>,
    mut server: ResMut<RenetServer>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("Player {} connected.", client_id);
                // Spawn player cube
                let player_entity = commands
                    .spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(1.0)))),
                        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
                        Transform::from_xyz(0.0, 0.5, 0.0),
                    ))
                    .insert(PlayerInput::default())
                    .insert(Player { id: *client_id })
                    .id();

                // We could send an InitState with all the players id and positions for the client
                // but this is easier to do.
                for &player_id in lobby.players.keys() {
                    let message = bincode::serialize(&ServerMessages::PlayerConnected { id: player_id }).unwrap();
                    server.send_message(*client_id, DefaultChannel::ReliableOrdered, message);
                }

                lobby.players.insert(*client_id, player_entity);

                let message = bincode::serialize(&ServerMessages::PlayerConnected { id: *client_id }).unwrap();
                server.broadcast_message(DefaultChannel::ReliableOrdered, message);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Player {} disconnected: {}", client_id, reason);
                if let Some(player_entity) = lobby.players.remove(client_id) {
                    commands.entity(player_entity).despawn();
                }

                let message = bincode::serialize(&ServerMessages::PlayerDisconnected { id: *client_id }).unwrap();
                server.broadcast_message(DefaultChannel::ReliableOrdered, message);
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered) {
            let player_input: PlayerInput = bincode::deserialize(&message).unwrap();
            if let Some(player_entity) = lobby.players.get(&client_id) {
                commands.entity(*player_entity).insert(player_input);
            }
        }
    }
}

fn server_sync_players(mut server: ResMut<RenetServer>, query: Query<(&Transform, &Player)>) {
    let mut players: HashMap<ClientId, [f32; 3]> = HashMap::new();
    for (transform, player) in query.iter() {
        players.insert(player.id, transform.translation.into());
    }

    let sync_message = bincode::serialize(&players).unwrap();
    server.broadcast_message(DefaultChannel::Unreliable, sync_message);
}

/// set up a simple 3D scene
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

fn move_players_system(mut query: Query<(&mut Transform, &PlayerInput)>, time: Res<Time>) {
    for (mut transform, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        transform.translation.x += x * PLAYER_MOVE_SPEED * time.delta().as_secs_f32();
        transform.translation.z += y * PLAYER_MOVE_SPEED * time.delta().as_secs_f32();
    }
}

// If any error is found we just panic
#[allow(clippy::never_loop)]
fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
    for e in renet_error.read() {
        panic!("{}", e);
    }
}
