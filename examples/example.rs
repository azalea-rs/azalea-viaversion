use anyhow::Result;
use azalea::{prelude::*, swarm::SwarmBuilder, NoState};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    SwarmBuilder::new()
        .add_account(Account::offline("Azalea"))
        .add_plugins(ViaVersionPlugin::start("1.21.4").await)
        .set_handler(handler)
        .start("localhost")
        .await
        .unwrap();
}

async fn handler(_client: Client, event: Event, _: NoState) -> Result<()> {
    if let Event::Chat(chat) = event {
        println!("{}", chat.message().to_ansi());
    }

    Ok(())
}
