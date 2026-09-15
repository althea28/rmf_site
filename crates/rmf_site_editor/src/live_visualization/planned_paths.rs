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

pub const DEPENDENCY_Z_OFFSET: f32 = 0.051;
pub const DEPENDENCY_COLOR: Color = Color::srgb(1.0, 0.5, 0.0);
pub const DEPENDECY_DASH_LENGTH: f32 = 0.15;
pub const DEPENDECY_GAP_LENGTH: f32 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct LiveBlocker {
    pub name: String,
    pub required_progress: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveWaypoint {
    pub position: Vec3,
    pub progress: f32,
    pub departure_blockers: Vec<LiveBlocker>,
}

#[derive(Debug, Clone)]
pub struct LiveEventPlan {
    pub name: String,
    pub waypoints: Vec<LiveWaypoint>,
}

#[derive(Debug, Clone)]
pub struct LiveEventProgress {
    pub name: String,
    pub target_waypoint: usize,
    pub progress: f32,
}

#[derive(Default, Resource)]
pub struct LivePathsState(pub HashMap<String, PlannedPathData>);

pub struct PlannedPathData {
    pub waypoints: Vec<LiveWaypoint>,
    pub target_waypoint: usize,
    pub current_progress: f32,
}

pub fn update_live_paths(
    state: Res<LiveStreamState>,
    time: Res<Time>,
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
            .or_insert(PlannedPathData {
                waypoints: Vec::new(),
                target_waypoint: 1,
                current_progress: 0.0,
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
            .or_insert(PlannedPathData {
                waypoints: Vec::new(),
                target_waypoint: event.target_waypoint,
                current_progress: event.progress,
            });
        robot_path.target_waypoint = event.target_waypoint;
        robot_path.current_progress = event.progress;
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
                points_to_draw.extend(
                    path_data.waypoints[final_target_idx..]
                        .iter()
                        .map(|wp| wp.position),
                );

                if points_to_draw.len() > 1 {
                    gizmos.linestrip(points_to_draw, PLANNED_PATH_COLOR);
                }
            }
        }

        for wp in &path_data.waypoints {
            // Disappear if the waiting robot has already passed this waypoint
            if path_data.current_progress >= wp.progress {
                continue;
            }

            for blocker in &wp.departure_blockers {
                if let Some(blocking_path) = path_state.0.get(&blocker.name) {
                    // Skip drawing if the dependency is fulfilled
                    if blocking_path.current_progress >= blocker.required_progress {
                        continue;
                    }

                    // Find the coordinates where the blocking robot will clear the dependency
                    let mut clearance_pos = None;
                    for blocking_wp in &blocking_path.waypoints {
                        if blocking_wp.progress >= blocker.required_progress {
                            clearance_pos = Some(Vec3::new(
                                blocking_wp.position.x,
                                blocking_wp.position.y,
                                DEPENDENCY_Z_OFFSET,
                            ));
                            break;
                        }
                    }

                    // Draw dependency line connecting the waiting point to the clearance point
                    if let Some(end_pos) = clearance_pos {
                        let start_pos =
                            Vec3::new(wp.position.x, wp.position.y, DEPENDENCY_Z_OFFSET);
                        let delta = end_pos - start_pos;
                        let distance = delta.length();

                        if distance > 0.0 {
                            let dir = delta / distance;
                            let pattern_length = DEPENDECY_DASH_LENGTH + DEPENDECY_GAP_LENGTH;
                            let speed = 0.5;
                            let offset = (time.elapsed_secs() * speed) % pattern_length;
                            let mut current_dist = offset - pattern_length;

                            // Draw dashed line along vector
                            while current_dist < distance {
                                let start_dist = current_dist.max(0.0);
                                let end_dist = (current_dist + DEPENDECY_DASH_LENGTH).min(distance);

                                if start_dist < end_dist {
                                    let segment_start = start_pos + dir * start_dist;
                                    let segment_end = start_pos + dir * end_dist;

                                    gizmos.line(segment_start, segment_end, DEPENDENCY_COLOR);
                                }

                                current_dist += pattern_length;
                            }
                        }
                    }
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

            let waypoints: Vec<LiveWaypoint> = plan_msg
                .waypoints
                .iter()
                .map(|wp| {
                    let blockers = wp
                        .departure_blockers
                        .iter()
                        .map(|b| LiveBlocker {
                            name: b.name.clone(),
                            required_progress: b.required_progress,
                        })
                        .collect();

                    LiveWaypoint {
                        position: Vec3::new(
                            wp.position[0] as f32,
                            wp.position[1] as f32,
                            PLANNED_PATH_Z_OFFSET,
                        ),
                        progress: wp.progress,
                        departure_blockers: blockers,
                    }
                })
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
                progress: prog_msg.progress,
            }) {
                error!("Failed to send Progress event across channel: {}", e);
                break;
            }
        }
    }
}
