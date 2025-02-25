use once_cell::sync::Lazy;
use std::env;
use std::time::SystemTime;

use bevy::render::texture::ImagePlugin;
use bevy::sprite::{Sprite, TextureAtlas};
use bevy::{app::AppExit, prelude::*};
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
    sprite_size: Vec2,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            server_ip: env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string()),
            sprite_size: Vec2::new(64.0, 64.0),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Component, Resource, Clone)]
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
    PlayerConnected { id: ClientId, color: [f32; 4] },
    PlayerDisconnected { id: ClientId },
}

#[derive(Resource, Default)]
struct InitialSyncDone(bool);

// New components for animation
#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

// New resource to hold the TextureAtlas handle
#[derive(Resource)]
struct PlayerAsset {
    texture: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource)]
enum Direction {
    Left,
    Right,
    Up,
}

// New resource to track last horizontal direction.
#[derive(Resource, Default)]
struct LastDirection(Option<Direction>);

// New resource that holds the animation index configurations.
#[derive(Resource)]
struct AnimationConfig {
    idle_right: (usize, usize),
    idle_left: (usize, usize),
    idle_up: (usize, usize),
    run_right: (usize, usize),
    run_left: (usize, usize),
    run_up: (usize, usize),
}

// Insert default values (adjust as needed).
impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            idle_right: (0, 0),
            run_right: (1, 6),
            idle_left: (7, 7),
            run_left: (8, 13),
            idle_up: (14, 14),
            run_up: (15, 20),
        }
    }
}

// New system to animate the sprite
fn animate_sprite(time: Res<Time>, mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite)>) {
    for (indices, mut timer, mut sprite) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                // Ensure index is within the expected range.
                if atlas.index < indices.first || atlas.index >= indices.last {
                    atlas.index = indices.first;
                } else {
                    atlas.index += 1;
                }
            }
        }
    }
}

// Composite system to update AnimationIndices based on player input and LastDirection.
fn update_direction_and_indices(
    player_input: Res<PlayerInput>,
    mut last_direction: ResMut<LastDirection>,
    animation_config: Res<AnimationConfig>,
    mut query: Query<&mut AnimationIndices>,
) {
    if player_input.left {
        *last_direction = LastDirection(Some(Direction::Left));
        let (first, last_val) = animation_config.run_left;
        for mut indices in query.iter_mut() {
            indices.first = first;
            indices.last = last_val;
        }
    } else if player_input.right {
        *last_direction = LastDirection(Some(Direction::Right));
        let (first, last_val) = animation_config.run_right;
        for mut indices in query.iter_mut() {
            indices.first = first;
            indices.last = last_val;
        }
    } else if player_input.up {
        *last_direction = LastDirection(Some(Direction::Up));
        let (first, last_val) = animation_config.run_up;
        for mut indices in query.iter_mut() {
            indices.first = first;
            indices.last = last_val;
        }
    } else if let Some(dir) = &last_direction.0 {
        match dir {
            Direction::Left => {
                let (first, last_val) = animation_config.idle_left;
                for mut indices in query.iter_mut() {
                    indices.first = first;
                    indices.last = last_val;
                }
            }
            Direction::Right => {
                let (first, last_val) = animation_config.idle_right;
                for mut indices in query.iter_mut() {
                    indices.first = first;
                    indices.last = last_val;
                }
            }
            Direction::Up => {
                let (first, last_val) = animation_config.idle_up;
                for mut indices in query.iter_mut() {
                    indices.first = first;
                    indices.last = last_val;
                }
            }
        }
    }
}

/// Run bevy client
fn main() {
    let client_settings = ClientSettings::default();
    let multiplayer = env::var("MULTIPLAYER").unwrap_or_default().to_lowercase() == "true";

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            // Prevent blurry sprites.
            .set(ImagePlugin::default_nearest()),
    )
    .init_resource::<Lobby>()
    .init_resource::<PlayerInput>()
    .init_resource::<InitialSyncDone>()
    .init_resource::<LastDirection>() // initialize LastDirection.
    .insert_resource(client_settings.clone())
    .insert_resource(AnimationConfig::default()); // add AnimationConfig

    // Register setup and animation system.
    if multiplayer {
        // Multiplayer: initialize renet client and add network plugins/systems.
        let (renet_client, renet_transport) = new_renet_client(&client_settings);
        app.insert_resource(renet_client)
            .insert_resource(renet_transport)
            .add_plugins((RenetClientPlugin, NetcodeClientPlugin))
            .add_systems(Startup, setup)
            .add_systems(Update, animate_sprite)
            .add_systems(
                Update,
                (player_input, client_send_input, client_sync_players, update_direction_and_indices).run_if(client_connected),
            )
            .add_systems(Update, (reconnect_on_error_system, reconnect_check_system, exit_system));
    } else {
        // Local mode: spawn local player, update input, then move the player.
        app.add_systems(Startup, setup)
            .add_systems(Startup, local_spawn_player.after(setup))
            .add_systems(
                Update,
                (
                    player_input,
                    exit_system,
                    local_update_player_input,
                    local_move_players_system,
                    animate_sprite,
                    update_direction_and_indices, // new composite system here
                ),
            );
    }

    app.run();
}

/// Create a new RenetClient and NetcodeClientTransport using settings from ClientSettings
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
    mut client: ResMut<RenetClient>,
    mut lobby: ResMut<Lobby>,
    mut initial_sync: ResMut<InitialSyncDone>,
    player_asset: Res<PlayerAsset>,
    settings: Res<ClientSettings>,
    anim_config: Res<AnimationConfig>,
) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let server_message: ServerMessages = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerConnected { id, color: _ } => {
                info!("Player {} connected.", id);
                let mut sprite = Sprite::from_atlas_image(
                    player_asset.texture.clone(),
                    TextureAtlas {
                        layout: player_asset.layout.clone(),
                        index: 1,
                    },
                );
                sprite.custom_size = Some(settings.sprite_size);
                let (first, last_val) = anim_config.idle_right;
                let player_entity = commands
                    .spawn((
                        sprite,
                        Transform::from_scale(Vec3::splat(6.0)),
                        AnimationIndices { first, last: last_val },
                        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
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
        let players: HashMap<ClientId, ([f32; 3], [f32; 4])> = bincode::deserialize(&message).unwrap();
        for (player_id, (translation, _color)) in players.iter() {
            let new_translation = Vec3::new(translation[0], -translation[2], 0.0);
            if let Some(&player_entity) = lobby.players.get(player_id) {
                // Only update transform
                commands.entity(player_entity).insert(Transform {
                    translation: new_translation,
                    ..Default::default()
                });
            } else if !initial_sync.0 {
                let mut sprite = Sprite::from_atlas_image(
                    player_asset.texture.clone(),
                    TextureAtlas {
                        layout: player_asset.layout.clone(),
                        index: 1,
                    },
                );
                sprite.custom_size = Some(settings.sprite_size);
                let (first, last_val) = anim_config.idle_right;
                // Spawn using idle indices.
                let player_entity = commands
                    .spawn((
                        sprite,
                        Transform {
                            translation: new_translation,
                            scale: Vec3::splat(6.0),
                            ..Default::default()
                        },
                        AnimationIndices { first, last: last_val },
                        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
                    ))
                    .id();
                lobby.players.insert(*player_id, player_entity);
            }
        }
        initial_sync.0 = true;
    }
}

/// Setup the scene
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>) {
    commands.spawn(Camera2d);
    let texture = asset_server.load("player.png");
    let num_columns = 7;
    let num_rows = 3; // right, left, up layers
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(24), num_columns, num_rows, None, None);
    let layout_handle = texture_atlas_layouts.add(layout);
    commands.insert_resource(PlayerAsset {
        texture,
        layout: layout_handle,
    });
}

/// Update the player input
fn player_input(keyboard_input: Res<ButtonInput<KeyCode>>, mut player_input: ResMut<PlayerInput>) {
    player_input.left = keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    player_input.right = keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    player_input.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    player_input.down = keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);
}

/// Exit system that gracefully disconnects from renet on Escape key press
fn exit_system(keyboard_input: Res<ButtonInput<KeyCode>>, client: Option<ResMut<RenetClient>>, mut exit: EventWriter<AppExit>) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        println!("Exit requested. Disconnecting gracefully...");
        if let Some(mut client) = client {
            client.disconnect();
        }
        exit.send(AppExit::Success);
    }
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
            sprite_size: Vec2::new(64.0, 64.0),
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
        sprite_size: Vec2::new(64.0, 64.0),
    });

    // Reinsert the new client
    commands.insert_resource(new_client);
    commands.insert_resource(new_transport);

    println!("✅ Reconnected to server!");
}

/// For local simulation, add a simple system that updates transformations using local input
fn local_move_players_system(mut query: Query<(&mut Transform, &PlayerInput)>, time: Res<Time>) {
    for (mut transform, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        transform.translation.x += x * time.delta().as_secs_f32();
        transform.translation.y += y * time.delta().as_secs_f32();
    }
}

/// Spawn a local player cube for local play
fn local_spawn_player(mut commands: Commands, mut lobby: ResMut<Lobby>, player_asset: Res<PlayerAsset>, anim_config: Res<AnimationConfig>) {
    if lobby.players.is_empty() {
        let local_client_id: ClientId = 0;
        let sprite = Sprite::from_atlas_image(
            player_asset.texture.clone(),
            TextureAtlas {
                layout: player_asset.layout.clone(),
                index: 1,
            },
        );
        let (first, last_val) = anim_config.idle_right; // default idle for local player.
        let player_entity = commands
            .spawn((
                sprite,
                Transform::from_scale(Vec3::splat(6.0)),
                PlayerInput::default(),
                AnimationIndices { first, last: last_val },
                AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
            ))
            .id();
        lobby.players.insert(local_client_id, player_entity);
        info!("Spawned local player with id {} using idle right: index {}", local_client_id, first);
    }
}

/// Update local player's PlayerInput component from the global resource
fn local_update_player_input(player_input_res: Res<PlayerInput>, lobby: Res<Lobby>, mut query: Query<&mut PlayerInput>) {
    if let Some(&local_entity) = lobby.players.get(&0) {
        if let Ok(mut component) = query.get_mut(local_entity) {
            *component = player_input_res.clone();
        }
    }
}
