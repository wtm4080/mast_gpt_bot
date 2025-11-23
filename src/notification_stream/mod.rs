use crate::config::BotConfig;
use crate::conversation_store::ConversationStore;
use anyhow::Result;
use std::sync::Arc;

mod connection;
mod context;
mod handler;
mod rate_limit;

pub async fn run_notification_stream(
    client: &reqwest::Client,
    config: &BotConfig,
    conv_store: Arc<ConversationStore>,
) -> Result<()> {
    connection::run_notification_stream(client, config, conv_store).await
}
