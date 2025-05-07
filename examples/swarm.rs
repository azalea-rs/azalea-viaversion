//! A super basic example of adding a `ViaVersionPlugin` to a `SwarmBuilder` and
//! connecting to a localhost server.

use azalea::{prelude::*, swarm::SwarmBuilder};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Initialize a 1.21.4 ViaProxy instance
    let plugin = ViaVersionPlugin::start("1.21.4").await;
    // Create a SwarmBuilder and add the ViaVersion plugin
    let builder = SwarmBuilder::new().add_plugins(plugin);

    // Start the client and connect to a localhost server
    let acc = Account::offline("Azalea");
    builder.add_account(acc).start("localhost").await.unwrap()
}
