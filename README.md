# Azalea ViaVersion

An [Azalea] plugin using [ViaProxy] to support connecting to every Minecraft server version.

## Usage

Add the `ViaVersionPlugin` to your `ClientBuilder` or `SwarmBuilder`.

```rust
use azalea::prelude::*;
use azalea_viaversion::ViaVersionPlugin;

#[tokio::main]
async fn main() {
    ClientBuilder::new()
        .add_plugins(ViaVersionPlugin::start("1.21.4").await)
        .start(Account::offline("Azalea"), "localhost")
        .await
        .unwrap();
}
```

## Compatibility

This plugin depends on the `main` branch of [Azalea].

> [!IMPORTANT]
> If you want use a different branch or fork you **_must_** patch your project's `Cargo.toml` file!

```toml
[dependencies]
azalea = { git = "https://github.com/azalea-rs/azalea" }
azalea-viaversion = { git = "https://github.com/azalea-rs/azalea-viaversion" }

# Note: You can also use this to pin Azalea to a specific commit.
# [patch.'https://github.com/azalea-rs/azalea']
# azalea = { git = "https://github.com/azalea-rs/azalea", branch = "1.21.4" }
```

## Matrix/Discord

If you'd like to chat about Azalea, you can join the Matrix space at [#azalea:matdoes.dev](https://matrix.to/#/#azalea:matdoes.dev) (recommended) or the Discord server at [discord.gg/FaRey6ytmC](https://discord.gg/FaRey6ytmC) (they're bridged so you don't need to join both).

## How it works

The plugin will automatically download ViaProxy to `~/.minecraft/azalea-viaversion`. It then starts up ViaProxy in the
background and changes the connection address for the bots to the proxy. It also implements OpenAuthMod so it can keep
using Azalea's normal auth mechanisms.

[Azalea]: https://github.com/azalea-rs/azalea
[ViaProxy]: https://github.com/ViaVersion/ViaProxy
