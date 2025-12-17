use std::{io::Cursor, net::SocketAddr, path::Path, process::Stdio};

use anyhow::{Context, Result};
use azalea::protocol::address::{ResolvedAddr, ServerAddr};
use azalea::{
    app::{App, Plugin, Startup, prelude::*},
    auth::sessionserver::{
        ClientSessionServerError::{ForbiddenOperation, InvalidSession},
        join_with_server_id_hash,
    },
    bevy_tasks::{IoTaskPool, Task, futures_lite::future},
    buf::AzaleaRead,
    ecs::prelude::*,
    join::StartJoinServerEvent,
    packet::login::{ReceiveCustomQueryEvent, SendLoginPacketEvent},
    prelude::*,
    protocol::{connect::Proxy, packets::login::ServerboundCustomQueryAnswer},
    swarm::Swarm,
};
use futures_util::StreamExt;
use kdam::{BarExt, tqdm};
use lazy_regex::{regex_captures, regex_replace_all};
use reqwest::IntoUrl;
use semver::Version;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    process::Command,
};
use tracing::{error, trace, warn};

const JAVA_DOWNLOAD_URL: &str = "https://adoptium.net/installation";
const VIA_OAUTH_VERSION: Version = Version::new(1, 0, 2);
// https://github.com/ViaVersion/ViaProxy/releases
const VIA_PROXY_VERSION: Version = Version::new(3, 4, 7);

#[derive(Clone, Resource)]
pub struct ViaVersionPlugin {
    bind_addr: SocketAddr,
    mc_version: String,
    proxy: Option<Proxy>,
}

impl Plugin for ViaVersionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.clone())
            .add_systems(Startup, Self::handle_change_address)
            .add_systems(
                Update,
                (
                    Self::handle_oauth.before(azalea::login::reply_to_custom_queries),
                    Self::poll_all_oam_join_tasks,
                    Self::warn_about_proxy
                        .after(azalea::auto_reconnect::rejoin_after_delay)
                        .before(azalea::join::handle_start_join_server_event),
                ),
            );
    }
}

impl ViaVersionPlugin {
    /// Download and start a ViaProxy instance.
    ///
    /// # Panics
    ///
    /// Will panic if Java fails to parse, files fail to download, or ViaProxy
    /// fails to start.
    pub async fn start(mc_version: impl ToString) -> Self {
        let bind_addr = try_find_free_addr().await.expect("Failed to bind");
        let mc_version = mc_version.to_string();

        let plugin = Self {
            bind_addr,
            mc_version,
            proxy: None,
        };
        plugin.start_with_self().await
    }

    /// Same as [`Self::start`], but allows you to pass a Socks5 proxy.
    ///
    /// This is necessary if you want to use Azalea with a proxy and ViaVersion
    /// at the same time. This is incompatible with `JoinOpts::proxy`.
    ///
    /// ```no_run
    /// # use azalea::{prelude::*, protocol::connect::Proxy};
    /// # use azalea_viaversion::ViaVersionPlugin;
    /// #[tokio::main]
    /// async fn main() {
    ///     let account = Account::offline("bot");
    ///
    ///     ClientBuilder::new()
    ///         .set_handler(handle)
    ///         .add_plugins(
    ///             ViaVersionPlugin::start_with_proxy(
    ///                 "1.21.5",
    ///                 Proxy::new("10.124.1.186:1080".parse().unwrap(), None),
    ///             )
    ///             .await,
    ///         )
    ///         .start(account, "6.tcp.ngrok.io:14910")
    ///         .await;
    /// }
    /// # async fn handle(mut bot: Client, event: Event, state: azalea::NoState) { }
    /// ```
    pub async fn start_with_proxy(mc_version: impl ToString, proxy: Proxy) -> Self {
        let bind_addr = try_find_free_addr().await.expect("Failed to bind");
        let mc_version = mc_version.to_string();

        let plugin = Self {
            bind_addr,
            mc_version,
            proxy: Some(proxy),
        };
        plugin.start_with_self().await
    }

    async fn start_with_self(self) -> Self {
        let Some(java_version) = try_find_java_version().await.expect("Failed to parse") else {
            panic!(
                "Java installation not found! Please download Java from {JAVA_DOWNLOAD_URL} or use your system's package manager."
            );
        };

        let mc_path = minecraft_folder_path::minecraft_dir().expect("Unsupported Platform");

        #[rustfmt::skip]
        let via_proxy_ext = if java_version.major < 17 { "+java8.jar" } else { ".jar" };
        let via_proxy_name = format!("ViaProxy-{VIA_PROXY_VERSION}{via_proxy_ext}");
        let via_proxy_path = mc_path.join("azalea-viaversion");
        let via_proxy_url = format!(
            "https://github.com/ViaVersion/ViaProxy/releases/download/v{VIA_PROXY_VERSION}/{via_proxy_name}"
        );
        try_download_file(via_proxy_url, &via_proxy_path, &via_proxy_name)
            .await
            .expect("Failed to download ViaProxy");

        let via_oauth_name = format!("ViaProxyOpenAuthMod-{VIA_OAUTH_VERSION}.jar");
        let via_oauth_path = via_proxy_path.join("plugins");
        let via_oauth_url = format!(
            "https://github.com/ViaVersionAddons/ViaProxyOpenAuthMod/releases/download/v{VIA_OAUTH_VERSION}/{via_oauth_name}"
        );
        try_download_file(via_oauth_url, &via_oauth_path, &via_oauth_name)
            .await
            .expect("Failed to download ViaProxyOpenAuthMod");

        let mut command = Command::new("java");
        command
            /* Java Args */
            .args(["-jar", &via_proxy_name])
            /* ViaProxy Args */
            .arg("cli")
            .args(["--auth-method", "OPENAUTHMOD"])
            .args(["--bind-address", &self.bind_addr.to_string()])
            .args(["--target-address", "127.0.0.1:0"])
            .args(["--target-version", &self.mc_version])
            .args(["--wildcard-domain-handling", "INTERNAL"]);

        if let Some(proxy) = &self.proxy {
            trace!("Starting ViaProxy with proxy: {proxy}");
            command.args(["--backend-proxy-url", &proxy.to_string()]);
        }

        let mut child = command
            .current_dir(via_proxy_path)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        let (tx, mut rx) = tokio::sync::watch::channel(());
        tokio::spawn(async move {
            let mut stdout = child.stdout.as_mut().expect("Failed to get stdout");
            let mut reader = BufReader::new(&mut stdout);
            let mut line = String::new();

            loop {
                line.clear();
                reader.read_line(&mut line).await.expect("Failed to read");

                let line = line.trim();
                // strip ansi escape codes
                let line = regex_replace_all!(r"(\x1b\[[0-9;]*m)", line, |_, _| "");

                if line.contains("/WARN]") {
                    warn!("{line}");
                } else {
                    trace!("{line}");
                }
                if line.contains("Finished mapping loading") {
                    let _ = tx.send(());
                }
            }
        });

        /* Wait until ViaProxy is ready */
        let _ = rx.changed().await;

        self
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn handle_change_address(plugin: Res<Self>, swarm: Res<Swarm>) {
        let ResolvedAddr { server, .. } = swarm.address.read().clone();
        let ServerAddr { host, port } = server;

        // sadly, the first part of the resolved address is unused as viaproxy will
        // resolve it on its own more info: https://github.com/ViaVersion/ViaProxy/issues/338
        let data_after_null_byte = host.split_once('\x07').map(|(_, data)| data);

        let mut connection_host = format!(
            "localhost\x07{host}:{port}\x07{version}",
            version = plugin.mc_version
        );
        if let Some(data) = data_after_null_byte {
            connection_host.push('\0');
            connection_host.push_str(data);
        }

        *swarm.address.write() = ResolvedAddr {
            server: ServerAddr {
                port,
                host: connection_host,
            },
            socket: plugin.bind_addr,
        };

        /* Must wait to be written until after reading above */
    }

    pub fn handle_oauth(
        mut commands: Commands,
        mut events: MessageMutator<ReceiveCustomQueryEvent>,
        mut query: Query<&Account>,
    ) {
        for event in events.read() {
            if event.packet.identifier.to_string().as_str() != "oam:join" {
                continue;
            }

            let mut buf = Cursor::new(&*event.packet.data);
            let Ok(hash) = String::azalea_read(&mut buf) else {
                error!("Failed to read server id hash from oam:join packet");
                continue;
            };

            let Ok(account) = query.get_mut(event.entity) else {
                continue;
            };

            // this makes it so azalea doesn't reply to the query so we can handle it
            // ourselves
            event.disabled = true;

            let Some(access_token) = &account.access_token else {
                warn!("The server tried to make us authenticate, but our account is offline-mode");
                commands.trigger(SendLoginPacketEvent::new(
                    event.entity,
                    build_custom_query_answer(event.packet.transaction_id, true),
                ));
                continue;
            };

            let client = reqwest::Client::new();
            let token = access_token.lock().clone();
            let uuid = account.uuid_or_offline();
            let account = account.clone();
            let transaction_id = event.packet.transaction_id;

            let task_pool = IoTaskPool::get();
            let task = task_pool.spawn(async move {
                // joining servers uses tokio, but we poll the task with `futures`
                let res = async_compat::Compat::new(async {
                    Some(
                        match join_with_server_id_hash(&client, &token, &uuid, &hash).await {
                            Ok(()) => Ok(()), /* Successfully Authenticated */
                            Err(InvalidSession | ForbiddenOperation) => {
                                if let Err(error) = account.refresh().await {
                                    error!("Failed to refresh account: {error}");
                                    return None;
                                }

                                /* Retry after refreshing */
                                join_with_server_id_hash(&client, &token, &uuid, &hash).await
                            }
                            Err(error) => Err(error),
                        },
                    )
                })
                .await?;

                Some(build_custom_query_answer(transaction_id, res.is_ok()))
            });
            commands
                .entity(event.entity)
                .insert(OpenAuthModJoinTask(task));
        }
    }

    fn poll_all_oam_join_tasks(
        mut commands: Commands,
        mut tasks: Query<(Entity, &mut OpenAuthModJoinTask)>,
    ) {
        for (entity, mut task) in tasks.iter_mut() {
            let Some(res) = future::block_on(future::poll_once(&mut task.0)) else {
                continue;
            };

            commands.entity(entity).remove::<OpenAuthModJoinTask>();

            let Some(packet) = res else {
                error!("Failed to do Mojang auth for openauthmod, not sending response");
                continue;
            };

            commands.trigger(SendLoginPacketEvent::new(entity, packet));
            // inserting this is necessary to make azalea send the chat signing certs
            commands
                .entity(entity)
                .insert(azalea::login::IsAuthenticated);
        }
    }

    fn warn_about_proxy(mut events: MessageMutator<StartJoinServerEvent>) {
        for event in events.read() {
            if event.connect_opts.server_proxy.is_some() {
                warn!(
                    "You are using JoinOpts::proxy and ViaVersionPlugin at the same time, which is not a supported configuration. \
                    Please set your proxy with `ViaVersionPlugin::start_with_proxy` instead."
                );
            }
        }
    }
}

#[derive(Component)]
pub struct OpenAuthModJoinTask(Task<Option<ServerboundCustomQueryAnswer>>);

fn build_custom_query_answer(transaction_id: u32, success: bool) -> ServerboundCustomQueryAnswer {
    ServerboundCustomQueryAnswer {
        transaction_id,
        data: Some(vec![u8::from(success)].into()),
    }
}

/// Try to find the system's Java version.
///
/// This uses `-version` and `stderr`, because it's backwards compatible.
///
/// # Errors
/// Will return `Err` if `Version::parse` fails.
///
/// # Options
/// Will return `None` if java is not found.
pub async fn try_find_java_version() -> Result<Option<Version>> {
    Ok(match Command::new("java").arg("-version").output().await {
        Err(_) => None, /* Java not found */
        Ok(output) => {
            let stderr = String::from_utf8(output.stderr).context("UTF-8")?;
            Some(parse_java_version(&stderr)?)
        }
    })
}

fn parse_java_version(stderr: &str) -> Result<Version> {
    // whole, first group, second group
    let (_, major, mut minor_patch) =
        regex_captures!(r"(\d+)(\.\d+\.\d+)?", stderr).context("Regex")?;
    if minor_patch.is_empty() {
        minor_patch = ".0.0";
    }

    let text = format!("{major}{minor_patch}");
    Ok(Version::parse(&text)?)
}

/// Try to find a free port and return the socket address
///
/// This uses `TcpListener` to ask the system for a free port.
///
/// # Errors
/// Will return `Err` if `TcpListener::bind` or `TcpListener::local_addr` fails.
pub async fn try_find_free_addr() -> Result<SocketAddr> {
    Ok(TcpListener::bind("127.0.0.1:0").await?.local_addr()?)
}

/// Try to download and save a file if it doesn't exist.
///
/// # Errors
/// Will return `Err` if the file fails to download or save.
pub async fn try_download_file<U, P>(url: U, dir: P, file: &str) -> Result<()>
where
    U: IntoUrl + Send + Sync,
    P: AsRef<Path> + Send + Sync,
{
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.as_ref().join(file);
    if path.exists() {
        return Ok(());
    }

    let response = reqwest::get(url).await?;
    let mut pb = tqdm!(
        total = usize::try_from(response.content_length().unwrap_or(0))?,
        unit_scale = true,
        unit_divisor = 1024,
        unit = "B",
        force_refresh = true
    );

    pb.write(format!("Downloading {file}"))?;

    let mut file = File::create(path).await?;
    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk).await?;
        pb.update(chunk.len())?;
    }

    pb.refresh()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_openjdk_ea() {
        let stderr = "openjdk version \"24-ea\" 2025-03-18
OpenJDK Runtime Environment (build 24-ea+29-3578)
OpenJDK 64-Bit Server VM (build 24-ea+29-3578, mixed mode, sharing)"
            .to_string();
        let version = parse_java_version(&stderr).unwrap();
        assert_eq!(version, Version::new(24, 0, 0));
    }

    #[test]
    fn test_parse_openjdk_8() {
        let stderr = "openjdk version \"1.8.0_432\"
OpenJDK Runtime Environment (build 1.8.0_432-b05)
OpenJDK 64-Bit Server VM (build 25.432-b05, mixed mode)"
            .to_string();
        let version = parse_java_version(&stderr).unwrap();
        assert_eq!(version, Version::new(1, 8, 0));
    }

    #[test]
    fn test_parse_openjdk_11() {
        let stderr = "openjdk version \"11.0.25\" 2024-10-15
OpenJDK Runtime Environment (build 11.0.25+9)
OpenJDK 64-Bit Server VM (build 11.0.25+9, mixed mode)"
            .to_string();
        let version = parse_java_version(&stderr).unwrap();
        assert_eq!(version, Version::new(11, 0, 25));
    }
}
