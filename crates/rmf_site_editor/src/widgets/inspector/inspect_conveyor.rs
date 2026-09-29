/*
 * Copyright (C) 2026 Open Source Robotics Foundation
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

use crate::{
    site::{Anchor, Change, ConveyorHeight, Dependents},
    widgets::{prelude::*, Inspect},
};
use bevy::prelude::*;
use bevy_egui::egui::{TextEdit, Ui};
use rmf_site_egui::WidgetSystem;
use rmf_site_format::{ConveyorMarker, Edge};
use std::collections::{BTreeSet, VecDeque};

#[derive(SystemParam)]
pub struct InspectConveyor<'w, 's> {
    commands: Commands<'w, 's>,
    conveyors: Query<
        'w,
        's,
        (Entity, &'static Edge<Entity>, &'static ConveyorHeight),
        With<ConveyorMarker>,
    >,
    dependents: Query<'w, 's, &'static Dependents, With<Anchor>>,
}

impl<'w, 's> WidgetSystem<Inspect> for InspectConveyor<'w, 's> {
    fn show(
        Inspect { selection, .. }: Inspect,
        ui: &mut Ui,
        state: &mut SystemState<Self>,
        world: &mut World,
    ) {
        let mut params = state.get_mut(world);
        let Ok((_, _, height)) = params.conveyors.get(selection) else {
            return;
        };

        let id = ui.make_persistent_id(("conveyor_height", selection));
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| format!("{:.2}", height.0));

        let mut changed_val = None;

        ui.horizontal(|ui| {
            ui.label("height");
            let response = ui.add(TextEdit::singleline(&mut text).desired_width(60.0));
            if response.changed() {
                let mut has_dot = false;
                let filtered: String = text
                    .chars()
                    .filter(|c| {
                        if c.is_ascii_digit() {
                            true
                        } else if *c == '.' && !has_dot {
                            has_dot = true;
                            true
                        } else {
                            false
                        }
                    })
                    .collect();
                text = filtered;
                ui.data_mut(|d| d.insert_temp(id, text.clone()));

                if let Ok(val) = text.parse::<f32>() {
                    if val >= 0.0 && val != height.0 {
                        changed_val = Some(val);
                    }
                }
            } else if !response.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, format!("{:.2}", height.0)));
            }
        });

        if let Some(new_height) = changed_val {
            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::new();
            visited.insert(selection);
            queue.push_back(selection);

            while let Some(current) = queue.pop_front() {
                if let Ok((_, edge, _)) = params.conveyors.get(current) {
                    for anchor in [edge.start(), edge.end()] {
                        if let Ok(deps) = params.dependents.get(anchor) {
                            for dep in deps.iter() {
                                if params.conveyors.contains(*dep) && visited.insert(*dep) {
                                    queue.push_back(*dep);
                                }
                            }
                        }
                    }
                }
            }

            for conveyor in visited {
                params
                    .commands
                    .trigger(Change::new(ConveyorHeight(new_height), conveyor));
            }
        }
        ui.add_space(10.0);
    }
}
