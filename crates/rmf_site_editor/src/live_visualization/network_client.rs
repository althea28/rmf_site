use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rmf_site_msgs::rmf_prototype_msgs::msg::ParticipantList;
use roslibrust::rosbridge::ClientHandle;

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
        connect_flag: Arc<AtomicBool>,
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
            .push(Arc::new(move |robot_name, client, connect_flag| {
                T::spawn_stream(robot_name, client, tx_clone.clone(), connect_flag);
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

pub fn start_rosbridge_subscriber(
    ws_url: &str,
    registry: StreamRegistry,
    connect_flag: Arc<AtomicBool>,
) {
    let url = ws_url.to_string();
    spawn_network_task(run_rosbridge_loop(url, registry, connect_flag));
}

async fn run_rosbridge_loop(url: String, registry: StreamRegistry, connect_flag: Arc<AtomicBool>) {
    if let Ok(client) = ClientHandle::new(&url).await {
        info!("Connected via roslibrust to {}", url);

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

                    println!("Subscribing to: {}", p.name);

                    for spawner in &registry.spawners {
                        spawner(p.name.clone(), client.clone(), connect_flag.clone());
                    }
                }
            }
        }
    } else {
        println!("Failed to connect to rosbridge WebSocket at {}", url);
    }
}
