use std::{net::SocketAddr, path::PathBuf};

/// Extra options for creating a [`ViaVersionPlugin`].
///
/// See [`ViaVersionPlugin::start_with_opts`] for more details.
///
/// [`ViaVersionPlugin`]: crate::ViaVersionPlugin
/// [`ViaVersionPlugin::start_with_opts`]: crate::ViaVersionPlugin::start_with_opts
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct ViaVersionOpts {
    /// See [`Self::bind_addr`].
    pub bind_addr: Option<SocketAddr>,
    /// See [`Self::proxy`].
    pub proxy: Option<String>,
    /// See [`Self::viaproxy_args`].
    pub viaproxy_args: Vec<String>,
    /// See [`Self::viaproxy_data_path`].
    pub viaproxy_data_path: Option<PathBuf>,
}

impl ViaVersionOpts {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.bind_addr = Some(bind_addr);
        self
    }
    /// Allows you to make your bots connect through a SOCKS4, SOCKS5, HTTP, or
    /// HTTPS proxy.
    ///
    /// Note that HTTP/HTTPS proxies often don't support making arbitrary TCP
    /// connections; SOCKS5 is recommended.
    ///
    /// Supported formats:
    /// - `type://address:port`
    /// - `type://username:password@address:port`
    ///
    /// This is necessary if you want to use Azalea with a proxy and ViaVersion
    /// at the same time. This is incompatible with `JoinOpts::proxy`.
    pub fn proxy(mut self, proxy: &str) -> Self {
        self.proxy = Some(proxy.to_string());
        self
    }
    /// Set extra command line arguments for ViaProxy.
    ///
    /// More info:
    /// https://github.com/ViaVersion/ViaProxy/blob/main/src/main/java/net/raphimc/viaproxy/protocoltranslator/viaproxy/ViaProxyConfig.java
    ///
    /// ```rs
    /// let plugin = ViaVersionPlugin::start_with_opts(
    ///     args.version,
    ///     ViaVersionOpts::new()
    ///         .viaproxy_args(["--ignore-protocol-translation-errors", "true"])
    /// )
    // .await;
    /// ```
    pub fn viaproxy_args<T: AsRef<str>>(
        mut self,
        viaproxy_args: impl IntoIterator<Item = T>,
    ) -> Self {
        self.viaproxy_args = viaproxy_args
            .into_iter()
            .map(|s| s.as_ref().to_owned())
            .collect();
        self
    }
    /// Set the path to the ViaProxy jar and data will be stored at.
    ///
    /// If this is unset, it defaults to `~/.minecraft/azalea-viaproxy`.
    pub fn viaproxy_data_path(mut self, viaproxy_data_path: impl Into<PathBuf>) -> Self {
        self.viaproxy_data_path = Some(viaproxy_data_path.into());
        self
    }
}
