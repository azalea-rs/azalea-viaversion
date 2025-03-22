//! An [`azalea`] bot that repeats chat messages sent by other players.
//!
//! # Note
//! The `never_type` feature is completely optional, see how the `swarm` example does not use it.
#![feature(never_type)]

use azalea::{prelude::*, NoState, StartError};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() -> Result<!, StartError> {
    tracing_subscriber::fmt::init();

    // Initialize a 1.21.4 ViaProxy instance
    let plugin = ViaVersionPlugin::start("1.21.4").await;
    let builder = ClientBuilder::new().add_plugins(plugin);

    // Start the client and connect to a localhost server
    let acc = Account::offline("Azalea");
    builder.set_handler(handle).start(acc, "localhost").await
}

/// A simple event handler that repeats chat messages sent by other players.
async fn handle(bot: Client, event: Event, _: NoState) -> anyhow::Result<()> {
    match event {
        Event::Chat(message) => {
            // Split the message into the sender and message content
            let (sender, message) = message.split_sender_and_content();
            // If the sender is not the bot, repeat the message
            if sender.is_none_or(|sender| sender != bot.profile.name) {
                bot.chat(&message);
            }
        }
        // Log disconnect reasons
        Event::Disconnect(Some(reason)) => eprintln!("Disconnected: {}", reason.to_ansi()),
        Event::Disconnect(None) => eprintln!("Disconnected: No reason provided"),
        _ => {}
    }

    Ok(())
}
