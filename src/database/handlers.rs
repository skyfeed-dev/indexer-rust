use anyhow::Result;
use atrium_api::{
    record::KnownRecord,
    types::string::{Did, RecordKey},
};
use chrono::Utc;
use log::warn;
use surrealdb::{engine::remote::ws::Client, Surreal};

use crate::websocket::events::{Commit, Kind};

use super::{
    definitions::{BskyProfile, Record},
    delete_record, utils,
};

/// Handle a new websocket event on the database
pub async fn handle_event(db: &Surreal<Client>, event: Kind) -> Result<()> {
    // Handle event types
    match event {
        Kind::CommitEvent {
            did,
            time_us,
            commit,
        } => {
            // Handle types of commits
            let did_key = utils::did_to_key(did.as_str())?;
            match commit {
                Commit::CreateOrUpdate {
                    rev,
                    collection,
                    rkey,
                    record,
                    cid,
                } => {
                    on_commit_event_createorupdate(
                        db, did, time_us, did_key, rev, collection, rkey, record, cid,
                    )
                    .await?
                }
                Commit::Delete {
                    rev,
                    collection,
                    rkey,
                } => {
                    on_commit_event_delete(db, did, time_us, did_key, rev, collection, rkey).await?
                }
            }
        }
        Kind::IdentityEvent {
            did,
            time_us,
            identity,
        } => warn!(target: "indexer", "ignored identity event {} {} {:?}",
                did.as_str(), time_us, identity), // TODO implement
        Kind::KeyEvent {
            did,
            time_us,
            account,
        } => warn!(target: "indexer", "ignored key event {} {} {:?}",
                did.as_str(), time_us, account), // TODO implement
    }

    Ok(())
}

/// If the new commit is a create or update, handle it
async fn on_commit_event_createorupdate(
    db: &Surreal<Client>,
    did: Did,
    _time_us: u64,
    did_key: String,
    _rev: String,
    collection: String,
    rkey: RecordKey,
    record: KnownRecord,
    _cid: String,
) -> Result<()> {
    utils::ensure_valid_rkey(rkey.to_string())?;
    match record {
        KnownRecord::AppBskyActorProfile(d) => {
            // NOTE: using .ok() here isn't optimal, incorrect data should
            // probably not be entered into the database at all, but for now
            // we'll just ignore it.
            let profile = BskyProfile {
                display_name: d.display_name.clone(),
                description: d.description.clone(),
                avatar: None, // TODO Implement
                banner: None, // TODO Implement
                created_at: d
                    .created_at
                    .as_ref()
                    .and_then(|dt| utils::extract_dt(dt).ok()),
                seen_at: Utc::now().into(),
                joined_via_starter_pack: d
                    .joined_via_starter_pack
                    .as_ref()
                    .and_then(|d| utils::strong_ref_to_record_id(d).ok()),
                pinned_post: d
                    .pinned_post
                    .as_ref()
                    .and_then(|d| utils::strong_ref_to_record_id(d).ok()),
                labels: d
                    .labels
                    .as_ref()
                    .and_then(|d| utils::extract_self_labels(d)),
            };
            // TODO this should be a db.upsert(...).merge(...)
            let _: Option<Record> = db.upsert(("did", did_key)).content(profile).await?;
        }
        KnownRecord::AppBskyGraphFollow(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);
            let to = utils::did_to_key(d.subject.as_str())?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE did:{}->follow->did:{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        KnownRecord::AppBskyFeedLike(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);
            let to = utils::at_uri_to_record_id(&d.subject.uri)?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE did:{}->like->{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        KnownRecord::AppBskyFeedRepost(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);
            let to = utils::at_uri_to_record_id(&d.subject.uri)?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE did:{}->repost->{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        KnownRecord::AppBskyGraphBlock(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);
            let to = utils::did_to_key(d.subject.as_str())?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE did:{}->block->did:{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        KnownRecord::AppBskyGraphListblock(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);
            let to = utils::at_uri_to_record_id(&d.subject)?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE did:{}->listblock->{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        KnownRecord::AppBskyGraphListitem(d) => {
            // TODO ensure_valid_rkey_strict(rkey.as_str())?;
            let from = utils::did_to_key(did.as_str())?;
            let id = format!("{}_{}", rkey.as_str(), from);

            let from = utils::at_uri_to_record_id(&d.list)?;
            let to = utils::did_to_key(&d.subject)?;
            let created_at = utils::extract_dt(&d.created_at)?;

            let query = format!(
                "RELATE {}->listitem->did:{} SET id = '{}', createdAt = {};",
                from, to, id, created_at
            );

            let _ = db.query(query).await?;
        }
        _ => {
            warn!(target: "indexer", "ignored create_or_update {} {} {}",
                did.as_str(), collection, rkey.as_str());
        }
    }

    Ok(())
}

/// If the new commit is a delete, handle it
async fn on_commit_event_delete(
    db: &Surreal<Client>,
    did: Did,
    _time_us: u64,
    _did_key: String,
    _rev: String,
    collection: String,
    rkey: RecordKey,
) -> Result<()> {
    utils::ensure_valid_rkey(rkey.to_string())?;

    let id = format!("{}_{}", rkey.as_str(), utils::did_to_key(did.as_str())?);
    match collection.as_str() {
        "app.bsky.graph.follow" => {
            delete_record(db, "follow", &id).await?;
        }
        "app.bsky.feed.repost" => {
            delete_record(db, "repost", &id).await?;
        }
        "app.bsky.feed.like" => {
            delete_record(db, "like", &id).await?;
        }
        "app.bsky.graph.block" => {
            delete_record(db, "block", &id).await?;
        }
        "app.bsky.graph.listblock" => {
            delete_record(db, "listblock", &id).await?;
        }
        "app.bsky.feed.post" => {
            for table in vec!["post", "posts", "replies", "replyto", "quotes"] {
                delete_record(db, table, &id).await?;
            }
        }
        "app.bsky.graph.listitem" => {
            delete_record(db, "listitem", &id).await?;
        }
        "app.bsky.feed.threadgate" => {
            delete_record(db, "threadgate", &id).await?;
        }
        "app.bsky.feed.generator" => {
            delete_record(db, "feed", &id).await?;
        }
        "app.bsky.graph.list" => {
            delete_record(db, "list", &id).await?;
        }
        "app.bsky.feed.postgate" => {
            delete_record(db, "postgate", &id).await?;
        }
        "app.bsky.graph.starterpack" => {
            delete_record(db, "starterpack", &id).await?;
        }
        "app.bsky.labeler.service" => {
            delete_record(db, "app_bsky_labeler_service", &id).await?;
        }
        "chat.bsky.actor.declaration" => {
            delete_record(db, "chat_bsky_actor_declaration", &id).await?;
        }
        _ => {
            warn!(target: "indexer", "could not handle operation {} {} {} {}",
                did.as_str(), "delete", collection, rkey.as_str());
        }
    }

    Ok(())
}
