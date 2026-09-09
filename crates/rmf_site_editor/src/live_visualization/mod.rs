pub mod connection_window;
pub mod network_client;
pub mod robot_odometry;

use bevy::prelude::*;
use crossbeam_channel::unbounded;
use rmf_site_egui::{HeaderPanel, HeaderTilePlugin};

use connection_window::{LiveStreamButton, LiveStreamState};
use network_client::StreamChannel;
use robot_odometry::{update_live_robots, LiveEventOdom};

pub struct LiveVisualizationPlugin;

impl Plugin for LiveVisualizationPlugin {
    fn build(&self, app: &mut App) {
        let (odom_tx, odom_rx) = unbounded();

        app.insert_resource(StreamChannel::<LiveEventOdom> {
            sender: odom_tx,
            receiver: odom_rx,
        })
        .init_resource::<LiveStreamState>()
        .add_systems(Update, update_live_robots);

        if app.world().get_resource::<HeaderPanel>().is_some() {
            app.add_plugins(HeaderTilePlugin::<LiveStreamButton>::new());
        }
    }
}
