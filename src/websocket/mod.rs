use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::Context;
use fastwebsockets::{OpCode, WebSocket};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log::{debug, error, info, trace, warn};
use surrealdb::{engine::remote::ws::Client, Surreal};
use tokio::{runtime::Builder, time::sleep};

mod conn;
pub mod events;
mod handler;

/// Shared state for the websocket module
#[derive(Debug)]
struct SharedState {
    host: String,
    rx: Arc<Mutex<Receiver<(String, bool)>>>,
    db: Surreal<Client>,
    cursor: AtomicU64,
}

impl SharedState {
    /// Update the cursor
    pub fn update_cursor(&self, cursor: u64) {
        self.cursor.store(cursor, Ordering::Relaxed);
    }
}

/// Subscribe to a websocket server
pub async fn start(
    host: String,
    certificate: String,
    cursor: u64,
    handlers: Option<usize>,
    db: Surreal<Client>,
) -> anyhow::Result<()> {
    // create a shared state
    info!(target: "indexer", "Entering websocket loop");
    let (tx, rx) = mpsc::channel();
    let state = Arc::new(SharedState {
        host: host.clone(),
        rx: Arc::new(Mutex::new(rx)),
        db,
        cursor: AtomicU64::new(cursor),
    });

    // spin up the thread pool
    let cpus = handlers.unwrap_or_else(num_cpus::get);
    info!(target: "indexer", "Spinning up {} handler threads", cpus);
    for i in 0..cpus {
        let state = state.clone();
        thread::Builder::new()
            .name(format!("WebSocket Handler Thread {}", i))
            .spawn(move || {
                Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let res = thread_handler(state).await;
                        if let Err(e) = res {
                            error!(target: "indexer", "Handler thread {} failed: {:?}", i, e);
                        } else {
                            debug!(target: "indexer", "Handler thread {} exited", i);
                        }
                    });
            })
            .context("Failed to spawn handler thread")?;
    }

    // loop infinitely, ensuring connection aborts are handled
    loop {
        // get current cursor
        let cursor = {
            let c = (&state).cursor.load(Ordering::Relaxed) as u64;
            if c == 0 {
                None
            } else {
                Some(c)
            }
        };

        // create websocket connection
        info!(target: "indexer", "Establishing new connection to: {}", host);
        let ws = conn::connect_tls(&host, &certificate, cursor).await;
        if let Err(e) = ws {
            warn!(target: "indexer", "Unable to open websocket connection to {}: {:?}", host, e);
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        let ws = ws.unwrap();

        // handle the websocket connection
        info!(target: "indexer", "Handling websocket connection starting at cursor: {:?}", cursor);
        let res = manage_ws(&tx, ws).await;
        if let Err(e) = res {
            warn!(target: "indexer", "Websocket connection failed: {:?}", e);
        }

        // rewind cursor by 10 seconds
        {
            const REWIND_TIME: u64 = 10_000_000; // 10 seconds in microseconds
            let cursor = (&state).cursor.fetch_sub(REWIND_TIME, Ordering::Relaxed);
            info!(target: "indexer", "Rewinding cursor by 10 seconds: {} -> {}", cursor, cursor - REWIND_TIME);
        }

        // let the server breathe
        sleep(Duration::from_millis(200)).await;
    }
}

async fn manage_ws(
    tx: &Sender<(String, bool)>,
    mut ws: WebSocket<TokioIo<Upgraded>>,
) -> anyhow::Result<()> {
    let mut time = Instant::now();
    loop {
        // try to read a message
        let msg = ws
            .read_frame()
            .await
            .context("Failed to read frame from websocket")?;

        // check if cursor needs an update
        let update_cursor = if time.elapsed().as_secs() >= 60 {
            time = Instant::now();
            true
        } else {
            false
        };

        // handle message
        match msg.opcode {
            // spec states only text frames are allowed
            OpCode::Continuation | OpCode::Binary | OpCode::Ping | OpCode::Pong => {
                warn!(target: "indexer", "Unexpected opcode received: {:?}", msg.opcode);
            }
            // can be emitted by the server
            OpCode::Close => {
                anyhow::bail!("Unexpected connection close received: {:?}", msg.payload);
            }
            // handle text message
            OpCode::Text => {
                trace!(target: "indexer", "Received text message: {}", msg.payload.len());
                let text = String::from_utf8(msg.payload.to_vec())
                    .context("Failed to decode text message")?;
                tx.send((text, update_cursor))
                    .context("Failed to send message to handler thread")?;
            }
        };
    }
}

async fn thread_handler(state: Arc<SharedState>) -> anyhow::Result<()> {
    // loop infinitely, handling messages
    loop {
        // get the next message
        let msg = state.rx.lock().unwrap().recv();
        if let Err(e) = msg {
            debug!(target: "indexer", "Receiver closed: {:?}", e);
            break;
        }
        let msg = msg.unwrap();

        // handle the message
        trace!(target: "indexer", "Handling message: {}", &msg.0);
        let res = handler::handle_message(&state, msg.0, msg.1).await;
        if let Err(e) = res {
            warn!(target: "indexer", "Failed to handle message: {:?}", e);
        }
    }

    Ok(())
}
