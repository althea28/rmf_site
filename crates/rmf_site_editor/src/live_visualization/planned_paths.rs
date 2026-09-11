use bevy::prelude::*;
use crossbeam_channel::Sender;
use rmf_site_msgs::rmf_prototype_msgs::msg::{Plan, Progress};
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::connection_window::LiveStreamState;
use super::network_client::StreamChannel;
use super::robot_odometry::{LiveRobotMarker, LiveRobotsMap};

pub const PLANNED_PATH_Z_OFFSET: f32 = 0.05;
pub const PLANNED_PATH_COLOR: Color = Color::srgb(0.0, 1.0, 0.0);

#[derive(Debug, Clone)]
pub struct LiveEventPlan {
    pub name: String,
    pub waypoints: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct LiveEventProgress {
    pub name: String,
    pub target_waypoint: usize,
}

#[derive(Component)]
pub struct LivePathMarker {
    pub name: String,
    pub waypoints: Vec<Vec3>,
    pub target_waypoint: usize,
}

#[derive(Default, Resource)]
pub struct LivePathsMap(pub HashMap<String, Entity>);

pub fn update_live_paths(
    state: Res<LiveStreamState>,
    plan_channel: Res<StreamChannel<LiveEventPlan>>,
    progress_channel: Res<StreamChannel<LiveEventProgress>>,
    mut commands: Commands,
    mut path_map: ResMut<LivePathsMap>,
    robot_map: Res<LiveRobotsMap>,
    mut path_query: Query<&mut LivePathMarker>,
    robot_query: Query<&Transform, With<LiveRobotMarker>>,
    mut gizmos: Gizmos,
) {
    if !state.is_connected {
        return;
    }

    while let Ok(event) = plan_channel.receiver.try_recv() {
        if let Some(&path_entity) = path_map.0.get(&event.name) {
            if let Ok(mut path_marker) = path_query.get_mut(path_entity) {
                path_marker.waypoints = event.waypoints;
                path_marker.target_waypoint = 1;
                continue;
            } else {
                path_map.0.remove(&event.name);
            }
        }

        let path_entity = commands
            .spawn(LivePathMarker {
                name: event.name.clone(),
                waypoints: event.waypoints,
                target_waypoint: 1,
            })
            .id();
        path_map.0.insert(event.name, path_entity);
    }

    while let Ok(event) = progress_channel.receiver.try_recv() {
        if let Some(&path_entity) = path_map.0.get(&event.name) {
            if let Ok(mut path_marker) = path_query.get_mut(path_entity) {
                path_marker.target_waypoint = event.target_waypoint;
            }
        }
    }

    for path_marker in path_query.iter() {
        if path_marker.waypoints.is_empty() {
            continue;
        }

        let mut robot_pos = None;
        if let Some(&robot_entity) = robot_map.0.get(&path_marker.name) {
            if let Ok(transform) = robot_query.get(robot_entity) {
                robot_pos = Some(Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    PLANNED_PATH_Z_OFFSET,
                ));
            }
        }

        if let Some(start_pos) = robot_pos {
            let mut target_idx = path_marker.target_waypoint;
            // If the network hasn't given us a Progress update (still at default 1),
            // we dynamically infer the robot's progress by finding the closest waypoint.
            if target_idx <= 1 && path_marker.waypoints.len() > 1 {
                let mut min_dist = f32::MAX;
                let mut closest_idx = 0;

                for (i, wp) in path_marker.waypoints.iter().enumerate() {
                    let dist = start_pos.distance(*wp);
                    if dist < min_dist {
                        min_dist = dist;
                        closest_idx = i;
                    }
                }

                // If the closest waypoint is the absolute last dot on the path, it has arrived!
                if closest_idx == path_marker.waypoints.len() - 1 {
                    continue; // Skip rendering this path entirely
                }

                // Otherwise, assume the robot is heading to the waypoint *after* the closest one
                target_idx = closest_idx + 1;
            }

            let final_target_idx = target_idx.min(path_marker.waypoints.len().saturating_sub(1));

            if final_target_idx < path_marker.waypoints.len() {
                let mut points_to_draw = vec![start_pos];

                points_to_draw.extend_from_slice(&path_marker.waypoints[final_target_idx..]);

                if points_to_draw.len() > 1 {
                    gizmos.linestrip(points_to_draw, PLANNED_PATH_COLOR);
                }
            }
        }
    }
}

pub async fn handle_plan_stream(
    robot_name: String,
    client: ClientHandle,
    sender: Sender<LiveEventPlan>,
    connect_flag: Arc<AtomicBool>,
) {
    let topic_name = format!("/{}/plan", robot_name);

    if let Ok(plan_sub) = client.subscribe::<Plan>(&topic_name).await {
        loop {
            let plan_msg = plan_sub.next().await;

            if !connect_flag.load(Ordering::Relaxed) {
                break;
            }

            let waypoints: Vec<Vec3> = plan_msg
                .waypoints
                .iter()
                .map(|wp| Vec3::new(wp.position[0], wp.position[1], 0.05))
                .collect();

            if let Err(e) = sender.send(LiveEventPlan {
                name: robot_name.clone(),
                waypoints,
            }) {
                error!("Failed to send Plan event across channel: {}", e);
                break;
            }
        }
    }
}

pub async fn handle_progress_stream(
    robot_name: String,
    client: ClientHandle,
    sender: Sender<LiveEventProgress>,
    connect_flag: Arc<AtomicBool>,
) {
    let topic_name = format!("/{}/plan/progress", robot_name);

    if let Ok(prog_sub) = client.subscribe::<Progress>(&topic_name).await {
        loop {
            let prog_msg = prog_sub.next().await;

            if !connect_flag.load(Ordering::Relaxed) {
                break;
            }

            if let Err(e) = sender.send(LiveEventProgress {
                name: robot_name.clone(),
                target_waypoint: prog_msg.target_waypoint as usize,
            }) {
                error!("Failed to send Progress event across channel: {}", e);
                break;
            }
        }
    }
}
