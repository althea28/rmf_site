use crate::interaction::billboard::Billboard;
use crate::site::SiteAssets;
use bevy::prelude::*;
use bevy_rich_text3d::*;
use rmf_site_format::{Angle, NameInSite, Pose, Rotation};
use rmf_site_msgs::nav_msgs::msg::Odometry;
use rmf_site_picking::VisualCue;
use roslibrust::rosbridge::ClientHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use super::live_state::LiveStreamState;
use super::network_client::{
    spawn_network_task, wait_until_inactive, LiveStreamHandler, VisualizationStreamChannel,
};

const STATUS_BILLBOARD_Z_HEIGHT: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct LiveEventOdom {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

impl LiveStreamHandler for LiveEventOdom {
    fn spawn_stream(
        robot_name: String,
        client: ClientHandle,
        sender: UnboundedSender<Self>,
        connect_flag: Arc<AtomicBool>,
        connection_active: Arc<AtomicBool>,
    ) {
        let topic_name = format!("/{}/odom", robot_name);

        let task = async move {
            if let Ok(odom_sub) = client.subscribe::<Odometry>(&topic_name).await {
                loop {
                    let odom = tokio::select! {
                        msg = odom_sub.next() => msg,
                        _ = wait_until_inactive(&connection_active) => break,
                    };

                    if !connect_flag.load(Ordering::Relaxed)
                        || !connection_active.load(Ordering::Relaxed)
                    {
                        break;
                    }

                    let pos = &odom.pose.pose.position;
                    let q = &odom.pose.pose.orientation;

                    let siny_cosp: f64 = 2.0 * (q.w * q.z + q.x * q.y);
                    let cosy_cosp: f64 = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
                    let yaw = siny_cosp.atan2(cosy_cosp) as f32;

                    if let Err(e) = sender.send(LiveEventOdom {
                        name: robot_name.clone(),
                        x: pos.x as f32,
                        y: pos.y as f32,
                        z: pos.z as f32,
                        yaw,
                    }) {
                        error!("Failed to send Odometry event across channel: {}", e);
                        break;
                    }
                }
            }
        };
        spawn_network_task(task);
    }
}

#[derive(Component)]
pub struct LiveRobotMarker {
    pub name: String,
}

#[derive(Component)]
pub struct LiveRobotBillboard;

#[derive(Default, Resource)]
pub struct LiveRobotsMap(pub HashMap<String, Entity>);

pub fn update_live_robots(
    state: Res<LiveStreamState>,
    mut channel: ResMut<VisualizationStreamChannel<LiveEventOdom>>,
    mut commands: Commands,
    mut robot_map: ResMut<LiveRobotsMap>,
    mut live_query: Query<(Entity, &LiveRobotMarker, &mut Pose)>,
    mut untracked_query: Query<(Entity, &NameInSite, &mut Pose), Without<LiveRobotMarker>>,
    site_assets: Res<SiteAssets>,
    billboard_query: Query<Entity, With<LiveRobotBillboard>>,
) {
    if !state.connection_active.load(Ordering::Relaxed) {
        if !robot_map.0.is_empty() {
            robot_map.0.clear();
            for (entity, _, _) in live_query.iter_mut() {
                commands.entity(entity).remove::<LiveRobotMarker>();
            }
            for billboard_entity in billboard_query.iter() {
                commands.entity(billboard_entity).despawn();
            }
        }
        return;
    }

    while let Ok(event) = channel.receiver.try_recv() {
        let mut found = false;

        // Find existing robot
        for (_, robot, mut pose) in live_query.iter_mut() {
            if robot.name == event.name {
                pose.trans = [event.x, event.y, event.z];
                pose.rot = Rotation::Yaw(Angle::Rad(event.yaw).match_variant(pose.rot.yaw()));
                found = true;
                break;
            }
        }

        if found {
            continue;
        }

        // If already hooked in this frame (before deferred LiveRobotMarker command applies),
        // update its pose directly without re-hooking or spawning duplicate billboards.
        if let Some(&entity) = robot_map.0.get(&event.name) {
            if let Ok((_, _, mut pose)) = untracked_query.get_mut(entity) {
                pose.trans = [event.x, event.y, event.z];
                pose.rot = Rotation::Yaw(Angle::Rad(event.yaw).match_variant(pose.rot.yaw()));
                continue;
            }
        }

        // New untracked robot: find matching NameInSite
        for (entity, name_in_site, mut pose) in untracked_query.iter_mut() {
            if name_in_site.0 == event.name {
                pose.trans = [event.x, event.y, event.z];
                pose.rot = Rotation::Yaw(Angle::Rad(event.yaw).match_variant(pose.rot.yaw()));

                commands.entity(entity).insert(LiveRobotMarker {
                    name: event.name.clone(),
                });

                robot_map.0.insert(event.name.clone(), entity);
                add_status_billboard_to_robot(&mut commands, &site_assets, entity, &event.name);

                println!("Hooked onto existing site robot: {}", event.name);
                found = true;
                break;
            }
        }

        if !found {
            println!("No matching NameInSite found for: {}", event.name);
        }
    }
}

pub fn add_status_billboard_to_robot(
    commands: &mut Commands,
    site_assets: &Res<SiteAssets>,
    robot_entity: Entity,
    robot_name: &str,
) {
    let text_billboard_entity = commands
        .spawn((
            Text3d::new(robot_name),
            Text3dStyling {
                font: "Fira Sans".into(),
                size: 120.,
                weight: Weight(550),
                color: Srgba::BLACK,
                world_scale: Some(Vec2::splat(0.08)),
                layer_offset: 0.001,
                align: TextAlign::Center,
                ..Default::default()
            },
            Mesh3d::default(),
            MeshMaterial3d(site_assets.text3d_material.clone()),
            Transform::default(),
            Billboard {
                offset: Vec3::new(0.0, 0.0, STATUS_BILLBOARD_Z_HEIGHT),
                hover_enabled: false,
            },
            VisualCue::no_outline(),
            LiveRobotBillboard,
        ))
        .id();

    let background_entity = commands
        .spawn((
            Mesh3d(site_assets.status_billboard_mesh.clone()),
            MeshMaterial3d(site_assets.status_billboard_material.clone()),
            Transform::from_xyz(0.0, 0.0, -0.01),
            VisualCue::no_outline(),
        ))
        .id();

    commands
        .entity(text_billboard_entity)
        .add_child(background_entity);

    commands
        .entity(robot_entity)
        .add_child(text_billboard_entity);
}
