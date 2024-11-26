use ::atrium_api::{
    app::bsky::actor::profile::RecordLabelsRefs,
    com::atproto::repo::strong_ref::Main,
    types::{string::RecordKey, Union},
};
use anyhow::{Context, Result};
use atrium_api::types::string as atrium_api;
use lazy_static::lazy_static;
use regex::Regex;
use surrealdb::RecordId;

lazy_static! {
    static ref VALID_DID_KEY_REGEX: Regex = Regex::new(r"^(plc|web)_[a-z0-9_]+$").unwrap();
}

/// Converts a datetime from the atrium API to a surreal datetime
pub fn extract_dt(dt: &atrium_api::Datetime) -> Result<surrealdb::Datetime> {
    Ok(chrono::DateTime::parse_from_rfc3339(dt.as_str())
        .context("Datetime conversion failed")?
        .to_utc()
        .into())
}

/// Extracts the self labels from a record labels refs
pub fn extract_self_labels(labels: &Union<RecordLabelsRefs>) -> Option<Vec<String>> {
    match labels {
        Union::Refs(refs) => match refs {
            RecordLabelsRefs::ComAtprotoLabelDefsSelfLabels(labels) => {
                Some(labels.values.iter().map(|l| l.val.clone()).collect())
            }
        },
        Union::Unknown(_) => None,
    }
}

/// Converts a DID to a key
pub fn did_to_key(did: &str) -> Result<String> {
    did_to_key_impl(did, false)
}

/// Converts a DID to a (full) key
pub fn did_to_key_impl(did: &str, full: bool) -> Result<String> {
    // did:plc covers 99.99% of all DIDs
    let val = if did.starts_with("did:plc:") {
        format!("plc_{}", &did[8..])
    } else if did.starts_with("did:web:") {
        format!("web_{}", &did[8..].replace('.', "_").replace('-', "__"))
    } else {
        anyhow::bail!("Invalid DID {}", did);
    };

    if !VALID_DID_KEY_REGEX.is_match(&val) {
        anyhow::bail!("Found invalid DID: {} {} {}", did, full, val);
    }

    if full {
        Ok(format!("did:{}", val))
    } else {
        Ok(val)
    }
}

/// Converts a strong ref to a record ID
pub fn strong_ref_to_record_id(sr: &Main) -> Result<RecordId> {
    Ok(at_uri_to_record_id(&sr.uri).context("Unable to convert strong ref to record id")?)
}

/// Converts an AT URI to a record ID
pub fn at_uri_to_record_id(uri: &str) -> Result<RecordId> {
    let u: Vec<&str> = uri.split('/').collect();
    let u_hostname = u.get(2).context("Hostname missing")?.to_string();
    let u_collection = *u.get(3).context("Collection type missing")?;
    let u_rkey = u.get(4).context("Rkey missing")?.to_string();

    let table = match u_collection {
        "app.bsky.feed.post" => "post",
        "app.bsky.graph.list" => "list",
        "app.bsky.graph.starterpack" => "starterpack",
        _ => anyhow::bail!("Unsupported URI {}", uri),
    };

    let mut did = did_to_key(&u_hostname)?;
    if did.starts_with("plc_did:plc:") {
        did = format!("plc_{}", &did[12..]);
    }

    ensure_valid_rkey(u_rkey.to_string())?;

    Ok(RecordId::from_table_key(
        table,
        format!("{}_{}", u_rkey, did),
    ))
}

/// Ensures that the provided rkey is valid
pub fn ensure_valid_rkey(rkey: String) -> Result<()> {
    let key = RecordKey::new(rkey);
    if key.is_err() {
        anyhow::bail!("Provided rkey is not valid!");
    }
    Ok(())
}
