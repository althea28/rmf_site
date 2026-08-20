/*
 * Copyright (C) 2024 Open Source Robotics Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
*/

use bevy::{
    ecs::system::SystemState,
    prelude::*,
};
use bevy_egui::egui::{self, Color32, RichText, ScrollArea, Ui, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use serde::{Deserialize, Serialize};

use crate::{
    widgets::{
        building_preview::BuildingPreview,
        inspector::{Inspector, MainInspector},
        view_groups::ViewGroups,
        view_layers::ViewLayers,
        view_levels::ViewLevels,
        view_lights::ViewLights,
        view_model_instances::ViewModelInstances,
        view_nav_graphs::ViewNavGraphs,
        view_scenarios::ViewScenarios,
    },
    AppState,
};
use rmf_site_egui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PropertyTab {
    Levels,
    Scenarios,
    Models,
    Navigation,
    Layers,
    Inspect,
    Groups,
    Lights,
    BuildingPreview,
}

impl PropertyTab {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Levels => "Levels",
            Self::Scenarios => "Scenarios",
            Self::Models => "Models",
            Self::Navigation => "Navigation",
            Self::Layers => "Layers",
            Self::Inspect => "Inspect",
            Self::Groups => "Groups",
            Self::Lights => "Lights",
            Self::BuildingPreview => "Preview",
        }
    }

    pub fn all() -> &'static [PropertyTab] {
        &[
            Self::Levels,
            Self::Scenarios,
            Self::Models,
            Self::Navigation,
            Self::Layers,
            Self::Inspect,
            Self::Groups,
            Self::Lights,
            Self::BuildingPreview,
        ]
    }

    pub fn default_tabs() -> Vec<PropertyTab> {
        vec![
            Self::Levels,
            Self::Scenarios,
            Self::Models,
            Self::Navigation,
            Self::Layers,
            Self::Inspect,
            Self::Groups,
            Self::Lights,
            Self::BuildingPreview,
        ]
    }
}

#[derive(Resource)]
pub struct PropertiesPanelState {
    pub dock_state: DockState<PropertyTab>,
}

impl FromWorld for PropertiesPanelState {
    fn from_world(_world: &mut World) -> Self {
        let top_tabs = vec![
            PropertyTab::Levels,
            PropertyTab::Scenarios,
            PropertyTab::Models,
            PropertyTab::Navigation,
        ];
        let bottom_tabs = vec![
            PropertyTab::Layers,
            PropertyTab::Inspect,
            PropertyTab::Groups,
            PropertyTab::Lights,
            PropertyTab::BuildingPreview,
        ];

        let mut dock_state = DockState::new(top_tabs);
        dock_state
            .main_surface_mut()
            .split_below(NodeIndex::root(), 0.5, bottom_tabs);
        Self { dock_state }
    }
}

#[derive(Resource)]
pub struct PropertiesTabStates {
    pub levels: SystemState<ViewLevels<'static, 'static>>,
    pub scenarios: SystemState<ViewScenarios<'static, 'static>>,
    pub models: SystemState<ViewModelInstances<'static, 'static>>,
    pub nav_graphs: SystemState<ViewNavGraphs<'static, 'static>>,
    pub layers: SystemState<ViewLayers<'static, 'static>>,
    pub groups: SystemState<ViewGroups<'static, 'static>>,
    pub lights: SystemState<ViewLights<'static, 'static>>,
    pub building_preview: SystemState<BuildingPreview<'static>>,
    pub inspector: SystemState<Inspector<'static, 'static>>,
}

impl FromWorld for PropertiesTabStates {
    fn from_world(world: &mut World) -> Self {
        Self {
            levels: SystemState::new(world),
            scenarios: SystemState::new(world),
            models: SystemState::new(world),
            nav_graphs: SystemState::new(world),
            layers: SystemState::new(world),
            groups: SystemState::new(world),
            lights: SystemState::new(world),
            building_preview: SystemState::new(world),
            inspector: SystemState::new(world),
        }
    }
}

pub struct PropertiesTabViewer<'a> {
    pub world: &'a mut World,
    pub settings: PanelSettings,
}

impl<'a> TabViewer for PropertiesTabViewer<'a> {
    type Tab = PropertyTab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title().into()
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        let tab = *tab;
        let settings = self.settings;

        macro_rules! render_tab {
            ($state:expr, $world:expr, $ui:expr) => {{
                let mut params = $state.get_mut($world);
                params.show_widget($ui);
                $state.apply($world);
            }};
        }

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.world.resource_scope::<PropertiesTabStates, ()>(|world, mut states| {
                    match tab {
                        PropertyTab::Levels => render_tab!(states.levels, world, ui),
                        PropertyTab::Scenarios => render_tab!(states.scenarios, world, ui),
                        PropertyTab::Models => render_tab!(states.models, world, ui),
                        PropertyTab::Navigation => render_tab!(states.nav_graphs, world, ui),
                        PropertyTab::Layers => render_tab!(states.layers, world, ui),
                        PropertyTab::Groups => render_tab!(states.groups, world, ui),
                        PropertyTab::Lights => render_tab!(states.lights, world, ui),
                        PropertyTab::BuildingPreview => render_tab!(states.building_preview, world, ui),
                        PropertyTab::Inspect => {
                            let main_inspector_id = world.get_resource::<MainInspector>().map(|m| m.get());
                            if let Some(main_inspector_id) = main_inspector_id {
                                Inspector::show_inspector(
                                    &mut states.inspector,
                                    world,
                                    main_inspector_id,
                                    settings,
                                    ui,
                                );
                            }
                        }
                    }
                });
            });
    }
}

pub fn show_properties_panel(
    In(PanelWidgetInput { id, context }): In<PanelWidgetInput>,
    world: &mut World,
) {
    let in_display_mode = world
        .get_resource::<State<AppState>>()
        .is_some_and(|s| match s.get() {
            AppState::MainMenu => false,
            AppState::SiteEditor | AppState::SiteVisualizer | AppState::SiteDrawingEditor => true,
        });

    if !in_display_mode {
        return;
    }

    let settings = world.get::<PanelSettings>(id).copied().unwrap_or(PanelSettings::right());

    let mut style = Style::from_egui(context.style().as_ref());
    style.tab_bar.show_scroll_bar_on_overflow = true;
    style.tab_bar.fill_tab_bar = false;
    style.tab_bar.height = 24.0;
    style.buttons.close_tab_active_color = Color32::from_rgb(240, 80, 80);

    let config = world.get::<PanelConfig>(id).copied().unwrap_or_default();

    egui::SidePanel::right("properties_panel")
        .resizable(config.resizable)
        .default_width(config.default_dimension)
        .min_width(200.0)
        .max_width(800.0)
        .show(&context, |ui| {
            world.resource_scope::<PropertiesPanelState, ()>(|world, mut dock_state| {
                // Add button in top header bar to manage closed/open tabs
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("tabs_manager_menu")
                        .selected_text("Tabs")
                        .width(0.0)
                        .height(400.0)
                        .show_ui(ui, |ui| {
                            for &tab in PropertyTab::all() {
                                let is_open = dock_state.dock_state.find_tab(&tab).is_some();
                                if ui.selectable_label(is_open, tab.title()).clicked() {
                                    if is_open {
                                        if let Some(index) = dock_state.dock_state.find_tab(&tab) {
                                            dock_state.dock_state.remove_tab(index);
                                        }
                                    } else {
                                        dock_state.dock_state.main_surface_mut().push_to_focused_leaf(tab);
                                    }
                                }
                            }

                            ui.separator();
                            if ui.button("Restore All Tabs").clicked() {
                                for &tab in PropertyTab::all() {
                                    if dock_state.dock_state.find_tab(&tab).is_none() {
                                        dock_state.dock_state.main_surface_mut().push_to_focused_leaf(tab);
                                    }
                                }
                            }
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Properties").weak());
                    });
                });

                ui.separator();

                let mut tab_viewer = PropertiesTabViewer {
                    world,
                    settings,
                };

                DockArea::new(&mut dock_state.dock_state)
                    .style(style)
                    .show_close_buttons(true)
                    .draggable_tabs(true)
                    .show_inside(ui, &mut tab_viewer);
            });
        });
}
