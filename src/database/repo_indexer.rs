use crate::database::{handlers::on_commit_event_createorupdate, utils::unsafe_user_key_to_did};
use anyhow::Context;
use atrium_api::{
    record::KnownRecord,
    types::string::{Did, RecordKey},
};
use futures::stream::{Stream, TryStreamExt};
use ipld_core::cid::Cid;
use iroh_car::CarReader;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use surrealdb::{engine::any::Any, Surreal};

pub async fn start_full_repo_indexer(db: Surreal<Any>, max_tasks: usize) -> anyhow::Result<()> {
    let mut processed_dids: BTreeSet<String> = BTreeSet::new();
    let (tx, rx) = async_channel::bounded(10_000);

    let state = Arc::new(SharedState { rx: rx, db });

    info!(target: "indexer", "Spinning up {} handler tasks", max_tasks);

    for i in 0..max_tasks {
        let state = state.clone();
        tokio::spawn(async move {
            /*   thread::Builder::new()
            .name(format!("Indexer Thread {}", i))
            .spawn(move || {
                Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .unwrap()
                    .block_on(async { */
            let res = thread_handler(state).await;
            if let Err(e) = res {
                error!(target: "indexer", "Handler thread {} failed: {:?}", i, e);
            } else {
                debug!(target: "indexer", "Handler thread {} exited", i);
            }
        });
        /* })
        .context("Failed to spawn handler thread")?; */
    }

    let mut anchor = "3juj4".to_string();
    loop {
        info!(target: "repo_indexer", "anchor {}", anchor);

        let mut res = state
            .db
            .query(format!(
                "SELECT id,in,out FROM follow:{}.. LIMIT 100000;",
                anchor
            ))
            .await?;
        let likes_res: Vec<BskyFollowRes> = res.take(0)?;

        if likes_res.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(10000)).await;
            continue;
        }

        anchor = format!("{}", likes_res.last().unwrap().id.key());

        let mut dids: BTreeSet<String> = BTreeSet::new();

        for like in likes_res {
            for record_id in vec![like.from, like.to] {
                let did = unsafe_user_key_to_did(&format!("{}", record_id.key()));
                if !processed_dids.contains(&did) {
                    dids.insert(did.clone());
                    processed_dids.insert(did);
                }
            }
        }

        for did in dids {
            tx.send(did)
                .await
                .context("Failed to send message to handler thread")?;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[derive(Deserialize)]
struct BskyFollowRes {
    #[serde(rename = "in")]
    pub from: surrealdb::RecordId,
    #[serde(rename = "out")]
    pub to: surrealdb::RecordId,

    pub id: surrealdb::RecordId,
}

#[derive(Debug)]
struct SharedState {
    rx: async_channel::Receiver<String>,
    db: Surreal<Any>,
}

async fn thread_handler(state: Arc<SharedState>) -> anyhow::Result<()> {
    // loop infinitely, handling repo index tasks
    loop {
        // get the next repo to be indexed
        let did = {
            let x = state.rx.recv().await;
            x.unwrap()
        };

        let res = index_repo(&state.db, &did).await;
        if let Err(e) = res {
            warn!(target: "indexer", "Failed to index repo {}: {:?}", did, e);
        }
    }
}

pub async fn index_repo(db: &Surreal<Any>, did: &String) -> anyhow::Result<()> {
    let did_key = crate::database::utils::did_to_key(did.as_str())?;

    let li: Option<LastIndexedTimestamp> = db.select(("li_did", &did_key)).await?;
    if li.is_some() {
        debug!("skip {}", did);
        return Ok(());
    }

    let client = reqwest::Client::new();

    let timestamp_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let resp = client
        .get(format!("https://plc.directory/{}", did))
        .send()
        .await?
        .json::<PlcDirectoryDidResponse>()
        .await?;

    if let Some(service) = resp.service.first() {
        let car_res_bytes = client
            .get(format!(
                "{}/xrpc/com.atproto.sync.getRepo?did={}",
                service.service_endpoint, did,
            ))
            .send()
            .await?
            .bytes()
            .await?;

        let buf_reader = tokio::io::BufReader::new(&car_res_bytes[..]);
        let car_reader = CarReader::new(buf_reader).await?;
        let files: Vec<(ipld_core::cid::Cid, Vec<u8>)> = car_reader.stream().try_collect().await?;

        let mut map: BTreeMap<ipld_core::cid::Cid, &Vec<u8>> = BTreeMap::new();

        for f in &files {
            map.insert(f.0, &f.1);
        }

        for file in &files {
            let node_data_res = serde_ipld_dagcbor::from_reader::<NodeData, _>(&file.1[..]);

            if let Ok(node_data) = node_data_res {
                let mut key = "".to_string();
                for e in node_data.e {
                    let k = String::from_utf8(e.k)?;
                    key = format!("{}{}", key.split_at(e.p as usize).0, k);

                    let block = map.get(&e.v);

                    if let Some(b) = block {
                        let record_res = serde_ipld_dagcbor::from_reader::<KnownRecord, _>(&b[..]);
                        if record_res.is_ok() {
                            let record = record_res.unwrap();

                            let mut parts = key.split("/");

                            let res = on_commit_event_createorupdate(
                                db,
                                Did::new(did.clone()).unwrap(),
                                did_key.clone(),
                                parts.next().unwrap().to_string(),
                                RecordKey::new(parts.next().unwrap().to_string()).unwrap(),
                                record,
                            )
                            .await;
                            if res.is_err() {
                                warn!(
                                    "on_commit_event_createorupdate {} {}",
                                    res.unwrap_err(),
                                    did
                                );
                            }
                        }
                    }
                }
            }
        }
        let _: Option<super::definitions::Record> = db
            .upsert(("li_did", did_key))
            .content(LastIndexedTimestamp {
                time_us: timestamp_us as u64,
                time_dt: chrono::Utc::now().into(),
            })
            .await?;
    }
    Ok(())
}

#[derive(Deserialize, Debug)]
struct PlcDirectoryDidResponse {
    #[serde(rename = "alsoKnownAs")]
    also_known_as: Vec<String>,
    service: Vec<PlcDirectoryDidResponseService>,
}

#[derive(Deserialize, Debug)]
struct PlcDirectoryDidResponseService {
    #[serde(rename = "serviceEndpoint")]
    service_endpoint: String,
    #[serde(rename = "type")]
    type_: String,
    id: String,
}

#[derive(Deserialize, Debug)]
pub struct TreeEntry {
    pub p: u8,
    #[serde(with = "serde_bytes")]
    pub k: Vec<u8>,
    pub v: Cid,
    pub t: Option<Cid>,
}

#[derive(Deserialize, Debug)]
pub struct NodeData {
    pub l: Option<Cid>,
    pub e: Vec<TreeEntry>,
}

/// Database struct for a repo indexing timestamp
#[derive(Debug, Serialize, Deserialize)]
pub struct LastIndexedTimestamp {
    pub time_us: u64,
    pub time_dt: surrealdb::Datetime,
}
