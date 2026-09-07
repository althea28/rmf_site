use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use bevy::tasks::IoTaskPool;
use bevy_egui::{egui, EguiContexts};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rmf_site_format::NameInSite;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rmf_site_msgs::nav_msgs::msg::Odometry;
use rmf_site_msgs::rmf_prototype_msgs::msg::ParticipantList;
use roslibrust::rosbridge::ClientHandle;

#[derive(Debug, Clone)]
pub enum LiveEvent {
    Odom {
        name: String,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
    },
}

#[derive(Component)]
pub struct LiveRobotMarker {
    pub name: String,
    pub target_translation: Vec3,
    pub target_rotation: Quat,
}

#[derive(Resource)]
pub struct RosbridgeStreamChannel {
    pub sender: Sender<LiveEvent>,
    pub receiver: Receiver<LiveEvent>,
}

#[derive(Resource)]
pub struct LiveStreamState {
    pub url: String,
    pub is_connected: bool,
    pub connect_flag: Arc<AtomicBool>,
}

impl Default for LiveStreamState {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:9090".to_string(),
            is_connected: false,
            connect_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct LiveVisualizationPlugin;

impl Plugin for LiveVisualizationPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = unbounded();
        app.insert_resource(RosbridgeStreamChannel {
            sender: tx,
            receiver: rx,
        })
        .init_resource::<LiveStreamState>()
        .add_systems(Update, (update_live_robots, live_stream_ui));
    }
}

pub fn live_stream_ui(
    mut state: ResMut<LiveStreamState>,
    channel: Res<RosbridgeStreamChannel>,
    mut egui_context: EguiContexts,
) {
    egui::Window::new("Live Fleet Visualization")
        .default_pos([10.0, 10.0])
        .show(egui_context.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("ROSBridge WS URL:");
                ui.add_enabled_ui(!state.is_connected, |ui| {
                    ui.text_edit_singleline(&mut state.url);
                });
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                if state.is_connected {
                    ui.colored_label(egui::Color32::GREEN, "Connected");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "Not connected");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.is_connected {
                        if ui.button("Disconnect").clicked() {
                            state.is_connected = false;
                            state.connect_flag.store(false, Ordering::Relaxed);
                        }
                    } else {
                        if ui.button("Connect").clicked() {
                            state.is_connected = true;
                            state.connect_flag.store(true, Ordering::Relaxed);
                            start_rosbridge_subscriber(
                                &state.url,
                                channel.sender.clone(),
                                state.connect_flag.clone(),
                            );
                        }
                    }
                });
            });
        });
}

fn start_rosbridge_subscriber(
    ws_url: &str,
    sender: Sender<LiveEvent>,
    connect_flag: Arc<AtomicBool>,
) {
    let url = ws_url.to_string();

    // If compiling for a desktop, spawn a native OS background thread and initialize a fully-featured tokio multi-threaded async runtime
    // Gives native users maximum performance, as desktop computers handle networking best using OS-level threads and async runtimes like tokio.
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Failed to initialize Tokio runtime: {e}");
                    return;
                }
            };
            rt.block_on(run_rosbridge_loop(url, sender, connect_flag));
        });
    }

    // If compiling for wasm32, use Bevy's IoTaskPool
    // This is required as web browser security sandboxes completely block native OS threads and tokio multi-threading.
    #[cfg(target_arch = "wasm32")]
    {
        IoTaskPool::get()
            .spawn(run_rosbridge_loop(url, sender, connect_flag))
            .detach();
    }
}

async fn run_rosbridge_loop(url: String, sender: Sender<LiveEvent>, connect_flag: Arc<AtomicBool>) {
    if let Ok(client) = ClientHandle::new(&url).await {
        println!("Successfully connected via roslibrust to {}", url);

        // Subscribes to fleet discovery topic to find new robots
        if let Ok(discovery_sub) = client
            .subscribe::<ParticipantList>("/destination/discovery")
            .await
        {
            // Track which robots have been seen and subscribed to
            let mut subscribed_robots = HashSet::new();

            loop {
                if !connect_flag.load(Ordering::Relaxed) {
                    println!("Disconnecting from rosbridge.");
                    break;
                }

                let msg = discovery_sub.next().await;

                // Checks if user has disconnected
                if !connect_flag.load(Ordering::Relaxed) {
                    println!("Disconnecting from rosbridge.");
                    break;
                }

                for p in msg.participants {
                    if subscribed_robots.contains(&p.name) {
                        continue;
                    }
                    subscribed_robots.insert(p.name.clone());

                    // Build the async sub-task to handle this new robot's data stream
                    let robot_name = p.name.clone();
                    let topic_name = format!("/{}/odom", robot_name);
                    let sender_clone = sender.clone();
                    let odom_client = client.clone();
                    let connect_clone = connect_flag.clone();

                    println!("Dynamically discovering and subscribing to: {}", topic_name);

                    let odom_task = async move {
                        if let Ok(odom_sub) = odom_client.subscribe::<Odometry>(&topic_name).await {
                            loop {
                                if !connect_clone.load(Ordering::Relaxed) {
                                    break;
                                }

                                let odom = odom_sub.next().await;

                                if !connect_clone.load(Ordering::Relaxed) {
                                    break;
                                }

                                // Package and send odometry data to Bevy main thread
                                let pos = &odom.pose.pose.position;
                                let q = &odom.pose.pose.orientation;

                                let siny_cosp: f64 = 2.0 * (q.w * q.z + q.x * q.y);
                                let cosy_cosp: f64 = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
                                let yaw = siny_cosp.atan2(cosy_cosp) as f32;

                                let _ = sender_clone.send(LiveEvent::Odom {
                                    name: robot_name.clone(),
                                    x: pos.x as f32,
                                    y: pos.y as f32,
                                    z: pos.z as f32,
                                    yaw,
                                });
                            }
                        }
                    };

                    // Spawn the async task depending on the target platform
                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::spawn(odom_task);

                    #[cfg(target_arch = "wasm32")]
                    IoTaskPool::get().spawn(odom_task).detach();
                }
            }
        }
    } else {
        println!("Failed to connect to rosbridge WebSocket at {}", url);
    }
}

fn update_live_robots(
    channel: Res<RosbridgeStreamChannel>,
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut live_robots_query: Query<(&mut LiveRobotMarker, &mut Transform)>,
    mut untracked_entities_query: Query<
        (Entity, &NameInSite, &mut Transform),
        Without<LiveRobotMarker>,
    >,
) {
    // Wait for message from channel receiver
    while let Ok(event) = channel.receiver.try_recv() {
        // Match event type to update world
        match event {
            LiveEvent::Odom { name, x, y, z, yaw } => {
                let target_pos = Vec3::new(x, y, z);
                let target_rot = Quat::from_rotation_z(yaw);
                let mut found = false;

                // Update target position of tracked robot
                for (mut robot, _) in live_robots_query.iter_mut() {
                    if robot.name == name {
                        robot.target_translation = target_pos;
                        robot.target_rotation = target_rot;
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }

                // Update target position of new untracked robot, add marker to track it
                for (entity, name_in_site, mut transform) in untracked_entities_query.iter_mut() {
                    if name_in_site.0 == name {
                        transform.translation = target_pos;
                        transform.rotation = target_rot;
                        commands.entity(entity).insert(LiveRobotMarker {
                            name: name.clone(),
                            target_translation: target_pos,
                            target_rotation: target_rot,
                        });
                        found = true;
                        break;
                    }
                }

                if !found {
                    // TODO: add fallback?
                }
            }
        }
    }

    // Use linear interpolation to smooth out movement
    let smooth_factor = (10.0 * time.delta_secs()).min(1.0);
    for (marker, mut transform) in live_robots_query.iter_mut() {
        transform.translation = transform
            .translation
            .lerp(marker.target_translation, smooth_factor);
        transform.rotation = transform
            .rotation
            .slerp(marker.target_rotation, smooth_factor);
    }
}
