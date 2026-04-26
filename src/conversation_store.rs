use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task;

#[derive(Clone)]
pub struct ConversationStore {
    worker: DbWorker,
}

#[derive(Clone)]
struct DbWorker {
    sender: mpsc::Sender<DbCommand>,
}

enum DbCommand {
    GetPreviousResponseId {
        thread_key: String,
        reply: mpsc::Sender<Result<Option<String>>>,
    },
    UpsertLastResponseId {
        thread_key: String,
        response_id: String,
        updated_at: i64,
        reply: mpsc::Sender<Result<()>>,
    },
}

impl ConversationStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let worker = DbWorker::start(path.as_ref().to_path_buf())?;

        Ok(Self { worker })
    }

    pub async fn get_previous_response_id(&self, thread_key: &str) -> Result<Option<String>> {
        self.worker.get_previous_response_id(thread_key.to_string()).await
    }

    pub async fn upsert_last_response_id(&self, thread_key: &str, response_id: &str) -> Result<()> {
        let updated_at = unix_timestamp_seconds();
        self.worker
            .upsert_last_response_id(thread_key.to_string(), response_id.to_string(), updated_at)
            .await
    }
}

impl DbWorker {
    fn start(path: PathBuf) -> Result<Self> {
        let (command_sender, command_receiver) = mpsc::channel();
        let (init_sender, init_receiver) = mpsc::channel();

        thread::spawn(move || run_database_worker(path, command_receiver, init_sender));

        init_receiver.recv().context("ConversationStore init task failed")??;

        Ok(Self { sender: command_sender })
    }

    async fn get_previous_response_id(&self, thread_key: String) -> Result<Option<String>> {
        let sender = self.sender.clone();

        task::spawn_blocking(move || {
            let (reply_sender, reply_receiver) = mpsc::channel();
            sender
                .send(DbCommand::GetPreviousResponseId { thread_key, reply: reply_sender })
                .map_err(|_| anyhow!("ConversationStore database worker stopped"))?;

            reply_receiver.recv().context("ConversationStore database worker stopped")?
        })
        .await
        .context("ConversationStore get_previous_response_id task failed")?
    }

    async fn upsert_last_response_id(
        &self,
        thread_key: String,
        response_id: String,
        updated_at: i64,
    ) -> Result<()> {
        let sender = self.sender.clone();

        task::spawn_blocking(move || {
            let (reply_sender, reply_receiver) = mpsc::channel();
            sender
                .send(DbCommand::UpsertLastResponseId {
                    thread_key,
                    response_id,
                    updated_at,
                    reply: reply_sender,
                })
                .map_err(|_| anyhow!("ConversationStore database worker stopped"))?;

            reply_receiver.recv().context("ConversationStore database worker stopped")?
        })
        .await
        .context("ConversationStore upsert_last_response_id task failed")?
    }
}

fn run_database_worker(
    path: PathBuf,
    command_receiver: mpsc::Receiver<DbCommand>,
    init_sender: mpsc::Sender<Result<()>>,
) {
    let conn = match open_connection(&path) {
        Ok(conn) => {
            let _ = init_sender.send(Ok(()));
            conn
        }
        Err(e) => {
            let _ = init_sender.send(Err(e));
            return;
        }
    };

    for command in command_receiver {
        handle_db_command(&conn, command);
    }
}

fn open_connection(path: &Path) -> Result<Connection> {
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

    Ok(conn)
}

fn handle_db_command(conn: &Connection, command: DbCommand) {
    match command {
        DbCommand::GetPreviousResponseId { thread_key, reply } => {
            let result = query_previous_response_id(conn, &thread_key);
            let _ = reply.send(result);
        }
        DbCommand::UpsertLastResponseId { thread_key, response_id, updated_at, reply } => {
            let result = upsert_response_id(conn, &thread_key, &response_id, updated_at);
            let _ = reply.send(result);
        }
    }
}

fn query_previous_response_id(conn: &Connection, thread_key: &str) -> Result<Option<String>> {
    let mut stmt =
        conn.prepare("SELECT last_response_id FROM conversations WHERE thread_key = ?1")?;
    let mut rows = stmt.query(params![thread_key])?;
    if let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        Ok(Some(id))
    } else {
        Ok(None)
    }
}

fn upsert_response_id(
    conn: &Connection,
    thread_key: &str,
    response_id: &str,
    updated_at: i64,
) -> Result<()> {
    conn.execute(
        r#"
                INSERT INTO conversations (thread_key, last_response_id, updated_at)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(thread_key) DO UPDATE SET
                    last_response_id = excluded.last_response_id,
                    updated_at = excluded.updated_at
                "#,
        params![thread_key, response_id, updated_at],
    )?;
    Ok(())
}

fn unix_timestamp_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
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

    #[tokio::test]
    async fn cloned_store_reads_updates_from_original() {
        let store = ConversationStore::new(":memory:").unwrap();
        let cloned = store.clone();

        store.upsert_last_response_id("thread-1", "resp-1").await.unwrap();
        let previous = cloned.get_previous_response_id("thread-1").await.unwrap();

        assert_eq!(previous.as_deref(), Some("resp-1"));
    }
}
