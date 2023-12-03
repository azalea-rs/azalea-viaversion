use azalea::{prelude::*, swarm::SwarmBuilder};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() {
    let account = Account::microsoft("example@example.com").await.unwrap();

    loop {
        let e = SwarmBuilder::new()
            .set_handler(handle)
            .add_plugins(ViaVersionPlugin::start("1.19.4").await)
            .add_account(account.clone())
            .start("localhost")
            .await;
        eprintln!("{e:?}");
    }
}

#[derive(Default, Clone, Component)]
pub struct State;

async fn handle(_bot: Client, event: Event, _state: State) -> anyhow::Result<()> {
    match event {
        Event::Chat(m) => {
            println!("{}", m.message().to_ansi());
        }
        _ => {}
    }

    Ok(())
}
