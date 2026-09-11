use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rmf_site_msgs::rmf_prototype_msgs::msg::ParticipantList;
use roslibrust::rosbridge::ClientHandle;

use super::planned_paths::{
    handle_plan_stream, handle_progress_stream, LiveEventPlan, LiveEventProgress,
};
use super::robot_odometry::{handle_odometry_stream, LiveEventOdom};

#[derive(Resource)]
pub struct StreamChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

#[derive(Clone)]
pub struct NetworkSenders {
    pub odom: Sender<LiveEventOdom>,
    pub plan: Sender<LiveEventPlan>,
    pub progress: Sender<LiveEventProgress>,
}

pub fn spawn_network_task<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(future);
        } else {
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("Failed to initialize Tokio runtime: {e}");
                        return;
                    }
                };
                rt.block_on(future);
            });
        }
    }

    #[cfg(target_arch = "wasm32")]
    bevy::tasks::IoTaskPool::get().spawn(future).detach();
}

pub fn start_rosbridge_subscriber(
    ws_url: &str,
    senders: NetworkSenders,
    connect_flag: Arc<AtomicBool>,
) {
    let url = ws_url.to_string();
    spawn_network_task(run_rosbridge_loop(url, senders, connect_flag));
}

async fn run_rosbridge_loop(url: String, senders: NetworkSenders, connect_flag: Arc<AtomicBool>) {
    if let Ok(client) = ClientHandle::new(&url).await {
        info!("Successfully connected via roslibrust to {}", url);

        if let Ok(discovery_sub) = client
            .subscribe::<ParticipantList>("/destination/discovery")
            .await
        {
            let mut subscribed_robots = HashSet::new();

            loop {
                let msg = discovery_sub.next().await;

                if !connect_flag.load(Ordering::Relaxed) {
                    println!("Disconnecting from rosbridge discovery stream.");
                    break;
                }

                for p in msg.participants {
                    if subscribed_robots.contains(&p.name) {
                        continue;
                    }
                    subscribed_robots.insert(p.name.clone());

                    println!("Dynamically discovering and subscribing to: {}", p.name);

                    spawn_network_task(handle_odometry_stream(
                        p.name.clone(),
                        client.clone(),
                        senders.odom.clone(),
                        connect_flag.clone(),
                    ));

                    spawn_network_task(handle_plan_stream(
                        p.name.clone(),
                        client.clone(),
                        senders.plan.clone(),
                        connect_flag.clone(),
                    ));

                    spawn_network_task(handle_progress_stream(
                        p.name.clone(),
                        client.clone(),
                        senders.progress.clone(),
                        connect_flag.clone(),
                    ));
                }
            }
        }
    } else {
        println!("Failed to connect to rosbridge WebSocket at {}", url);
    }
}
