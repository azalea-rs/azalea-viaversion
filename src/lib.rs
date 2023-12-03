mod download;
mod get_mc_dir;

use std::{
    io::Cursor,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use azalea::{
    app::{App, Plugin, PreUpdate, Startup, Update},
    auth::sessionserver::ClientSessionServerError,
    buf::McBufReadable,
    ecs::prelude::*,
    packet_handling::login::IgnoreQueryIds,
    packet_handling::login::{self, LoginPacketEvent, SendLoginPacketEvent},
    prelude::*,
    protocol::{
        packets::login::{
            serverbound_custom_query_answer_packet::ServerboundCustomQueryAnswerPacket,
            ClientboundLoginPacket,
        },
        ServerAddress,
    },
    swarm::Swarm,
};
use download::download_file;
use tokio::{
    io::AsyncBufReadExt,
    sync::{mpsc, oneshot},
};
use tracing::{error, info};

const VIAPROXY_DOWNLOAD_URL: &str =
    "https://github.com/ViaVersion/ViaProxy/releases/download/v3.0.22/ViaProxy-3.0.22.jar";

#[derive(Clone, Debug, Resource)]
pub struct ViaVersionPlugin {
    bind_port: u16,
    version: String,
    auth_request_tx: mpsc::UnboundedSender<AuthRequest>,
}

impl ViaVersionPlugin {
    pub async fn start(version: &str) -> Self {
        let minecraft_dir = get_mc_dir::minecraft_dir().unwrap_or_else(|| {
            panic!(
                "No {} environment variable found",
                get_mc_dir::home_env_var()
            )
        });

        let download_directory = minecraft_dir.join("azalea-viaversion");
        let download_filename = VIAPROXY_DOWNLOAD_URL.split('/').last().unwrap();
        let download_path = download_directory.join(download_filename);

        if !download_directory.exists() {
            std::fs::create_dir_all(&download_directory).unwrap();
        }

        if !download_path.exists() {
            let client = reqwest::Client::new();
            download_file(
                &client,
                VIAPROXY_DOWNLOAD_URL,
                &download_path.to_string_lossy(),
            )
            .await
            .unwrap();
        }

        // pick a port to run viaproxy on

        let bind_port = portpicker::pick_unused_port().expect("No ports available");

        // java -jar ViaProxy-whateverversion.jar --bind_port 25568 --target_ip 127.0.0.1 --target_port 25565 --version b1.7-b1.7.3

        let mut child = tokio::process::Command::new("java")
            .current_dir(&download_directory)
            .arg("-jar")
            .arg(download_path)
            .arg("--bind_port")
            .arg(bind_port.to_string())
            .arg("--internal_srv_mode")
            // these don't matter because we have srv mode
            .arg("--target_ip")
            .arg("127.0.0.1")
            .arg("--target_port")
            .arg("0")
            .arg("--version")
            .arg(version)
            .arg("--openauthmod_auth")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to start ViaProxy");

        let Some(stdout) = child.stdout.as_mut() else {
            panic!("ViaProxy failed to start");
        };
        let mut stdout = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            stdout.read_line(&mut line).await.unwrap();
            if line.contains("Binding proxy server to ") {
                info!("ViaProxy is ready!");
                break;
            }
        }

        // wait 100ms just to be safe
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let (auth_request_tx, auth_request_rx) = mpsc::unbounded_channel();

        tokio::spawn(handle_auth_requests_loop(auth_request_rx));

        Self {
            bind_port,
            version: version.to_string(),
            auth_request_tx,
        }
    }
}

pub struct AuthRequest {
    server_id_hash: String,
    account: Account,
    tx: oneshot::Sender<Result<(), ClientSessionServerError>>,
}

async fn handle_auth_requests_loop(mut rx: mpsc::UnboundedReceiver<AuthRequest>) {
    while let Some(AuthRequest {
        server_id_hash,
        account,
        tx,
    }) = rx.recv().await
    {
        let client = reqwest::Client::new();

        let uuid = account.uuid_or_offline();
        let Some(access_token) = account.access_token.clone() else {
            continue;
        };

        let mut attempts = 0;
        let result = loop {
            if let Err(e) = {
                let access_token = access_token.lock().clone();
                azalea::auth::sessionserver::join_with_server_id_hash(
                    &client,
                    &access_token,
                    &uuid,
                    &server_id_hash,
                )
                .await
            } {
                if attempts >= 2 {
                    // if this is the second attempt and we failed
                    // both times, give up
                    break Err(e.into());
                }
                if matches!(
                    e,
                    ClientSessionServerError::InvalidSession
                        | ClientSessionServerError::ForbiddenOperation
                ) {
                    // uh oh, we got an invalid session and have
                    // to reauthenticate now
                    if let Err(e) = account.refresh().await {
                        error!("Failed to refresh account: {e:?}");
                        continue;
                    }
                } else {
                    break Err(e.into());
                }
                attempts += 1;
            } else {
                break Ok(());
            }
        };

        let _ = tx.send(result);
    }
}

impl Plugin for ViaVersionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, change_connection_address);
        app.add_systems(
            PreUpdate,
            handle_openauthmod.before(login::process_packet_events),
        );
        app.add_systems(Update, handle_join_task);

        app.insert_resource(self.clone());
    }
}

fn change_connection_address(swarm: Res<Swarm>, plugin: Res<ViaVersionPlugin>) {
    let target_address = swarm.address.read().clone();

    *swarm.address.write() = ServerAddress {
        // ip\7port\7version\7mppass
        host: format!(
            "{ip}\x07{port}\x07{version}\x07{mppass}",
            ip = target_address.host,
            port = target_address.port,
            version = plugin.version,
            mppass = ""
        ),
        port: 25565,
    };
    *swarm.resolved_address.write() =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, plugin.bind_port));
}

fn handle_openauthmod(
    mut commands: Commands,
    mut events: EventReader<LoginPacketEvent>,
    mut query: Query<(&Account, &mut IgnoreQueryIds)>,

    plugin: Res<ViaVersionPlugin>,
) {
    for event in events.read() {
        let ClientboundLoginPacket::CustomQuery(p) = &*event.packet else {
            continue;
        };
        let mut data = Cursor::new(&*p.data);

        match p.identifier.to_string().as_str() {
            "oam:join" => {
                let Ok(server_id_hash) = String::read_from(&mut data) else {
                    error!("Failed to read server id hash from oam:join packet");
                    continue;
                };

                let (account, mut ignore_query_ids) = query.get_mut(event.entity).unwrap();

                ignore_query_ids.insert(p.transaction_id);

                if account.access_token.is_none() {
                    error!("Server is online-mode, but our account is offline-mode");
                    continue;
                };

                let (tx, rx) = oneshot::channel();

                let request = AuthRequest {
                    server_id_hash,
                    account: account.clone(),
                    tx,
                };

                plugin.auth_request_tx.send(request).unwrap();

                commands.spawn(JoinServerTask {
                    entity: event.entity,
                    rx,
                    transaction_id: p.transaction_id,
                });
            }
            "oam:sign_nonce" => {}
            "oam:data" => {}
            _ => {}
        }
    }
}

#[derive(Component)]
struct JoinServerTask {
    entity: Entity,
    rx: oneshot::Receiver<Result<(), ClientSessionServerError>>,
    transaction_id: u32,
}

fn handle_join_task(
    mut commands: Commands,
    mut join_server_tasks: Query<(Entity, &mut JoinServerTask)>,
    mut send_packets: EventWriter<SendLoginPacketEvent>,
) {
    for (entity, mut task) in &mut join_server_tasks {
        if let Ok(result) = task.rx.try_recv() {
            // Task is complete, so remove task component from entity
            commands.entity(entity).remove::<JoinServerTask>();

            if let Err(e) = &result {
                error!("Sessionserver error: {e:?}");
            }

            send_packets.send(SendLoginPacketEvent {
                entity: task.entity,
                packet: ServerboundCustomQueryAnswerPacket {
                    transaction_id: task.transaction_id,
                    data: Some(vec![if result.is_ok() { 1 } else { 0 }].into()),
                }
                .get(),
            })
        }
    }
}
