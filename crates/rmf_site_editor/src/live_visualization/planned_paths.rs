use bevy::prelude::*;
use crossbeam_channel::Sender;
use rmf_site_msgs::rmf_prototype_msgs::msg::{Plan, Progress};
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::connection_window::LiveStreamState;
use super::network_client::StreamChannel;
use super::odometry::{LiveRobotMarker, LiveRobotsMap};

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

#[derive(Default, Resource)]
pub struct LivePathsState(pub HashMap<String, RobotPathData>);

pub struct RobotPathData {
    pub waypoints: Vec<Vec3>,
    pub target_waypoint: usize,
}

pub fn update_live_paths(
    state: Res<LiveStreamState>,
    plan_channel: Res<StreamChannel<LiveEventPlan>>,
    progress_channel: Res<StreamChannel<LiveEventProgress>>,
    mut path_state: ResMut<LivePathsState>,
    robot_map: Res<LiveRobotsMap>,
    robot_query: Query<&Transform, With<LiveRobotMarker>>,
    mut gizmos: Gizmos,
) {
    if !state.is_connected {
        path_state.0.clear();
        return;
    }

    while let Ok(event) = plan_channel.receiver.try_recv() {
        let robot_path = path_state
            .0
            .entry(event.name.clone())
            .or_insert(RobotPathData {
                waypoints: Vec::new(),
                target_waypoint: 1,
            });

        if robot_path.waypoints != event.waypoints {
            // Check if this is a detour/new path
            let has_existing_path = !robot_path.waypoints.is_empty();
            robot_path.waypoints = event.waypoints;

            // If the robot already had a path, this is a brand new detour.
            // Target is reset to the beginning of the new path.
            // If it did not have an existing path, the Progress message arrived
            // first, so the target_waypoint is left alone.
            if has_existing_path {
                robot_path.target_waypoint = 1;
            }
        }
    }

    while let Ok(event) = progress_channel.receiver.try_recv() {
        let robot_path = path_state
            .0
            .entry(event.name.clone())
            .or_insert(RobotPathData {
                waypoints: Vec::new(),
                target_waypoint: event.target_waypoint,
            });
        robot_path.target_waypoint = event.target_waypoint;
    }

    for (name, path_data) in path_state.0.iter() {
        if path_data.waypoints.is_empty() {
            continue;
        }

        let mut robot_pos = None;
        if let Some(&robot_entity) = robot_map.0.get(name) {
            if let Ok(transform) = robot_query.get(robot_entity) {
                robot_pos = Some(Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    PLANNED_PATH_Z_OFFSET,
                ));
            }
        }

        if let Some(start_pos) = robot_pos {
            let final_target_idx = path_data
                .target_waypoint
                .min(path_data.waypoints.len().saturating_sub(1));

            // Draw line from robot's current position to the target waypoint, then along the path to the final waypoint.
            if final_target_idx < path_data.waypoints.len() {
                let mut points_to_draw = vec![start_pos];
                points_to_draw.extend_from_slice(&path_data.waypoints[final_target_idx..]);

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
