pub mod connection_window;
pub mod network_client;
pub mod odometry;
pub mod planned_paths;

use bevy::prelude::*;
use rmf_site_egui::{HeaderPanel, HeaderTilePlugin};
use std::sync::atomic::Ordering;

use connection_window::{LiveStreamState, LiveStreamStatusWidget};
use network_client::StreamPlugin;
use odometry::{update_live_robots, LiveEventOdom, LiveRobotMarker, LiveRobotsMap};
use planned_paths::{update_live_paths, LiveEventPlan, LiveEventProgress, LivePathsState};

pub struct LiveVisualizationPlugin;

impl Plugin for LiveVisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            StreamPlugin::<LiveEventOdom>::default(),
            StreamPlugin::<LiveEventPlan>::default(),
            StreamPlugin::<LiveEventProgress>::default(),
        ))
        .init_resource::<LiveStreamState>()
        .init_resource::<LiveRobotsMap>()
        .init_resource::<LivePathsState>()
        .add_systems(Update, (update_live_robots, update_live_paths))
        .add_systems(OnEnter(crate::AppState::MainMenu), disconnect_live_stream);

        if app.world().get_resource::<HeaderPanel>().is_some() {
            app.add_plugins(HeaderTilePlugin::<LiveStreamStatusWidget>::new());
        }
    }
}

fn disconnect_live_stream(
    mut commands: Commands,
    state: Res<LiveStreamState>,
    mut robot_map: ResMut<LiveRobotsMap>,
    mut path_state: ResMut<LivePathsState>,
    live_robots: Query<Entity, With<LiveRobotMarker>>,
) {
    state.connection_requested.store(false, Ordering::Relaxed);
    state.connection_active.store(false, Ordering::Relaxed);
    robot_map.0.clear();
    path_state.0.clear();
    for entity in live_robots.iter() {
        commands.entity(entity).despawn();
    }
}
