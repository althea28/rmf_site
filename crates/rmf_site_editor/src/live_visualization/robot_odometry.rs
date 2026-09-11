use bevy::prelude::*;
use crossbeam_channel::Sender;
use rmf_site_format::NameInSite;
use rmf_site_msgs::nav_msgs::msg::Odometry;
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::connection_window::LiveStreamState;
use super::network_client::StreamChannel;

pub const SMOOTHING_SPEED: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct LiveEventOdom {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

#[derive(Component)]
pub struct LiveRobotMarker {
    pub name: String,
    pub target_translation: Vec3,
    pub target_rotation: Quat,
}

#[derive(Default, Resource)]
pub struct LiveRobotsMap(pub HashMap<String, Entity>);

pub fn update_live_robots(
    state: Res<LiveStreamState>,
    channel: Res<StreamChannel<LiveEventOdom>>,
    time: Res<Time>,
    mut commands: Commands,
    mut robot_map: ResMut<LiveRobotsMap>,
    mut live_robots_query: Query<(&mut LiveRobotMarker, &mut Transform)>,
    mut untracked_entities_query: Query<
        (Entity, &NameInSite, &mut Transform),
        Without<LiveRobotMarker>,
    >,
) {
    if !state.is_connected {
        return;
    }

    while let Ok(event) = channel.receiver.try_recv() {
        let target_pos = Vec3::new(event.x, event.y, event.z);
        let target_rot = Quat::from_rotation_z(event.yaw);

        // Find existing robot
        if let Some(&entity) = robot_map.0.get(&event.name) {
            if let Ok((mut robot, _)) = live_robots_query.get_mut(entity) {
                robot.target_translation = target_pos;
                robot.target_rotation = target_rot;
                continue;
            } else {
                robot_map.0.remove(&event.name);
            }
        }

        // New untracked robot: find matching NameInSite
        let mut found_entity = None;
        for (entity, name_in_site, mut transform) in untracked_entities_query.iter_mut() {
            if name_in_site.0 == event.name {
                transform.translation = target_pos;
                transform.rotation = target_rot;
                commands.entity(entity).insert(LiveRobotMarker {
                    name: event.name.clone(),
                    target_translation: target_pos,
                    target_rotation: target_rot,
                });
                found_entity = Some(entity);
                break;
            }
        }

        if let Some(entity) = found_entity {
            robot_map.0.insert(event.name, entity);
        } else {
            println!("Robot {} not found in the scene.", event.name);
        }
    }

    // Use linear interpolation to smooth out movement
    let smooth_factor = (SMOOTHING_SPEED * time.delta_secs()).min(1.0);
    for (marker, mut transform) in live_robots_query.iter_mut() {
        transform.translation = transform
            .translation
            .lerp(marker.target_translation, smooth_factor);
        transform.rotation = transform
            .rotation
            .slerp(marker.target_rotation, smooth_factor);
    }
}

pub async fn handle_odometry_stream(
    robot_name: String,
    client: ClientHandle,
    sender: Sender<LiveEventOdom>,
    connect_flag: Arc<AtomicBool>,
) {
    let topic_name = format!("/{}/odom", robot_name);

    if let Ok(odom_sub) = client.subscribe::<Odometry>(&topic_name).await {
        loop {
            let odom = odom_sub.next().await;

            if !connect_flag.load(Ordering::Relaxed) {
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
}
