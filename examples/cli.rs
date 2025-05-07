//! A swarm bot that allows you to modify the accounts being used and the target
//! version and the server address from the CLI.
//!
//! Example usage:
//! ```sh
//! cargo r -r --example cli -- --account bot --server localhost --version 1.21.5
//! ```

use std::{env, process, time::Duration};

use azalea::{prelude::*, swarm::SwarmBuilder};
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() {
    let args = parse_args();

    tracing_subscriber::fmt::init();

    let mut builder = SwarmBuilder::new();

    for username_or_email in &args.accounts {
        let account = if username_or_email.contains('@') {
            Account::microsoft(username_or_email).await.unwrap()
        } else {
            Account::offline(username_or_email)
        };

        builder = builder.add_account(account);
    }

    let plugin = ViaVersionPlugin::start(args.version).await;

    builder
        .add_plugins(plugin)
        .join_delay(Duration::from_millis(100))
        .start(args.server)
        .await
        .unwrap();
}

#[derive(Debug, Clone, Default)]
pub struct Args {
    pub accounts: Vec<String>,
    pub server: String,
    pub version: String,
}

fn parse_args() -> Args {
    let mut accounts = Vec::new();
    let mut server = "localhost".to_string();
    // default to the latest version
    let mut version = azalea::protocol::packets::VERSION_NAME.to_string();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--account" | "-A" => {
                for account in args.next().expect("Missing account").split(',') {
                    accounts.push(account.to_string());
                }
            }
            "--server" | "-S" => {
                server = args.next().expect("Missing server address");
            }
            "--version" | "-V" => {
                version = args.next().expect("Missing version");
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                process::exit(1);
            }
        }
    }

    if accounts.is_empty() {
        accounts.push("azalea".to_string());
    }

    Args {
        accounts,
        server,
        version,
    }
}
