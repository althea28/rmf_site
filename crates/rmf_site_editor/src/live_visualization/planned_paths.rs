use bevy::prelude::*;
use crossbeam_channel::Sender;
use rmf_site_msgs::rmf_prototype_msgs::msg::{Plan, Progress};
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::live_state::LiveStreamState;
use super::network_client::{spawn_network_task, LiveStreamHandler, VisualizationStreamChannel};
use super::odometry::{LiveRobotMarker, LiveRobotsMap};

pub const PLANNED_PATH_Z_OFFSET: f32 = 0.05;
pub const PLANNED_PATH_COLOR: Color = Color::srgb(0.0, 1.0, 0.0);

#[derive(Debug, Clone, PartialEq)]
pub struct LiveWaypoint {
    pub position: Vec3,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct LiveEventPlan {
    pub name: String,
    pub waypoints: Vec<LiveWaypoint>,
}

impl LiveStreamHandler for LiveEventPlan {
    fn spawn_stream(
        robot_name: String,
        client: ClientHandle,
        sender: Sender<Self>,
        connection_requested: Arc<AtomicBool>,
    ) {
        let topic_name = format!("/{}/plan", robot_name);

        let task = async move {
            if let Ok(plan_sub) = client.subscribe::<Plan>(&topic_name).await {
                loop {
                    let plan_msg = plan_sub.next().await;

                    if !connection_requested.load(Ordering::Relaxed) {
                        break;
                    }

                    let waypoints: Vec<LiveWaypoint> = plan_msg
                        .waypoints
                        .iter()
                        .map(|wp| LiveWaypoint {
                            position: Vec3::new(
                                wp.position[0] as f32,
                                wp.position[1] as f32,
                                PLANNED_PATH_Z_OFFSET,
                            ),
                            progress: wp.progress,
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
        };
        spawn_network_task(task);
    }
}

#[derive(Debug, Clone)]
pub struct LiveEventProgress {
    pub name: String,
    pub target_waypoint: usize,
    pub progress: f32,
}

impl LiveStreamHandler for LiveEventProgress {
    fn spawn_stream(
        robot_name: String,
        client: ClientHandle,
        sender: Sender<Self>,
        connection_requested: Arc<AtomicBool>,
    ) {
        let topic_name = format!("/{}/plan/progress", robot_name);

        let task = async move {
            if let Ok(prog_sub) = client.subscribe::<Progress>(&topic_name).await {
                loop {
                    let prog_msg = prog_sub.next().await;

                    if !connection_requested.load(Ordering::Relaxed) {
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
        };
        spawn_network_task(task);
    }
}

#[derive(Default, Resource)]
pub struct LivePathsState(pub HashMap<String, PlannedPathData>);

pub struct PlannedPathData {
    pub waypoints: Vec<LiveWaypoint>,
    pub target_waypoint: usize,
    pub current_progress: f32,
}

impl PlannedPathData {
    pub fn is_completed(&self) -> bool {
        self.waypoints.is_empty()
            || self.target_waypoint >= self.waypoints.len()
            || self
                .waypoints
                .last()
                .is_some_and(|last_wp| self.current_progress >= last_wp.progress)
    }
}

pub fn update_live_paths(
    state: Res<LiveStreamState>,
    plan_channel: Res<VisualizationStreamChannel<LiveEventPlan>>,
    progress_channel: Res<VisualizationStreamChannel<LiveEventProgress>>,
    mut path_state: ResMut<LivePathsState>,
    robot_map: Res<LiveRobotsMap>,
    robot_query: Query<&Transform, With<LiveRobotMarker>>,
    mut gizmos: Gizmos,
) {
    if !state.connection_requested.load(Ordering::Relaxed) {
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
                current_progress: f32::MAX,
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
                robot_path.current_progress = 0.0;
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
        if path_data.is_completed() {
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
    }
}
