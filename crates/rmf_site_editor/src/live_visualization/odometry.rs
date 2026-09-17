use bevy::prelude::*;
use crossbeam_channel::Sender;
use rmf_site_format::{Angle, NameInSite, Pose, Rotation};
use rmf_site_msgs::nav_msgs::msg::Odometry;
use rmf_site_picking::Selectable;
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::live_state::LiveStreamState;
use super::network_client::{spawn_network_task, LiveStreamHandler, VisualizationStreamChannel};

#[derive(Debug, Clone)]
pub struct LiveEventOdom {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

impl LiveStreamHandler for LiveEventOdom {
    fn spawn_stream(
        robot_name: String,
        client: ClientHandle,
        sender: Sender<Self>,
        connection_requested: Arc<AtomicBool>,
    ) {
        let topic_name = format!("/{}/odom", robot_name);

        let task = async move {
            if let Ok(odom_sub) = client.subscribe::<Odometry>(&topic_name).await {
                loop {
                    let odom = odom_sub.next().await;

                    if !connection_requested.load(Ordering::Relaxed) {
                        break;
                    }

                    let pos = &odom.pose.pose.position;
                    let q = &odom.pose.pose.orientation;

                    let siny_cosp: f64 = 2.0 * (q.w * q.z + q.x * q.y);
                    let cosy_cosp: f64 = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
                    let yaw = siny_cosp.atan2(cosy_cosp) as f32;

                    if let Err(e) = sender.send(LiveEventOdom {
                        name: robot_name.clone(),
                        x: pos.x as f32,
                        y: pos.y as f32,
                        z: pos.z as f32,
                        yaw,
                    }) {
                        error!("Failed to send Odometry event across channel: {}", e);
                        break;
                    }
                }
            }
        };
        spawn_network_task(task);
    }
}

#[derive(Component)]
pub struct LiveRobotMarker {
    pub name: String,
}

#[derive(Default, Resource)]
pub struct LiveRobotsMap(pub HashMap<String, Entity>);

pub fn update_live_robots(
    state: Res<LiveStreamState>,
    channel: Res<VisualizationStreamChannel<LiveEventOdom>>,
    mut commands: Commands,
    mut robot_map: ResMut<LiveRobotsMap>,
    mut live_query: Query<(&LiveRobotMarker, &mut Pose)>,
    mut untracked_query: Query<(Entity, &NameInSite, &mut Pose), Without<LiveRobotMarker>>,
) {
    if !state.connection_requested.load(Ordering::Relaxed) {
        return;
    }

    while let Ok(event) = channel.receiver.try_recv() {
        let mut found = false;

        // Find existing robot
        for (robot, mut pose) in live_query.iter_mut() {
            if robot.name == event.name {
                pose.trans = [event.x, event.y, event.z];
                pose.rot = Rotation::Yaw(Angle::Rad(event.yaw).match_variant(pose.rot.yaw()));
                found = true;
                break;
            }
        }

        if found {
            continue;
        }

        // New untracked robot: find matching NameInSite
        for (entity, name_in_site, mut pose) in untracked_query.iter_mut() {
            if name_in_site.0 == event.name {
                pose.trans = [event.x, event.y, event.z];
                pose.rot = Rotation::Yaw(Angle::Rad(event.yaw).match_variant(pose.rot.yaw()));

                commands.entity(entity).insert(LiveRobotMarker {
                    name: event.name.clone(),
                });
                robot_map.0.insert(event.name.clone(), entity);

                found = true;
                break;
            }
        }

        if !found {
            println!("No matching NameInSite found for: {}", event.name);
        }
    }
}
