use anyhow::Context;

use crate::database;

use super::{events, SharedState};

/// Handle a message from the websocket in parallel
pub async fn handle_message(
    state: &SharedState,
    msg: String,
    update_cursor: bool,
) -> anyhow::Result<()> {
    // parse event
    let event = events::parse_event(msg)?;

    // update cursor
    let time = match &event {
        events::Kind::CommitEvent { time_us, .. } => *time_us,
        events::Kind::IdentityEvent { time_us, .. } => *time_us,
        events::Kind::KeyEvent { time_us, .. } => *time_us,
    };
    state.update_cursor(time);
    if update_cursor {
        database::write_cursor(&state.db, &state.host, time)
            .await
            .context("Unable to write cursor to database!")?;
    }

    database::handlers::handle_event(&state.db, event)
        .await
        .context("Unable to handle event")?;

    Ok(())
}
