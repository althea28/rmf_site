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

use crate::interaction::Hovering;
use bevy::prelude::*;
use rmf_site_camera::{active_camera_maybe, ActiveCameraQuery};
use rmf_site_egui::canvas_tooltips::CanvasTooltips;
use rmf_site_picking::Hovered;
use std::borrow::Cow;

#[derive(Component, Clone, Debug, Default)]
pub struct Billboard {
    pub offset: Vec3,
}

#[derive(Component, Clone, Debug)]
pub struct BillboardTooltip(pub String);

fn new_billboard_position(billboard_vec: Vec3, camera_vec: Vec3) -> Vec3 {
    let radius = billboard_vec.length();

    if radius == 0.0 {
        return Vec3::ZERO;
    }

    let c_norm = camera_vec.normalize();
    let ad_vec = (billboard_vec - billboard_vec.dot(c_norm) * c_norm).normalize_or_zero();

    if ad_vec.length_squared() <= f32::EPSILON {
        let mut fallback_vec = Vec3::X;
        if c_norm.x.abs() > 0.9 {
            fallback_vec = Vec3::Y;
        }

        let fallback_dir = c_norm.cross(fallback_vec).normalize();
        return fallback_dir * radius;
    }
    ad_vec * radius
}

pub fn update_billboard_transforms(
    mut query_mesh: Query<(&mut Transform, &Billboard)>,
    query_cameras: Query<(&Projection, &GlobalTransform)>,
    active_camera: ActiveCameraQuery,
) {
    let Ok(active_camera_entity) = active_camera_maybe(&active_camera) else {
        return;
    };
    let Ok((_camera_projection, camera_transform)) = query_cameras.get(active_camera_entity) else {
        return;
    };

    let camera_direction = camera_transform.forward().into();

    for (mut transform, billboard) in &mut query_mesh {
        let new_position: Vec3 = new_billboard_position(billboard.offset, camera_direction);
        transform.translation = new_position;

        transform.rotation = Transform::IDENTITY
            .aligned_by(
                Dir3::Z,
                Dir3::new(-camera_direction).unwrap(),
                Dir3::Y,
                Dir3::new(new_position).unwrap(),
            )
            .rotation;
    }
}

pub fn update_billboard_text_hover_visualisation(
    mut tooltips: ResMut<CanvasTooltips>,
    hovering: Res<Hovering>,
    query_tooltips: Query<(&Hovered, &BillboardTooltip)>,
) {
    if let Some(hovering) = hovering.0 {
        if let Ok((hovered, tooltip)) = query_tooltips.get(hovering) {
            if hovered.cue() {
                tooltips.add(Cow::Owned(tooltip.0.clone()));
            }
        }
    }
}
