use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use bevy::tasks::IoTaskPool;
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rmf_site_msgs::nav_msgs::msg::Odometry;
use rmf_site_msgs::rmf_prototype_msgs::msg::{ParticipantList, Plan};
use roslibrust::rosbridge::ClientHandle;

use super::planned_paths::LiveEventPlan;
use super::robot_odometry::LiveEventOdom;

#[derive(Resource)]
pub struct StreamChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

#[derive(Clone)]
pub struct NetworkSenders {
    pub odom: Sender<LiveEventOdom>,
    pub plan: Sender<LiveEventPlan>,
}

pub fn start_rosbridge_subscriber(
    ws_url: &str,
    senders: NetworkSenders,
    connect_flag: Arc<AtomicBool>,
) {
    let url = ws_url.to_string();

    // If compiling for a desktop, spawn a native OS background thread and initialize a fully-featured tokio multi-threaded async runtime
    // Gives native users maximum performance, as desktop computers handle networking best using OS-level threads and async runtimes like tokio.
    #[cfg(not(target_arch = "wasm32"))]
    {
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
            rt.block_on(run_rosbridge_loop(url, senders, connect_flag));
        });
    }

    // If compiling for wasm32, use Bevy's IoTaskPool
    // This is required as web browser security sandboxes completely block native OS threads and tokio multi-threading.
    #[cfg(target_arch = "wasm32")]
    {
        IoTaskPool::get()
            .spawn(run_rosbridge_loop(url, senders, connect_flag))
            .detach();
    }
}

async fn run_rosbridge_loop(url: String, senders: NetworkSenders, connect_flag: Arc<AtomicBool>) {
    if let Ok(client) = ClientHandle::new(&url).await {
        println!("Successfully connected to {}", url);

        // Subscribes to fleet discovery topic to find new robots
        if let Ok(discovery_sub) = client
            .subscribe::<ParticipantList>("/destination/discovery")
            .await
        {
            // Track which robots have been seen and subscribed to
            let mut subscribed_robots = HashSet::new();

            loop {
                let msg = discovery_sub.next().await;

                // Checks if user has disconnected
                if !connect_flag.load(Ordering::Relaxed) {
                    println!("Disconnecting from rosbridge.");
                    break;
                }

                for p in msg.participants {
                    if subscribed_robots.contains(&p.name) {
                        continue;
                    }
                    subscribed_robots.insert(p.name.clone());

                    // Build the async sub-task to handle this new robot's data stream
                    let robot_name = p.name.clone();
                    let topic_name = format!("/{}/odom", robot_name);
                    let odom_sender = senders.odom.clone();
                    let odom_client = client.clone();
                    let connect_clone = connect_flag.clone();

                    println!("Subscribed to: {}", topic_name);

                    let odom_task = async move {
                        if let Ok(odom_sub) = odom_client.subscribe::<Odometry>(&topic_name).await {
                            loop {
                                let odom = odom_sub.next().await;

                                if !connect_clone.load(Ordering::Relaxed) {
                                    break;
                                }

                                // Package and send odometry data to main thread
                                let pos = &odom.pose.pose.position;
                                let q = &odom.pose.pose.orientation;

                                let siny_cosp: f64 = 2.0 * (q.w * q.z + q.x * q.y);
                                let cosy_cosp: f64 = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
                                let yaw = siny_cosp.atan2(cosy_cosp) as f32;

                                let _ = odom_sender.send(LiveEventOdom {
                                    name: robot_name.clone(),
                                    x: pos.x as f32,
                                    y: pos.y as f32,
                                    z: pos.z as f32,
                                    yaw,
                                });
                            }
                        }
                    };

                    // Spawn the async task depending on the target platform
                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::spawn(odom_task);

                    #[cfg(target_arch = "wasm32")]
                    IoTaskPool::get().spawn(odom_task).detach();

                    let plan_client = client.clone();
                    let plan_sender = senders.plan.clone();
                    let plan_cancel = connect_flag.clone();
                    let robot_name_plan = p.name.clone();
                    let plan_topic = format!("/{}/plan", p.name);

                    println!("Subscribed to plan: {}", plan_topic);

                    let plan_task = async move {
                        if let Ok(plan_sub) = plan_client.subscribe::<Plan>(&plan_topic).await {
                            loop {
                                let plan_msg = plan_sub.next().await;

                                if !plan_cancel.load(Ordering::Relaxed) {
                                    break;
                                }

                                let waypoints: Vec<Vec3> = plan_msg
                                    .waypoints
                                    .iter()
                                    .map(|wp| Vec3::new(wp.position[0], wp.position[1], 0.05))
                                    .collect();

                                let _ = plan_sender.send(LiveEventPlan {
                                    name: robot_name_plan.clone(),
                                    waypoints,
                                });
                            }
                        }
                    };

                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::spawn(plan_task);

                    #[cfg(target_arch = "wasm32")]
                    IoTaskPool::get().spawn(plan_task).detach();
                }
            }
        }
    } else {
        println!("Failed to connect to rosbridge WebSocket at {}", url);
    }
}
