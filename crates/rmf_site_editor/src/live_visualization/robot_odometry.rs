use bevy::prelude::*;
use rmf_site_format::NameInSite;

use super::network_client::StreamChannel;

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

pub fn update_live_robots(
    channel: Res<StreamChannel<LiveEventOdom>>,
    time: Res<Time>,
    mut commands: Commands,
    mut live_robots_query: Query<(&mut LiveRobotMarker, &mut Transform)>,
    mut untracked_entities_query: Query<
        (Entity, &NameInSite, &mut Transform),
        Without<LiveRobotMarker>,
    >,
) {
    // Wait for message from channel receiver
    while let Ok(event) = channel.receiver.try_recv() {
        let target_pos = Vec3::new(event.x, event.y, event.z);
        let target_rot = Quat::from_rotation_z(event.yaw);
        let mut found = false;

        // Update target position of tracked robot
        for (mut robot, _) in live_robots_query.iter_mut() {
            if robot.name == event.name {
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
            if name_in_site.0 == event.name {
                transform.translation = target_pos;
                transform.rotation = target_rot;
                commands.entity(entity).insert(LiveRobotMarker {
                    name: event.name.clone(),
                    target_translation: target_pos,
                    target_rotation: target_rot,
                });
                found = true;
                break;
            }
        }

        if !found {
            println!("Robot {} not found in the scene.", event.name);
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
