# Azalea ViaVersion Plugin

Enhance multi-version compatibility for your [Azalea] bots using [ViaProxy].

## Usage

To use this plugin, simply add the dependencies:

- `cargo add --git https://github.com/azalea-rs/azalea azalea`
- `cargo add --git https://github.com/azalea-rs/azalea-viaversion azalea-viaversion`

Then integrate it into your `ClientBuilder` or `SwarmBuilder`:

- `.add_plugins(ViaVersionPlugin::start("1.21.4").await)`

```rs
#[tokio::main]
async fn main() {
    SwarmBuilder::new()
        .add_account(Account::offline("Azalea"))
        .add_plugins(ViaVersionPlugin::start("1.21.4").await)
        .start("localhost")
        .await
        .unwrap();
}
```

## How it works

The plugin will automatically download ViaProxy to `~/.minecraft/azalea-viaversion`. It then starts up ViaProxy in the
background and changes the connection address for the bots to the proxy. It also implements OpenAuthMod so it can keep
using Azalea's normal auth mechanisms.

[Azalea]: https://github.com/mat-1/azalea

[ViaProxy]: https://github.com/ViaVersion/ViaProxy