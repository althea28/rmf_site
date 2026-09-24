use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy_egui::egui;
use rmf_site_egui::{Tile, WidgetSystem};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

pub const DEFAULT_CONNECTION_URL: &str = "ws://127.0.0.1:9090";
pub const DEFAULT_SITE_DATA_URL: &str = "http://127.0.0.1:8080/site_file";

#[derive(Resource)]
pub struct SiteFetchReceiver(pub UnboundedReceiver<Vec<u8>>);

#[derive(Resource)]
pub struct LiveStreamState {
    pub url: String,
    pub site_url: String,
    pub connection_requested: Arc<AtomicBool>,
    pub connection_active: Arc<AtomicBool>,
    pub site_loaded: bool,
}

impl Default for LiveStreamState {
    fn default() -> Self {
        Self {
            url: DEFAULT_CONNECTION_URL.to_string(),
            site_url: DEFAULT_SITE_DATA_URL.to_string(),
            connection_requested: Arc::new(AtomicBool::new(false)),
            connection_active: Arc::new(AtomicBool::new(false)),
            site_loaded: false,
        }
    }
}

// Holds receiver to receive site data asynchronously
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
                ui.label(egui::RichText::new("\u{2022}  Disconnected").color(egui::Color32::RED));
            }
        });
    }
}

pub fn auto_fetch_site_on_connect(mut state: ResMut<LiveStreamState>, mut commands: Commands) {
    let is_currently_active = state.connection_active.load(Ordering::Relaxed);

    if is_currently_active && !state.site_loaded {
        state.site_loaded = true;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        commands.insert_resource(SiteFetchReceiver(rx));

        let request = ehttp::Request::get(&state.site_url);
        ehttp::fetch(request, move |result| {
            if let Ok(response) = result {
                if response.status == 200 {
                    let _ = tx.send(response.bytes);
                }
            }
        });
    }
}

// Spawns site from data once all site data bytes are ready
pub fn process_site_download(
    mut commands: Commands,
    receiver: Option<ResMut<SiteFetchReceiver>>,
    mut load_site: EventWriter<crate::site::LoadSite>,
) {
    if let Some(mut rx) = receiver {
        if let Ok(bytes) = rx.0.try_recv() {
            if let Ok(mut site) = crate::site::LoadSite::from_data(&bytes, None) {
                site.focus = true;
                load_site.write(site);
            }
            commands.remove_resource::<SiteFetchReceiver>();
        }
    }
}
