use bevy::prelude::*;

use super::network_client::StreamChannel;
use bevy::color::palettes::css as Colors;

#[derive(Debug, Clone)]
pub struct LiveEventPlan {
    pub name: String,
    pub waypoints: Vec<Vec3>,
}

#[derive(Component)]
pub struct LivePathMarker {
    pub name: String,
    pub waypoints: Vec<Vec3>,
}

pub fn update_live_paths(
    channel: Res<StreamChannel<LiveEventPlan>>,
    mut commands: Commands,
    mut path_query: Query<&mut LivePathMarker>,
    mut gizmos: Gizmos,
) {
    while let Ok(event) = channel.receiver.try_recv() {
        let mut found = false;

        for mut path_marker in path_query.iter_mut() {
            if path_marker.name == event.name {
                path_marker.waypoints = event.waypoints.clone();
                found = true;
                break;
            }
        }

        if !found {
            commands.spawn(LivePathMarker {
                name: event.name.clone(),
                waypoints: event.waypoints,
            });
        }
    }

    for path_marker in path_query.iter() {
        if path_marker.waypoints.len() > 1 {
            gizmos.linestrip(path_marker.waypoints.clone(), Colors::GREEN);
        }
    }
}
