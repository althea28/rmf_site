use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rmf_site_msgs::rmf_prototype_msgs::msg::ParticipantList;
use roslibrust::rosbridge::ClientHandle;

const TIMEOUT_SECONDS: u64 = 2;

#[derive(Resource)]
pub struct VisualizationStreamChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

pub trait LiveStreamHandler: Send + Sync + 'static {
    fn spawn_stream(
        robot_name: String,
        client: ClientHandle,
        sender: Sender<Self>,
        connection_active: Arc<AtomicBool>,
    ) where
        Self: Sized;
}

#[derive(Resource, Clone, Default)]
pub struct StreamRegistry {
    pub spawners: Vec<Arc<dyn Fn(String, ClientHandle, Arc<AtomicBool>) + Send + Sync>>,
}

pub struct StreamPlugin<T> {
    _marker: PhantomData<T>,
}

impl<T> Default for StreamPlugin<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: LiveStreamHandler> Plugin for StreamPlugin<T> {
    fn build(&self, app: &mut App) {
        let (tx, rx) = unbounded();
        app.insert_resource(VisualizationStreamChannel::<T> {
            sender: tx.clone(),
            receiver: rx,
        });

        if !app.world().contains_resource::<StreamRegistry>() {
            app.insert_resource(StreamRegistry::default());
        }

        let tx_clone = tx.clone();
        app.world_mut()
            .resource_mut::<StreamRegistry>()
            .spawners
            .push(Arc::new(move |robot_name, client, connection_active| {
                T::spawn_stream(robot_name, client, tx_clone.clone(), connection_active);
            }));
    }
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

// On losing connection, this function kills all zombie tasks
pub async fn wait_until_inactive(connection_active: &Arc<AtomicBool>) {
    while connection_active.load(Ordering::Relaxed) {
        #[cfg(not(target_arch = "wasm32"))]
        tokio::time::sleep(std::time::Duration::from_secs(TIMEOUT_SECONDS)).await;
        #[cfg(target_arch = "wasm32")]
        break;
    }
}

pub fn start_rosbridge_subscriber(
    ws_url: &str,
    registry: StreamRegistry,
    connection_requested: Arc<AtomicBool>,
    connection_active: Arc<AtomicBool>,
) {
    let url = ws_url.to_string();
    spawn_network_task(run_rosbridge_loop(
        url,
        registry,
        connection_requested,
        connection_active,
    ));
}

async fn run_rosbridge_loop(
    url: String,
    registry: StreamRegistry,
    connection_requested: Arc<AtomicBool>,
    connection_active: Arc<AtomicBool>,
) {
    while connection_requested.load(Ordering::Relaxed) {
        // Add timeout for initial connection
        let opts = roslibrust::rosbridge::ClientHandleOptions::new(&url)
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECONDS));

        if let Ok(client) = ClientHandle::new_with_options(opts).await {
            info!("Connected via roslibrust to {}", url);
            connection_active.store(true, Ordering::Relaxed);

            let health_client = client.clone();
            let health_flag = connection_active.clone();
            let health_cancel = connection_requested.clone();

            // Async task to ping server with lightweight subscription every few seconds to check connection health
            spawn_network_task(async move {
                loop {
                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::time::sleep(std::time::Duration::from_secs(TIMEOUT_SECONDS)).await;

                    if !health_cancel.load(Ordering::Relaxed)
                        || !health_flag.load(Ordering::Relaxed)
                    {
                        break;
                    }

                    if health_client
                        .subscribe::<ParticipantList>("/destination/discovery")
                        .await
                        .is_err()
                    {
                        println!("Rosbridge connection lost.");
                        health_flag.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            });

            if let Ok(discovery_sub) = client
                .subscribe::<ParticipantList>("/destination/discovery")
                .await
            {
                let mut subscribed_robots = HashSet::new();

                loop {
                    #[cfg(not(target_arch = "wasm32"))]
                    let msg = tokio::select! {
                        msg = discovery_sub.next() => msg,
                        _ = wait_until_inactive(&connection_active) => break,
                    };
                    #[cfg(target_arch = "wasm32")]
                    let msg = discovery_sub.next().await;

                    if !connection_requested.load(Ordering::Relaxed)
                        || !connection_active.load(Ordering::Relaxed)
                    {
                        println!("Disconnecting from rosbridge discovery stream.");
                        break;
                    }

                    for p in msg.participants {
                        if subscribed_robots.contains(&p.name) {
                            continue;
                        }
                        subscribed_robots.insert(p.name.clone());

                        println!("Subscribing to: {}", p.name);

                        for spawner in &registry.spawners {
                            spawner(p.name.clone(), client.clone(), connection_active.clone());
                        }
                    }
                }
            }

            connection_active.store(false, Ordering::Relaxed);
            println!("Connection to server lost. Attempting to reconnect...");
        } else {
            connection_active.store(false, Ordering::Relaxed);
        }

        if connection_requested.load(Ordering::Relaxed) {
            #[cfg(not(target_arch = "wasm32"))]
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    println!("User disconnected. Shutting down network thread.");
    connection_active.store(false, Ordering::Relaxed);
}
