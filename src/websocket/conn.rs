use std::{future::Future, sync::Arc};

use anyhow::Context;
use fastwebsockets::{handshake, WebSocket};
use hyper::{
    header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    rt::Executor,
    upgrade::Upgraded,
    Request,
};
use hyper_util::rt::TokioIo;
use log::debug;
use tokio::{net::TcpStream, task};
use tokio_rustls::{
    rustls::{
        pki_types::{pem::PemObject, CertificateDer, ServerName},
        ClientConfig, RootCertStore,
    },
    TlsConnector,
};

/// A tokio executor for hyper
struct TokioExecutor;

impl<Fut> Executor<Fut> for TokioExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        task::spawn(fut);
    }
}

/// List of supported nsids
const NSIDS: [&str; 15] = [
    "app.bsky.actor.profile",
    "app.bsky.feed.generator",
    "app.bsky.feed.like",
    "app.bsky.feed.post",
    "app.bsky.feed.postgate",
    "app.bsky.feed.repost",
    "app.bsky.feed.threadgate",
    "app.bsky.graph.block",
    "app.bsky.graph.follow",
    "app.bsky.graph.list",
    "app.bsky.graph.listblock",
    "app.bsky.graph.listitem",
    "app.bsky.graph.starterpack",
    "app.bsky.labeler.service",
    "chat.bsky.actor.declaration",
];

/// Connect to a websocket server
pub async fn connect_tls(
    host: &String,
    certificate: &String,
    cursor: Option<u64>,
) -> anyhow::Result<WebSocket<TokioIo<Upgraded>>> {
    // prepare tls store
    debug!(target: "indexer", "Creating tls store for certificate: {}", certificate);
    let mut tls_store = RootCertStore::empty();
    let tls_cert = CertificateDer::from_pem_file(certificate)
        .with_context(|| format!("Unable to parse certificate from: {}", certificate))?;
    tls_store
        .add(tls_cert)
        .with_context(|| format!("Unable to add certificate to tls store: {}", certificate))?;

    // create tcp connection to server
    debug!(target: "indexer", "Connecting to: {}", host);
    let addr = format!("{}:443", host);
    let tcp_stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Unable to open tcp connection to: {}", addr))?;

    // encrypt the tcp stream with tls
    debug!(target: "indexer", "Establishing tls connection to: {}", host);
    let tls_config = ClientConfig::builder()
        .with_root_certificates(tls_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(tls_config));
    let tls_domain = ServerName::try_from(host.clone())
        .with_context(|| format!("Invalid dns name: {}", host))?;
    let tls_stream = connector
        .connect(tls_domain, tcp_stream)
        .await
        .with_context(|| format!("Unable to establish tls connection to: {}", host))?;

    // build uri
    let uri = format!(
        "wss://{}/subscribe?wantedCollections={}{}",
        host,
        NSIDS.join("&wantedCollections="),
        cursor.map_or_else(|| String::new(), |c| format!("&cursor={}", c))
    );

    // upgrade the connection to a websocket
    debug!(target: "indexer", "Upgrading connection to websocket: {}", &uri);
    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .header(HOST, host)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(SEC_WEBSOCKET_KEY, handshake::generate_key())
        .header(SEC_WEBSOCKET_VERSION, "13")
        .body(String::new())
        .with_context(|| format!("Unable to build websocket upgrade request for: {}", uri))?;

    let (ws, _) = handshake::client(&TokioExecutor, req, tls_stream)
        .await
        .with_context(|| format!("Unable to upgrade connection to websocket: {}", uri))?;

    Ok(ws)
}
