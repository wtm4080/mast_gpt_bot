use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task;

#[derive(Clone)]
pub struct ConversationStore {
    inner: Arc<Mutex<Connection>>,
}

fn lock_connection(conn: &Arc<Mutex<Connection>>) -> Result<MutexGuard<'_, Connection>> {
    conn.lock().map_err(|_| anyhow!("ConversationStore mutex poisoned"))
}

impl ConversationStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open SQLite database")?;

        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                thread_key TEXT NOT NULL UNIQUE,
                last_response_id TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .context("Failed to init conversations table")?;

        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    pub async fn get_previous_response_id(&self, thread_key: &str) -> Result<Option<String>> {
        let thread_key = thread_key.to_string();
        let conn = self.inner.clone();

        let opt: Option<String> = task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = lock_connection(&conn)?;
            let mut stmt =
                conn.prepare("SELECT last_response_id FROM conversations WHERE thread_key = ?1")?;
            let mut rows = stmt.query(params![thread_key])?;
            if let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                Ok(Some(id))
            } else {
                Ok(None)
            }
        })
        .await
        .context("ConversationStore get_previous_response_id task failed")??;

        Ok(opt)
    }

    pub async fn upsert_last_response_id(&self, thread_key: &str, response_id: &str) -> Result<()> {
        let thread_key = thread_key.to_string();
        let response_id = response_id.to_string();
        let conn = self.inner.clone();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

        task::spawn_blocking(move || -> Result<()> {
            let conn = lock_connection(&conn)?;
            conn.execute(
                r#"
                INSERT INTO conversations (thread_key, last_response_id, updated_at)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(thread_key) DO UPDATE SET
                    last_response_id = excluded.last_response_id,
                    updated_at = excluded.updated_at
                "#,
                params![thread_key, response_id, now],
            )?;
            Ok(())
        })
        .await
        .context("ConversationStore upsert_last_response_id task failed")??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_thread_has_no_previous_response() {
        let store = ConversationStore::new(":memory:").unwrap();

        let previous = store.get_previous_response_id("thread-1").await.unwrap();

        assert_eq!(previous, None);
    }

    #[tokio::test]
    async fn upserts_and_reads_previous_response_id() {
        let store = ConversationStore::new(":memory:").unwrap();

        store.upsert_last_response_id("thread-1", "resp-1").await.unwrap();
        let previous = store.get_previous_response_id("thread-1").await.unwrap();

        assert_eq!(previous.as_deref(), Some("resp-1"));
    }

    #[tokio::test]
    async fn upsert_replaces_existing_response_id_for_thread() {
        let store = ConversationStore::new(":memory:").unwrap();

        store.upsert_last_response_id("thread-1", "resp-1").await.unwrap();
        store.upsert_last_response_id("thread-1", "resp-2").await.unwrap();
        let previous = store.get_previous_response_id("thread-1").await.unwrap();

        assert_eq!(previous.as_deref(), Some("resp-2"));
    }
}
