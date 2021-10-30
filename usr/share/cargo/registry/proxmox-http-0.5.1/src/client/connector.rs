use anyhow::{bail, format_err, Error};
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::*;
use http::Uri;
use hyper::client::HttpConnector;
use openssl::ssl::SslConnector;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use proxmox::sys::linux::socket::set_tcp_keepalive;

use crate::proxy_config::ProxyConfig;
use crate::tls::MaybeTlsStream;
use crate::uri::build_authority;

#[derive(Clone)]
pub struct HttpsConnector {
    connector: HttpConnector,
    ssl_connector: Arc<SslConnector>,
    proxy: Option<ProxyConfig>,
    tcp_keepalive: u32,
}

impl HttpsConnector {
    pub fn with_connector(
        mut connector: HttpConnector,
        ssl_connector: SslConnector,
        tcp_keepalive: u32,
    ) -> Self {
        connector.enforce_http(false);
        Self {
            connector,
            ssl_connector: Arc::new(ssl_connector),
            proxy: None,
            tcp_keepalive,
        }
    }

    pub fn set_proxy(&mut self, proxy: ProxyConfig) {
        self.proxy = Some(proxy);
    }

    async fn secure_stream(
        tcp_stream: TcpStream,
        ssl_connector: &SslConnector,
        host: &str,
    ) -> Result<MaybeTlsStream<TcpStream>, Error> {
        let config = ssl_connector.configure()?;
        let mut conn: SslStream<TcpStream> = SslStream::new(config.into_ssl(host)?, tcp_stream)?;
        Pin::new(&mut conn).connect().await?;
        Ok(MaybeTlsStream::Secured(conn))
    }

    fn parse_status_line(status_line: &str) -> Result<(), Error> {
        if !(status_line.starts_with("HTTP/1.1 200") || status_line.starts_with("HTTP/1.0 200")) {
            bail!("proxy connect failed - invalid status: {}", status_line)
        }
        Ok(())
    }

    async fn parse_connect_response<R: AsyncRead + Unpin>(stream: &mut R) -> Result<(), Error> {
        let mut data: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 256];
        const END_MARK: &[u8; 4] = b"\r\n\r\n";

        'outer: loop {
            let n = stream.read(&mut buffer[..]).await?;
            if n == 0 {
                break;
            }
            let search_start = if data.len() > END_MARK.len() {
                data.len() - END_MARK.len() + 1
            } else {
                0
            };
            data.extend(&buffer[..n]);
            if data.len() >= END_MARK.len() {
                if let Some(pos) = data[search_start..]
                    .windows(END_MARK.len())
                    .position(|w| w == END_MARK)
                {
                    let response = String::from_utf8_lossy(&data);
                    let status_line = match response.split("\r\n").next() {
                        Some(status) => status,
                        None => bail!("missing newline"),
                    };
                    Self::parse_status_line(status_line)?;

                    if pos != data.len() - END_MARK.len() {
                        bail!("unexpected data after connect response");
                    }
                    break 'outer;
                }
            }
            if data.len() > 1024 * 32 {
                // max 32K (random chosen limit)
                bail!("too many bytes");
            }
        }
        Ok(())
    }
}

impl hyper::service::Service<Uri> for HttpsConnector {
    type Response = MaybeTlsStream<TcpStream>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connector.poll_ready(ctx).map_err(|err| err.into())
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let mut connector = self.connector.clone();
        let ssl_connector = Arc::clone(&self.ssl_connector);
        let is_https = dst.scheme() == Some(&http::uri::Scheme::HTTPS);
        let host = match dst.host() {
            Some(host) => host.to_owned(),
            None => {
                return futures::future::err(format_err!("missing URL scheme")).boxed();
            }
        };
        let port = dst.port_u16().unwrap_or(if is_https { 443 } else { 80 });
        let keepalive = self.tcp_keepalive;

        if let Some(ref proxy) = self.proxy {
            let use_connect = is_https || proxy.force_connect;

            let proxy_authority = match build_authority(&proxy.host, proxy.port) {
                Ok(authority) => authority,
                Err(err) => return futures::future::err(err.into()).boxed(),
            };

            let proxy_uri = match Uri::builder()
                .scheme("http")
                .authority(proxy_authority.as_str())
                .path_and_query("/")
                .build()
            {
                Ok(uri) => uri,
                Err(err) => return futures::future::err(err.into()).boxed(),
            };

            let authorization = proxy.authorization.clone();

            if use_connect {
                async move {
                    let mut tcp_stream = connector.call(proxy_uri).await.map_err(|err| {
                        format_err!("error connecting to {} - {}", proxy_authority, err)
                    })?;

                    let _ = set_tcp_keepalive(tcp_stream.as_raw_fd(), keepalive);

                    let mut connect_request = format!("CONNECT {0}:{1} HTTP/1.1\r\n", host, port);
                    if let Some(authorization) = authorization {
                        connect_request
                            .push_str(&format!("Proxy-Authorization: {}\r\n", authorization));
                    }
                    connect_request.push_str(&format!("Host: {0}:{1}\r\n\r\n", host, port));

                    tcp_stream.write_all(connect_request.as_bytes()).await?;
                    tcp_stream.flush().await?;

                    Self::parse_connect_response(&mut tcp_stream).await?;

                    if is_https {
                        Self::secure_stream(tcp_stream, &ssl_connector, &host).await
                    } else {
                        Ok(MaybeTlsStream::Normal(tcp_stream))
                    }
                }
                .boxed()
            } else {
                async move {
                    let tcp_stream = connector.call(proxy_uri).await.map_err(|err| {
                        format_err!("error connecting to {} - {}", proxy_authority, err)
                    })?;

                    let _ = set_tcp_keepalive(tcp_stream.as_raw_fd(), keepalive);

                    Ok(MaybeTlsStream::Proxied(tcp_stream))
                }
                .boxed()
            }
        } else {
            async move {
                let dst_str = dst.to_string(); // for error messages
                let tcp_stream = connector
                    .call(dst)
                    .await
                    .map_err(|err| format_err!("error connecting to {} - {}", dst_str, err))?;

                let _ = set_tcp_keepalive(tcp_stream.as_raw_fd(), keepalive);

                if is_https {
                    Self::secure_stream(tcp_stream, &ssl_connector, &host).await
                } else {
                    Ok(MaybeTlsStream::Normal(tcp_stream))
                }
            }
            .boxed()
        }
    }
}
