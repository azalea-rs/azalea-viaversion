//! A super basic example of adding a `ViaVersionPlugin` to a `ClientBuilder`
//! and connecting to a localhost server.

use azalea::{app::AppExit, prelude::*};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() -> AppExit {
    tracing_subscriber::fmt::init();

    // Initialize a 1.21.4 ViaProxy instance
    let plugin = ViaVersionPlugin::start("1.21.4").await;
    // Create a ClientBuilder and add the ViaVersion plugin
    let builder = ClientBuilder::new().add_plugins(plugin);

    // Start the client and connect to a localhost server
    builder.start(Account::offline("Azalea"), "localhost").await
}
