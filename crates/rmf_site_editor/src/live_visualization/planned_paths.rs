use bevy::color::palettes::css as Colors;
use bevy::prelude::*;

use super::connection_window::LiveStreamState;
use super::network_client::StreamChannel;
use super::robot_odometry::LiveRobotMarker;

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

pub fn update_live_paths(
    state: Res<LiveStreamState>,
    plan_channel: Res<StreamChannel<LiveEventPlan>>,
    progress_channel: Res<StreamChannel<LiveEventProgress>>,
    mut commands: Commands,
    mut path_query: Query<(Entity, &mut LivePathMarker)>,
    robot_query: Query<(&LiveRobotMarker, &Transform)>,
    mut gizmos: Gizmos,
) {
    if !state.is_connected {
        return;
    }

    while let Ok(event) = plan_channel.receiver.try_recv() {
        let mut found = false;
        for (_, mut path_marker) in path_query.iter_mut() {
            if path_marker.name == event.name {
                path_marker.waypoints = event.waypoints.clone();
                path_marker.target_waypoint = 1;
                found = true;
                break;
            }
        }

        if !found {
            commands.spawn(LivePathMarker {
                name: event.name.clone(),
                waypoints: event.waypoints,
                target_waypoint: 1,
            });
        }
    }

    while let Ok(event) = progress_channel.receiver.try_recv() {
        for (_, mut path_marker) in path_query.iter_mut() {
            if path_marker.name == event.name {
                path_marker.target_waypoint = event.target_waypoint;
                break;
            }
        }
    }

    for (_, path_marker) in path_query.iter() {
        if path_marker.waypoints.is_empty() {
            continue;
        }

        let mut robot_pos = None;
        for (robot, transform) in robot_query.iter() {
            if robot.name == path_marker.name {
                robot_pos = Some(Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    0.05,
                ));
                break;
            }
        }

        if let Some(start_pos) = robot_pos {let mut target_idx = path_marker.target_waypoint;
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
                    gizmos.linestrip(points_to_draw, Color::srgb(0.0, 1.0, 0.0));
                }
            }
        }
    }
}
