#![feature(async_await)]

use std::io;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::net::ToSocketAddrs;
use std::io::BufReader;
use structopt::StructOpt;
use async_std::net::TcpStream;
use async_std::task;
use async_std::io as aio;
use futures::prelude::*;
use tokio_rustls::{ TlsConnector, rustls::ClientConfig, webpki::DNSNameRef };

#[derive(StructOpt)]
struct Options {
    host: String,

    /// port
    #[structopt(short="p", long="port", default_value="443")]
    port: u16,

    /// domain
    #[structopt(short="d", long="domain")]
    domain: Option<String>,

    /// cafile
    #[structopt(short="c", long="cafile", parse(from_os_str))]
    cafile: Option<PathBuf>
}


fn main() -> io::Result<()> {
    let options = Options::from_args();

    let addr = (options.host.as_str(), options.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let domain = options.domain.unwrap_or(options.host);
    let content = format!(
        "GET / HTTP/1.0\r\nHost: {}\r\n\r\n",
        domain
    );

    let mut config = ClientConfig::new();
    if let Some(cafile) = &options.cafile {
        let mut pem = BufReader::new(File::open(cafile)?);
        config.root_store.add_pem_file(&mut pem)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid cert"))?;
    } else {
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    }
    let connector = TlsConnector::from(Arc::new(config));

    task::block_on(async {
        let stream = TcpStream::connect(&addr).await?;
        let (stdin, mut stdout) = (aio::stdin(), aio::stdout());

        let domain = DNSNameRef::try_from_ascii_str(&domain)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

        let mut stream = connector.connect(domain, stream).await?;
        stream.write_all(content.as_bytes()).await?;

        let (reader, mut writer) = stream.split();
        future::try_join(
            reader.copy_into(&mut stdout),
            stdin.copy_into(&mut writer)
        ).await?;

        Ok(())
    })
}
