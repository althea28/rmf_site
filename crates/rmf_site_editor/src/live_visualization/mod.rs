pub mod connection_window;
pub mod network_client;
pub mod planned_paths;
pub mod robot_odometry;

use bevy::prelude::*;
use crossbeam_channel::unbounded;
use rmf_site_egui::{HeaderPanel, HeaderTilePlugin};

use connection_window::{LiveStreamButton, LiveStreamState};
use network_client::StreamChannel;
use planned_paths::{update_live_paths, LiveEventPlan, LiveEventProgress, LivePathsMap};
use robot_odometry::{update_live_robots, LiveEventOdom, LiveRobotsMap};

pub struct LiveVisualizationPlugin;

impl Plugin for LiveVisualizationPlugin {
    fn build(&self, app: &mut App) {
        let (odom_tx, odom_rx) = unbounded();
        let (plan_tx, plan_rx) = unbounded();
        let (prog_tx, prog_rx) = unbounded();

        app.insert_resource(StreamChannel::<LiveEventOdom> {
            sender: odom_tx,
            receiver: odom_rx,
        })
        .insert_resource(StreamChannel::<LiveEventPlan> {
            sender: plan_tx,
            receiver: plan_rx,
        })
        .insert_resource(StreamChannel::<LiveEventProgress> {
            sender: prog_tx,
            receiver: prog_rx,
        })
        .init_resource::<LiveStreamState>()
        .init_resource::<LiveRobotsMap>()
        .init_resource::<LivePathsMap>()
        .add_systems(Update, (update_live_robots, update_live_paths));

        if app.world().get_resource::<HeaderPanel>().is_some() {
            app.add_plugins(HeaderTilePlugin::<LiveStreamButton>::new());
        }
    }
}
