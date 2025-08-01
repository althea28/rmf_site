/*
 * Copyright (C) 2022 Open Source Robotics Foundation
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
    interaction::{gizmo::Gizmo, IntersectGroundPlaneParams, *},
    site::{Anchor, Category, Delete, Dependents, SiteAssets, Subordinate},
    DebugMode,
};
use bevy::{ecs::hierarchy::ChildOf, prelude::*};
use rmf_site_picking::{Hovered, Selectable, Selected};

/// Use this resource to indicate whether anchors should be constantly highlighted.
/// This is used during anchor selection modes to make it easier for users to know
/// where selectable anchors are.
#[derive(Clone, Copy, Debug, Resource)]
pub struct HighlightAnchors(pub bool);

#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorVisualization {
    pub body: Entity,
    pub drag: Option<Entity>,
}

pub fn add_anchor_visual_cues(
    mut commands: Commands,
    new_anchors: Query<
        (Entity, &ChildOf, Option<&Subordinate>, &Anchor),
        (Added<Anchor>, Without<Preview>),
    >,
    categories: Query<&Category>,
    site_assets: Res<SiteAssets>,
    highlight: Res<HighlightAnchors>,
) {
    for (e, child_of, subordinate, anchor) in &new_anchors {
        let Ok(category) = categories.get(child_of.parent()) else {
            continue;
        };

        let body_mesh = match category {
            Category::Level => site_assets.level_anchor_mesh.clone(),
            Category::Lift => site_assets.lift_anchor_mesh.clone(),
            _ => site_assets.site_anchor_mesh.clone(),
        };

        let body = commands
            .spawn((
                Mesh3d(body_mesh),
                MeshMaterial3d(site_assets.passive_anchor_material.clone()),
                Transform::default(),
                Visibility::default(),
            ))
            .insert(Selectable::new(e))
            .id();
        if subordinate.is_none() {
            commands
                .entity(body)
                .insert(DragPlaneBundle::new(e, Vec3::Z));
        }

        let mut entity_commands = commands.entity(e);
        entity_commands
            .insert(AnchorVisualization { body, drag: None })
            .insert(OutlineVisualization::Anchor { body })
            .add_child(body);

        let cue = if anchor.is_3D() {
            // 3D anchors should always be visible with arrow cue meshes
            VisualCue::outline()
        } else {
            let mut cue = VisualCue::outline().irregular();
            cue.xray.set_always(highlight.0);
            cue
        };

        entity_commands.insert(cue);
    }
}

pub fn on_highlight_anchors_change(
    highlight: Res<HighlightAnchors>,
    mut anchor_visual_cues: Query<&mut VisualCue, With<Anchor>>,
) {
    if !highlight.is_changed() {
        return;
    }

    for mut cue in &mut anchor_visual_cues {
        cue.xray.set_always(highlight.0);
    }
}

pub fn remove_interaction_for_subordinate_anchors(
    mut commands: Commands,
    new_subordinates: Query<&Children, (With<Anchor>, Added<Subordinate>)>,
) {
    for children in &new_subordinates {
        for child in children {
            commands
                .entity(*child)
                .remove::<Gizmo>()
                .remove::<Draggable>()
                .remove::<DragPlane>();
        }
    }
}

pub fn move_anchor(
    mut anchors: Query<&mut Anchor, Without<Subordinate>>,
    mut move_to: EventReader<MoveTo>,
) {
    for move_to in move_to.read() {
        if let Ok(mut anchor) = anchors.get_mut(move_to.entity) {
            anchor.move_to(&move_to.transform);
        }
    }
}

pub fn update_anchor_proximity_xray(
    mut anchors: Query<(&GlobalTransform, &mut VisualCue), With<Anchor>>,
    intersect_ground_params: IntersectGroundPlaneParams,
    cursor_moved: EventReader<CursorMoved>,
) {
    if cursor_moved.is_empty() {
        return;
    }

    let p_c = match intersect_ground_params.ground_plane_intersection() {
        Some(p) => p.translation,
        None => return,
    };

    for (anchor_tf, mut cue) in &mut anchors {
        // TODO(@mxgrey): Make the proximity range configurable
        let proximity = {
            // We make the xray effect a little "sticky" so that there isn't an
            // ugly flicker for anchors that are right at the edge of the
            // proximity range.
            if cue.xray.any() {
                1.0
            } else {
                0.2
            }
        };

        let xray = 'xray: {
            let p_a = anchor_tf.translation();
            if p_a.x < p_c.x - proximity || p_c.x + proximity < p_a.x {
                break 'xray false;
            }

            if p_a.y < p_c.y - proximity || p_c.y + proximity < p_a.y {
                break 'xray false;
            }

            true
        };

        if xray != cue.xray.proximity() {
            cue.xray.set_proximity(xray);
        }
    }
}

pub fn update_unassigned_anchor_cues(
    mut anchors: Query<(&Dependents, &mut VisualCue), (With<Anchor>, Changed<Dependents>)>,
) {
    for (deps, mut cue) in &mut anchors {
        if deps.is_empty() != cue.xray.unassigned() {
            cue.xray.set_unassigned(deps.is_empty())
        }
    }
}

pub fn update_anchor_visual_cues(
    mut commands: Commands,
    mut anchors: Query<
        (
            Entity,
            &Anchor,
            &Hovered,
            &Selected,
            &mut AnchorVisualization,
            &mut VisualCue,
            Option<&Subordinate>,
            Ref<Hovered>,
            Ref<Selected>,
        ),
        Or<(Changed<Hovered>, Changed<Selected>, Changed<Dependents>)>,
    >,
    mut visibility: Query<&mut Visibility>,
    mut materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    deps: Query<&Dependents>,
    mut cursor: ResMut<Cursor>,
    site_assets: Res<SiteAssets>,
    interaction_assets: Res<InteractionAssets>,
    debug_mode: Option<Res<DebugMode>>,
    gizmo_blockers: Res<GizmoBlockers>,
    mut gizmos: Gizmos,
) {
    for (
        a,
        anchor,
        hovered,
        selected,
        mut shapes,
        mut cue,
        subordinate,
        hover_tracker,
        select_tracker,
    ) in &mut anchors
    {
        if debug_mode.as_ref().filter(|d| d.0).is_some() {
            // NOTE(MXG): I have witnessed a scenario where a lane is deleted
            // and then the anchors that supported it are permanently stuck as
            // though they are selected. I have not figured out what can cause
            // that, so I am keeping this printout available to debug that
            // scenario. Press the D key to activate this.
            dbg!((a, hovered, selected));
        }

        if cue.xray.selected() != selected.is_selected {
            cue.xray.set_selected(selected.is_selected)
        }

        if cue.xray.support_selected() != !selected.support_selected.is_empty() {
            cue.xray
                .set_support_selected(!selected.support_selected.is_empty())
        }

        if cue.xray.hovered() != hovered.is_hovered {
            cue.xray.set_hovered(hovered.is_hovered);
        }

        if cue.xray.support_hovered() != !hovered.support_hovering.is_empty() {
            cue.xray
                .set_support_hovered(!hovered.support_hovering.is_empty());
        }

        if hovered.is_hovered && !gizmo_blockers.blocking() {
            cursor.add_blocker(a, &mut visibility);
        } else {
            cursor.remove_blocker(a, &mut visibility);
        }

        if hovered.cue() && selected.cue() {
            set_material(
                shapes.body,
                &site_assets.hover_select_anchor_material,
                &mut materials,
            );
        } else if hovered.cue() {
            // Hovering but not selected
            set_material(
                shapes.body,
                &site_assets.hover_anchor_material,
                &mut materials,
            );
        } else if selected.cue() {
            // Selected but not hovering
            set_material(
                shapes.body,
                &site_assets.select_anchor_material,
                &mut materials,
            );
        } else {
            set_material(
                shapes.body,
                site_assets.decide_passive_anchor_material(a, &deps),
                &mut materials,
            );
        }

        if anchor.is_3D() {
            if select_tracker.is_changed() || hover_tracker.is_changed() {
                if selected.cue() || hovered.cue() {
                    if shapes.drag.is_none() {
                        interaction_assets.add_anchor_gizmos_3D(
                            &mut commands,
                            a,
                            shapes.as_mut(),
                            subordinate.is_none(),
                            &mut gizmos,
                        );
                    }
                } else {
                    if let Some(drag) = shapes.drag {
                        commands.entity(drag).despawn();
                    }
                    shapes.drag = None;
                }
            }
        } else {
            if select_tracker.is_changed() {
                if selected.cue() {
                    if shapes.drag.is_none() && subordinate.is_none() {
                        interaction_assets.add_anchor_gizmos_2D(&mut commands, a, shapes.as_mut());
                    }
                } else {
                    if let Some(drag) = shapes.drag {
                        commands.entity(drag).despawn();
                    }
                    shapes.drag = None;
                }
            }
        }
    }
}

// NOTE(MXG): Currently only anchors ever have support cues, so we filter down
// to entities with AnchorVisualization. We will need to broaden that if any other
// visual cue types ever have a supporting role.
pub fn remove_deleted_supports_from_visual_cues(
    mut hovered: Query<&mut Hovered, With<AnchorVisualization>>,
    mut selected: Query<&mut Selected, With<AnchorVisualization>>,
    mut deleted_elements: EventReader<Delete>,
) {
    for deletion in deleted_elements.read() {
        for mut h in &mut hovered {
            h.support_hovering.remove(&deletion.element);
        }

        for mut s in &mut selected {
            s.support_selected.remove(&deletion.element);
        }
    }
}
