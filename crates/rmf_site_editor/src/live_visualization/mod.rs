pub mod connection_window;
pub mod network_client;
pub mod odometry;
pub mod planned_paths;

use bevy::prelude::*;
use rmf_site_egui::{HeaderPanel, HeaderTilePlugin};

use connection_window::{LiveStreamButton, LiveStreamState};
use network_client::StreamPlugin;
use odometry::{update_live_robots, LiveEventOdom, LiveRobotsMap};
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
        .add_systems(Update, (update_live_robots, update_live_paths));

        if app.world().get_resource::<HeaderPanel>().is_some() {
            app.add_plugins(HeaderTilePlugin::<LiveStreamButton>::new());
        }
    }
}
