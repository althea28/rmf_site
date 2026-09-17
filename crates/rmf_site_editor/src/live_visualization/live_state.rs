use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy_egui::egui;
use rmf_site_egui::{Tile, WidgetSystem};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const DEFAULT_CONNECTION_URL: &str = "ws://127.0.0.1:9090";
pub const DEFAULT_SITE_DATA_URL: &str = "http://127.0.0.1:8080/site_file";

#[derive(Resource)]
pub struct LiveStreamState {
    pub url: String,
    pub site_url: String,
    pub connection_requested: Arc<AtomicBool>,
    pub connection_active: Arc<AtomicBool>,
}

impl Default for LiveStreamState {
    fn default() -> Self {
        Self {
            url: DEFAULT_CONNECTION_URL.to_string(),
            site_url: DEFAULT_SITE_DATA_URL.to_string(),
            connection_requested: Arc::new(AtomicBool::new(false)),
            connection_active: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(SystemParam)]
pub struct LiveStreamStatusWidget<'w> {
    state: Res<'w, LiveStreamState>,
}

impl<'w> WidgetSystem<Tile> for LiveStreamStatusWidget<'w> {
    fn show(_: Tile, ui: &mut egui::Ui, state: &mut SystemState<Self>, world: &mut World) {
        let params = state.get(world);

        if !params.state.connection_requested.load(Ordering::Relaxed) {
            return;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if params.state.connection_active.load(Ordering::Relaxed) {
                ui.label(egui::RichText::new("\u{2022}  Connected").color(egui::Color32::GREEN));
            } else {
                ui.label(egui::RichText::new("\u{2022}  Disconnected").color(egui::Color32::WHITE));
            }
        });
    }
}
