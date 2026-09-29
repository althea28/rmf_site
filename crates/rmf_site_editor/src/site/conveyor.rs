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

use crate::site::{
    line_stroke_transform, Anchor, AnchorParams, Category, Dependents, EdgeLabels, SiteAssets,
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use rmf_site_format::{ConveyorHeight, ConveyorMarker, Edge};
use rmf_site_picking::Selectable;

const CONVEYOR_WIDTH: f32 = 0.4;
const CONVEYOR_THICKNESS: f32 = 0.05;
const CONVEYOR_BAR_SIZE: f32 = 0.03;
const CONVEYOR_BAR_ELEVATION: f32 = 0.05;
const CONVEYER_MARGIN: f32 = 0.02;
const CONVEYER_BELT_THICKNESS: f32 = 0.01;

#[derive(Component, Debug, Clone, Copy)]
pub struct ConveyorSegments {
    start: Entity,
    start_belt: Entity,
    mid: Entity,
    mid_belt: Entity,
    end: Entity,
    end_belt: Entity,
    start_left_stand: Entity,
    start_right_stand: Entity,
    start_bar: Entity,
    end_left_stand: Entity,
    end_right_stand: Entity,
    end_bar: Entity,
}

pub fn add_conveyor_visuals(
    mut commands: Commands,
    conveyors: Query<(Entity, &Edge<Entity>, Option<&ConveyorHeight>), Added<ConveyorMarker>>,
    mut dependents: Query<&mut Dependents, With<Anchor>>,
    anchors: AnchorParams,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<SiteAssets>,
) {
    if conveyors.is_empty() {
        return;
    }

    let cylinder_mesh = meshes.add(Cylinder::new(CONVEYOR_WIDTH / 2.0, CONVEYOR_THICKNESS));
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, CONVEYOR_THICKNESS));
    let stand_mesh = meshes.add(Cuboid::new(CONVEYOR_BAR_SIZE, CONVEYOR_BAR_SIZE, 1.0));
    let bar_mesh = meshes.add(Cuboid::new(
        CONVEYOR_BAR_SIZE,
        CONVEYOR_WIDTH,
        CONVEYOR_BAR_SIZE,
    ));
    let belt_cylinder_mesh = meshes.add(Cylinder::new(
        CONVEYOR_WIDTH / 2.0 - CONVEYER_MARGIN,
        CONVEYER_BELT_THICKNESS,
    ));
    let belt_cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, CONVEYER_BELT_THICKNESS));

    let flat_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

    for (e, edge, conveyor_height) in &conveyors {
        for anchor in &edge.array() {
            if let Ok(mut deps) = dependents.get_mut(*anchor) {
                deps.insert(e);
            }
        }

        let start_anchor = anchors
            .point_in_parent_frame_of(edge.start(), Category::Conveyor, e)
            .unwrap();
        let end_anchor = anchors
            .point_in_parent_frame_of(edge.end(), Category::Conveyor, e)
            .unwrap();

        let dp = end_anchor - start_anchor;
        let length = dp.length();
        let u = if length > 0.001 { dp / length } else { Vec3::X };
        let v = Vec3::new(-u.y, u.x, 0.0);
        let rot = Quat::from_rotation_z(dp.y.atan2(dp.x));

        let height = conveyor_height.map(|h| h.0).unwrap_or(0.5);
        let stand_height = (height - CONVEYOR_THICKNESS / 2.0).max(0.001);
        let stand_z = stand_height / 2.0 - height;
        let bar_z = CONVEYOR_BAR_ELEVATION + CONVEYOR_BAR_SIZE / 2.0 - height;

        let offset = (CONVEYOR_WIDTH / 2.0 + CONVEYOR_BAR_SIZE / 2.0).min(length / 2.0);
        let start_center = start_anchor + u * offset;
        let end_center = end_anchor - u * offset;
        let y_offset = v * ((CONVEYOR_WIDTH - CONVEYOR_BAR_SIZE) / 2.0);

        let mut spawn_segment = |tf, mesh, mat| {
            commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    tf,
                    Visibility::Inherited,
                    ChildOf(e),
                    Selectable::new(e),
                ))
                .id()
        };

        let belt_z = (CONVEYOR_THICKNESS + CONVEYER_BELT_THICKNESS) / 2.0;

        let start = spawn_segment(
            Transform::from_xyz(start_anchor.x, start_anchor.y, 0.0).with_rotation(flat_rotation),
            cylinder_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let start_belt = spawn_segment(
            Transform::from_xyz(start_anchor.x, start_anchor.y, belt_z)
                .with_rotation(flat_rotation),
            belt_cylinder_mesh.clone(),
            assets.conveyor_belt_material.clone(),
        );

        let start_left_stand = spawn_segment(
            Transform::from_xyz(
                start_center.x - y_offset.x,
                start_center.y - y_offset.y,
                stand_z,
            )
            .with_rotation(rot)
            .with_scale(Vec3::new(1.0, 1.0, stand_height)),
            stand_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let start_right_stand = spawn_segment(
            Transform::from_xyz(
                start_center.x + y_offset.x,
                start_center.y + y_offset.y,
                stand_z,
            )
            .with_rotation(rot)
            .with_scale(Vec3::new(1.0, 1.0, stand_height)),
            stand_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let start_bar = spawn_segment(
            Transform::from_xyz(start_center.x, start_center.y, bar_z).with_rotation(rot),
            bar_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let end = spawn_segment(
            Transform::from_xyz(end_anchor.x, end_anchor.y, 0.0).with_rotation(flat_rotation),
            cylinder_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let end_belt = spawn_segment(
            Transform::from_xyz(end_anchor.x, end_anchor.y, belt_z)
                .with_rotation(flat_rotation),
            belt_cylinder_mesh.clone(),
            assets.conveyor_belt_material.clone(),
        );

        let end_left_stand = spawn_segment(
            Transform::from_xyz(
                end_center.x - y_offset.x,
                end_center.y - y_offset.y,
                stand_z,
            )
            .with_rotation(rot)
            .with_scale(Vec3::new(1.0, 1.0, stand_height)),
            stand_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let end_right_stand = spawn_segment(
            Transform::from_xyz(
                end_center.x + y_offset.x,
                end_center.y + y_offset.y,
                stand_z,
            )
            .with_rotation(rot)
            .with_scale(Vec3::new(1.0, 1.0, stand_height)),
            stand_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let end_bar = spawn_segment(
            Transform::from_xyz(end_center.x, end_center.y, bar_z).with_rotation(rot),
            bar_mesh.clone(),
            assets.conveyor_metal_material.clone(),
        );

        let mut mid_tf = line_stroke_transform(&start_anchor, &end_anchor, CONVEYOR_WIDTH);
        mid_tf.translation.z = 0.0;
        let mid = spawn_segment(mid_tf, cube_mesh.clone(), assets.conveyor_metal_material.clone());

        let mut mid_belt_tf = line_stroke_transform(
            &start_anchor,
            &end_anchor,
            CONVEYOR_WIDTH - 2.0 * CONVEYER_MARGIN,
        );
        mid_belt_tf.translation.z = belt_z;
        let mid_belt = spawn_segment(
            mid_belt_tf,
            belt_cube_mesh.clone(),
            assets.conveyor_belt_material.clone(),
        );

        let mut entity_commands = commands.entity(e);
        entity_commands.insert((
            ConveyorSegments {
                start,
                start_belt,
                mid,
                mid_belt,
                end,
                end_belt,
                start_left_stand,
                start_right_stand,
                start_bar,
                end_left_stand,
                end_right_stand,
                end_bar,
            },
            Transform::from_xyz(0.0, 0.0, height),
            Visibility::Inherited,
            Category::Conveyor,
            EdgeLabels::StartEnd,
        ));
        if conveyor_height.is_none() {
            entity_commands.insert(ConveyorHeight(height));
        }
    }
}

fn update_conveyor_visuals(
    entity: Entity,
    edge: &Edge<Entity>,
    segments: &ConveyorSegments,
    conveyor_height: Option<&ConveyorHeight>,
    anchors: &AnchorParams,
    transforms: &mut Query<&mut Transform>,
) {
    let start_anchor = anchors
        .point_in_parent_frame_of(edge.left(), Category::Conveyor, entity)
        .unwrap();
    let end_anchor = anchors
        .point_in_parent_frame_of(edge.right(), Category::Conveyor, entity)
        .unwrap();

    let flat_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let belt_z = (CONVEYOR_THICKNESS + CONVEYER_BELT_THICKNESS) / 2.0;

    if let Ok(mut tf) = transforms.get_mut(segments.start) {
        *tf = Transform::from_xyz(start_anchor.x, start_anchor.y, 0.0).with_rotation(flat_rotation);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.start_belt) {
        *tf = Transform::from_xyz(start_anchor.x, start_anchor.y, belt_z)
            .with_rotation(flat_rotation);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.mid) {
        let mut mid_tf = line_stroke_transform(&start_anchor, &end_anchor, CONVEYOR_WIDTH);
        mid_tf.translation.z = 0.0;
        *tf = mid_tf;
    }
    if let Ok(mut tf) = transforms.get_mut(segments.mid_belt) {
        let mut mid_belt_tf = line_stroke_transform(
            &start_anchor,
            &end_anchor,
            CONVEYOR_WIDTH - 2.0 * CONVEYER_MARGIN,
        );
        mid_belt_tf.translation.z = belt_z;
        *tf = mid_belt_tf;
    }
    if let Ok(mut tf) = transforms.get_mut(segments.end) {
        *tf = Transform::from_xyz(end_anchor.x, end_anchor.y, 0.0).with_rotation(flat_rotation);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.end_belt) {
        *tf = Transform::from_xyz(end_anchor.x, end_anchor.y, belt_z)
            .with_rotation(flat_rotation);
    }

    let dp = end_anchor - start_anchor;
    let length = dp.length();
    let u = if length > 0.001 { dp / length } else { Vec3::X };
    let v = Vec3::new(-u.y, u.x, 0.0);
    let rot = Quat::from_rotation_z(dp.y.atan2(dp.x));

    let height = conveyor_height.map(|h| h.0).unwrap_or(0.5);
    let stand_height = (height - CONVEYOR_THICKNESS / 2.0).max(0.001);
    let stand_z = stand_height / 2.0 - height;
    let bar_z = CONVEYOR_BAR_ELEVATION + CONVEYOR_BAR_SIZE / 2.0 - height;

    let offset = (CONVEYOR_WIDTH / 2.0 + CONVEYOR_BAR_SIZE / 2.0).min(length / 2.0);
    let start_center = start_anchor + u * offset;
    let end_center = end_anchor - u * offset;
    let y_offset = v * ((CONVEYOR_WIDTH - CONVEYOR_BAR_SIZE) / 2.0);

    if let Ok(mut tf) = transforms.get_mut(segments.start_left_stand) {
        tf.translation = Vec3::new(
            start_center.x - y_offset.x,
            start_center.y - y_offset.y,
            stand_z,
        );
        tf.rotation = rot;
        tf.scale = Vec3::new(1.0, 1.0, stand_height);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.start_right_stand) {
        tf.translation = Vec3::new(
            start_center.x + y_offset.x,
            start_center.y + y_offset.y,
            stand_z,
        );
        tf.rotation = rot;
        tf.scale = Vec3::new(1.0, 1.0, stand_height);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.start_bar) {
        tf.translation = Vec3::new(start_center.x, start_center.y, bar_z);
        tf.rotation = rot;
    }
    if let Ok(mut tf) = transforms.get_mut(segments.end_left_stand) {
        tf.translation = Vec3::new(
            end_center.x - y_offset.x,
            end_center.y - y_offset.y,
            stand_z,
        );
        tf.rotation = rot;
        tf.scale = Vec3::new(1.0, 1.0, stand_height);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.end_right_stand) {
        tf.translation = Vec3::new(
            end_center.x + y_offset.x,
            end_center.y + y_offset.y,
            stand_z,
        );
        tf.rotation = rot;
        tf.scale = Vec3::new(1.0, 1.0, stand_height);
    }
    if let Ok(mut tf) = transforms.get_mut(segments.end_bar) {
        tf.translation = Vec3::new(end_center.x, end_center.y, bar_z);
        tf.rotation = rot;
    }
}

pub fn update_conveyor_for_moved_anchor(
    conveyors: Query<
        (
            Entity,
            &Edge<Entity>,
            &ConveyorSegments,
            Option<&ConveyorHeight>,
        ),
        With<ConveyorMarker>,
    >,
    anchors: AnchorParams,
    changed_anchors: Query<
        &Dependents,
        (
            With<Anchor>,
            Or<(Changed<Anchor>, Changed<GlobalTransform>)>,
        ),
    >,
    mut transforms: Query<&mut Transform>,
) {
    for dependents in &changed_anchors {
        for dependent in dependents.iter() {
            if let Ok((e, edge, segments, height)) = conveyors.get(*dependent) {
                update_conveyor_visuals(e, edge, segments, height, &anchors, &mut transforms);
            }
        }
    }
}

pub fn update_changed_conveyor(
    conveyors: Query<
        (
            Entity,
            &Edge<Entity>,
            &ConveyorSegments,
            Option<&ConveyorHeight>,
        ),
        (With<ConveyorMarker>, Changed<Edge<Entity>>),
    >,
    anchors: AnchorParams,
    mut transforms: Query<&mut Transform>,
) {
    for (e, edge, segments, height) in &conveyors {
        update_conveyor_visuals(e, edge, segments, height, &anchors, &mut transforms);
    }
}

pub fn update_conveyor_height(
    mut conveyors: Query<
        (&ConveyorHeight, &ConveyorSegments, &mut Transform),
        (With<ConveyorMarker>, Changed<ConveyorHeight>),
    >,
    mut transforms: Query<&mut Transform, Without<ConveyorMarker>>,
) {
    for (height, segments, mut tf) in &mut conveyors {
        tf.translation.z = height.0;
        let stand_height = (height.0 - CONVEYOR_THICKNESS / 2.0).max(0.001);
        let stand_z = stand_height / 2.0 - height.0;
        let bar_z = CONVEYOR_BAR_ELEVATION + CONVEYOR_BAR_SIZE / 2.0 - height.0;

        for stand_entity in [
            segments.start_left_stand,
            segments.start_right_stand,
            segments.end_left_stand,
            segments.end_right_stand,
        ] {
            if let Ok(mut stand_tf) = transforms.get_mut(stand_entity) {
                stand_tf.translation.z = stand_z;
                stand_tf.scale.z = stand_height;
            }
        }

        for bar_entity in [segments.start_bar, segments.end_bar] {
            if let Ok(mut bar_tf) = transforms.get_mut(bar_entity) {
                bar_tf.translation.z = bar_z;
            }
        }
    }
}
