use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetClient};
use bevy_renet::{RenetClientPlugin, client_connected};

use std::time::SystemTime;
use std::{collections::HashMap, net::UdpSocket};

use serde::{Deserialize, Serialize};

const PROTOCOL_ID: u64 = 7;

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

fn new_renet_client() -> (RenetClient, NetcodeClientTransport) {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let client_id = current_time.as_millis() as u64;
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
    let client = RenetClient::new(ConnectionConfig::default());

    (client, transport)
}

fn main() {
    println!("Usage: run with \"server\" or \"client\" argument");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_resource::<Lobby>();

    app.add_plugins(RenetClientPlugin);
    app.add_plugins(NetcodeClientPlugin);
    app.init_resource::<PlayerInput>();
    let (client, transport) = new_renet_client();
    app.insert_resource(client);
    app.insert_resource(transport);

    app.add_systems(
        Update,
        (player_input, client_send_input, client_sync_players).run_if(client_connected),
    );

    app.add_systems(Startup, setup);
    app.add_systems(Update, panic_on_error_system);

    app.run();
}

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
                println!("Player {} connected.", id);
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
                println!("Player {} disconnected.", id);
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

fn player_input(keyboard_input: Res<ButtonInput<KeyCode>>, mut player_input: ResMut<PlayerInput>) {
    player_input.left = keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    player_input.right = keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    player_input.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    player_input.down = keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);
}

fn client_send_input(player_input: Res<PlayerInput>, mut client: ResMut<RenetClient>) {
    let input_message = bincode::serialize(&*player_input).unwrap();

    client.send_message(DefaultChannel::ReliableOrdered, input_message);
}

// If any error is found we just panic
#[allow(clippy::never_loop)]
fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
    for e in renet_error.read() {
        panic!("{}", e);
    }
}
