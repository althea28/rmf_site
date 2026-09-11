use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy_egui::egui::{self, Ui};
use rmf_site_egui::{Tile, WidgetSystem};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::network_client::{start_rosbridge_subscriber, NetworkSenders, StreamChannel};
use super::planned_paths::{LiveEventPlan, LiveEventProgress};
use super::robot_odometry::LiveEventOdom;
use crate::workspace::CurrentWorkspace;

pub const DEFAULT_CONNECTION_URL: &str = "ws://127.0.0.1:9090";

#[derive(Resource)]
pub struct LiveStreamState {
    pub is_connected: bool,
    pub connect_flag: Arc<AtomicBool>,
}

impl Default for LiveStreamState {
    fn default() -> Self {
        Self {
            is_connected: false,
            connect_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn draw_live_stream_button(
    ui: &mut Ui,
    state: &mut LiveStreamState,
    odom_channel: &StreamChannel<LiveEventOdom>,
    plan_channel: &StreamChannel<LiveEventPlan>,
    prog_channel: &StreamChannel<LiveEventProgress>,
) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if state.is_connected {
            let connected_text = egui::RichText::new("🔘 Connected").color(egui::Color32::GREEN);

            if ui.button(connected_text).clicked() {
                state.is_connected = false;
                state.connect_flag.store(false, Ordering::Relaxed);
            }
        } else {
            let disconnected_text = egui::RichText::new("🔘 Connect").color(egui::Color32::WHITE);

            if ui.button(disconnected_text).clicked() {
                state.is_connected = true;
                state.connect_flag.store(true, Ordering::Relaxed);

                let senders = NetworkSenders {
                    odom: odom_channel.sender.clone(),
                    plan: plan_channel.sender.clone(),
                    progress: prog_channel.sender.clone(),
                };

                start_rosbridge_subscriber(
                    DEFAULT_CONNECTION_URL,
                    senders,
                    state.connect_flag.clone(),
                );
            }
        }
    });
}

#[derive(SystemParam)]
pub struct LiveStreamButton<'w> {
    state: ResMut<'w, LiveStreamState>,
    odom_channel: Res<'w, StreamChannel<LiveEventOdom>>,
    plan_channel: Res<'w, StreamChannel<LiveEventPlan>>,
    prog_channel: Res<'w, StreamChannel<LiveEventProgress>>,
    workspace: Option<Res<'w, CurrentWorkspace>>,
}

impl<'w> WidgetSystem<Tile> for LiveStreamButton<'w> {
    fn show(_: Tile, ui: &mut Ui, state: &mut SystemState<Self>, world: &mut World) {
        let mut params = state.get_mut(world);
        let is_map_loaded = params
            .workspace
            .as_ref()
            .map(|w| w.root.is_some())
            .unwrap_or(false);
        if !is_map_loaded {
            return;
        }

        draw_live_stream_button(
            ui,
            &mut params.state,
            &params.odom_channel,
            &params.plan_channel,
            &params.prog_channel,
        );
    }
}
