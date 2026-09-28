use bevy::prelude::*;
use rmf_site_format::FloorMarker;
use rmf_site_picking::{CursorFrame, Hovering, Selected};
use std::sync::atomic::Ordering;

use super::live_state::LiveStreamState;
use super::odometry::LiveRobotMarker;

pub fn send_robot_paths(
    state: Res<LiveStreamState>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Query<&Transform, With<CursorFrame>>,
    selected_robots: Query<(&LiveRobotMarker, &Selected)>,
    hovering: Res<Hovering>,
    floors: Query<(), With<FloorMarker>>,
) {
    if !state.connection_active.load(Ordering::Relaxed)
        || !buttons.just_pressed(MouseButton::Right)
        || !keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
    {
        return;
    }

    let Ok(cursor_transform) = cursor.single() else {
        return;
    };
    let point = cursor_transform.translation;

    let pointing_at_floor = hovering.0.is_some_and(|e| floors.contains(e));

    if !pointing_at_floor {
        println!("Invalid destination");
        return;
    }

    for (robot, selected) in selected_robots.iter() {
        if !selected.is_selected {
            continue;
        }

        let mut request_url = match url::Url::parse(&state.site_url) {
            Ok(u) => u,
            Err(e) => {
                println!("Invalid HTTP URL: {}", e);
                continue;
            }
        };

        request_url.set_path("/destination");
        request_url.set_query(Some(&format!(
            "name={}&x={:.2}&y={:.2}",
            robot.name, point.x, point.y
        )));

        let request = ehttp::Request::get(request_url.as_str());
        ehttp::fetch(request, move |result| {
            if let Ok(response) = result {
                if response.status == 200 {
                    println!("Successfully sent command");
                } else {
                    println!("Command failed, backend returned: {}", response.status);
                }
            }
        });
    }
}
