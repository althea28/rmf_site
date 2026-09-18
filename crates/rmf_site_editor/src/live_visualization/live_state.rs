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

pub fn auto_fetch_site_on_connect(
    mut state: ResMut<LiveStreamState>,
    mut workspace_loader: crate::WorkspaceLoader,
) {
    let is_currently_active = state.connection_active.load(Ordering::Relaxed);

    if is_currently_active && !state.site_loaded {
        state.site_loaded = true;

        println!(
            "Network connected! Automatically downloading site data from: {}",
            state.site_url
        );

        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = ehttp::Request::get(&state.site_url);

        ehttp::fetch(request, move |result| {
            let _ = tx.send(result);
        });

        workspace_loader.load_site(async move {
            if let Ok(Ok(response)) = rx.await {
                if response.status == 200 {
                    println!("Successfully downloaded and applied live site map!");
                    return crate::site::LoadSite::from_data(&response.bytes, None);
                } else {
                    println!(
                        "Backend returned HTTP Error {}: {}",
                        response.status, response.status_text
                    );
                }
            } else {
                println!("Failed to download site map from backend.");
            }
            Err(crate::site::LoadSiteError::UnknownDataFormat)
        });
    }
}
