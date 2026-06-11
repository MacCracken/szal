//! Durable [`ExecutionStore`] backends backed by [`sqlx`].
//!
//! Enables crash-recoverable, queryable execution tracking against SQLite or
//! PostgreSQL. Each backend is feature-gated so the lean default build pulls no
//! database dependencies:
//!
//! - `sqlite` → [`SqliteExecutionStore`]
//! - `postgres` → [`PostgresExecutionStore`]
//!
//! Both expose an async, DB-authoritative API ([`connect`](SqliteExecutionStore::connect),
//! [`migrate`](SqliteExecutionStore::migrate), `save`/`get`/`list`/`remove`) for
//! dashboards and recovery, plus [`engine_sink`](SqliteExecutionStore::engine_sink)
//! which adapts the store to the synchronous
//! [`ExecutionStore`](crate::storage::ExecutionStore) the engine consumes.
//!
//! Records are stored as JSON in a single `szal_executions` table, keyed by
//! execution id and indexed by flow name:
//!
//! ```sql
//! CREATE TABLE szal_executions (
//!     execution_id TEXT PRIMARY KEY,
//!     flow_name    TEXT NOT NULL,
//!     data         TEXT NOT NULL  -- serialized ExecutionRecord
//! );
//! ```
//!
//! ## Engine integration semantics
//!
//! [`engine_sink`](SqliteExecutionStore::engine_sink) returns a sink whose `save`
//! is **fire-and-forget**: the record is mirrored in memory immediately (so the
//! sink's synchronous reads are consistent within this process) and the durable
//! DB write is spawned onto the Tokio runtime. For authoritative cross-process
//! reads, query the async store directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once, RwLock};

use tokio::sync::mpsc;

use crate::storage::{ExecutionRecord, ExecutionStore};

#[inline]
fn sqlx_err(e: sqlx::Error) -> crate::SzalError {
    crate::SzalError::Other(anyhow::Error::new(e))
}

#[inline]
fn json_err(e: serde_json::Error) -> crate::SzalError {
    crate::SzalError::Other(anyhow::Error::new(e))
}

/// Internal: a store that can durably persist/delete records asynchronously.
///
/// Lets the synchronous [`SpawnSink`] bridge stay generic over the concrete
/// backend without dynamic dispatch on a futures-returning trait.
trait DurableSave: Clone + Send + Sync + 'static {
    fn save_record(&self, record: ExecutionRecord) -> impl std::future::Future<Output = ()> + Send;
    fn delete_record(&self, id: String) -> impl std::future::Future<Output = ()> + Send;
}

/// An ordered durable write applied by the background writer.
enum WriteOp {
    Save(ExecutionRecord),
    Delete(String),
}

/// Synchronous [`ExecutionStore`] bridge over an async sqlx store.
///
/// Writes are mirrored in memory (so the sink's reads are immediately consistent
/// within this process) and forwarded over an ordered channel to a single
/// background task that applies them to the database **in submission order**.
/// Ordering matters: a flow's `Running` save must not overwrite its later
/// `Completed` save. Reads are served from the in-process mirror.
struct SpawnSink<S: DurableSave> {
    store: S,
    mirror: RwLock<HashMap<String, ExecutionRecord>>,
    tx: mpsc::UnboundedSender<WriteOp>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<WriteOp>>>,
    writer_started: Once,
}

impl<S: DurableSave> SpawnSink<S> {
    fn new(store: S) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            store,
            mirror: RwLock::new(HashMap::new()),
            tx,
            rx: Mutex::new(Some(rx)),
            writer_started: Once::new(),
        }
    }

    /// Lazily start the single background writer on first use. Spawning here
    /// (rather than in `new`) means it always lands inside the caller's Tokio
    /// runtime — the engine calls `save` from within `run`.
    fn ensure_writer(&self) {
        self.writer_started.call_once(|| {
            let Some(mut rx) = self
                .rx
                .lock()
                .expect("execution sink writer lock poisoned")
                .take()
            else {
                return;
            };
            let store = self.store.clone();
            tokio::spawn(async move {
                while let Some(op) = rx.recv().await {
                    match op {
                        WriteOp::Save(record) => store.save_record(record).await,
                        WriteOp::Delete(id) => store.delete_record(id).await,
                    }
                }
            });
        });
    }
}

impl<S: DurableSave> ExecutionStore for SpawnSink<S> {
    fn save(&self, record: ExecutionRecord) {
        self.ensure_writer();
        self.mirror
            .write()
            .expect("execution sink mirror poisoned")
            .insert(record.execution_id.clone(), record.clone());
        let _ = self.tx.send(WriteOp::Save(record));
    }

    fn get(&self, execution_id: &str) -> Option<ExecutionRecord> {
        self.mirror
            .read()
            .expect("execution sink mirror poisoned")
            .get(execution_id)
            .cloned()
    }

    fn list(&self, flow_name: Option<&str>) -> Vec<String> {
        self.mirror
            .read()
            .expect("execution sink mirror poisoned")
            .values()
            .filter(|r| flow_name.is_none_or(|n| r.flow_name == n))
            .map(|r| r.execution_id.clone())
            .collect()
    }

    fn remove(&self, execution_id: &str) -> Option<ExecutionRecord> {
        self.ensure_writer();
        let removed = self
            .mirror
            .write()
            .expect("execution sink mirror poisoned")
            .remove(execution_id);
        if removed.is_some() {
            let _ = self.tx.send(WriteOp::Delete(execution_id.to_owned()));
        }
        removed
    }
}

/// Generate a sqlx-backed execution store for a specific backend.
///
/// The SQL is identical across backends except for bind-parameter placeholders
/// (`?` for SQLite, `$N` for PostgreSQL), so those are supplied per invocation.
macro_rules! sqlx_execution_store {
    (
        $(#[$doc:meta])*
        store: $name:ident,
        pool: $pool:ty,
        connect: $connect:path,
        upsert: $upsert:literal,
        select: $select:literal,
        list_flow: $list_flow:literal,
        delete: $delete:literal $(,)?
    ) => {
        $(#[$doc])*
        #[derive(Clone)]
        pub struct $name {
            pool: $pool,
        }

        impl $name {
            /// Connect to `url` and build a connection pool.
            ///
            /// Does not create the schema — call [`migrate`](Self::migrate) once.
            pub async fn connect(url: &str) -> crate::Result<Self> {
                let pool = $connect(url).await.map_err(sqlx_err)?;
                Ok(Self { pool })
            }

            /// Build a store from an existing sqlx pool.
            #[must_use]
            pub fn from_pool(pool: $pool) -> Self {
                Self { pool }
            }

            /// The underlying sqlx connection pool.
            #[must_use]
            pub fn pool(&self) -> &$pool {
                &self.pool
            }

            /// Create the `szal_executions` table if it does not yet exist.
            pub async fn migrate(&self) -> crate::Result<()> {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS szal_executions (\
                     execution_id TEXT PRIMARY KEY, \
                     flow_name TEXT NOT NULL, \
                     data TEXT NOT NULL)",
                )
                .execute(&self.pool)
                .await
                .map_err(sqlx_err)?;
                Ok(())
            }

            /// Persist (insert or update) an execution record.
            pub async fn save(&self, record: &ExecutionRecord) -> crate::Result<()> {
                let data = serde_json::to_string(record).map_err(json_err)?;
                sqlx::query($upsert)
                    .bind(&record.execution_id)
                    .bind(&record.flow_name)
                    .bind(&data)
                    .execute(&self.pool)
                    .await
                    .map_err(sqlx_err)?;
                Ok(())
            }

            /// Load an execution record by id.
            pub async fn get(&self, execution_id: &str) -> crate::Result<Option<ExecutionRecord>> {
                use sqlx::Row;
                let row = sqlx::query($select)
                    .bind(execution_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(sqlx_err)?;
                match row {
                    Some(row) => {
                        let data: String = row.try_get("data").map_err(sqlx_err)?;
                        Ok(Some(serde_json::from_str(&data).map_err(json_err)?))
                    }
                    None => Ok(None),
                }
            }

            /// List execution ids, optionally filtered by flow name.
            pub async fn list(&self, flow_name: Option<&str>) -> crate::Result<Vec<String>> {
                use sqlx::Row;
                let rows = match flow_name {
                    Some(name) => sqlx::query($list_flow)
                        .bind(name)
                        .fetch_all(&self.pool)
                        .await,
                    None => sqlx::query("SELECT execution_id FROM szal_executions")
                        .fetch_all(&self.pool)
                        .await,
                }
                .map_err(sqlx_err)?;
                rows.iter()
                    .map(|r| r.try_get::<String, _>("execution_id").map_err(sqlx_err))
                    .collect()
            }

            /// Remove an execution record, returning it if it existed.
            pub async fn remove(
                &self,
                execution_id: &str,
            ) -> crate::Result<Option<ExecutionRecord>> {
                let existing = self.get(execution_id).await?;
                if existing.is_some() {
                    sqlx::query($delete)
                        .bind(execution_id)
                        .execute(&self.pool)
                        .await
                        .map_err(sqlx_err)?;
                }
                Ok(existing)
            }

            /// Adapt this store to the synchronous
            /// [`ExecutionStore`](crate::storage::ExecutionStore) the engine consumes.
            ///
            /// See the [module docs](crate::sql_store) for the fire-and-forget save
            /// semantics.
            #[must_use]
            pub fn engine_sink(&self) -> Arc<dyn ExecutionStore> {
                Arc::new(SpawnSink::new(self.clone()))
            }
        }

        impl DurableSave for $name {
            async fn save_record(&self, record: ExecutionRecord) {
                if let Err(e) = self.save(&record).await {
                    tracing::error!(
                        execution_id = %record.execution_id,
                        error = %e,
                        "failed to persist execution record"
                    );
                }
            }
            async fn delete_record(&self, id: String) {
                if let Err(e) = self.remove(&id).await {
                    tracing::error!(execution_id = %id, error = %e, "failed to delete execution record");
                }
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}

#[cfg(feature = "sqlite")]
sqlx_execution_store! {
    /// Durable [`ExecutionStore`](crate::storage::ExecutionStore) backed by SQLite.
    ///
    /// ```no_run
    /// use szal::sql_store::SqliteExecutionStore;
    ///
    /// # async fn demo() -> szal::Result<()> {
    /// let store = SqliteExecutionStore::connect("sqlite://executions.db?mode=rwc").await?;
    /// store.migrate().await?;
    /// let ids = store.list(None).await?;
    /// # let _ = ids;
    /// # Ok(())
    /// # }
    /// ```
    store: SqliteExecutionStore,
    pool: sqlx::SqlitePool,
    connect: sqlx::SqlitePool::connect,
    upsert: "INSERT INTO szal_executions (execution_id, flow_name, data) VALUES (?, ?, ?) \
             ON CONFLICT(execution_id) DO UPDATE SET flow_name = excluded.flow_name, data = excluded.data",
    select: "SELECT data FROM szal_executions WHERE execution_id = ?",
    list_flow: "SELECT execution_id FROM szal_executions WHERE flow_name = ?",
    delete: "DELETE FROM szal_executions WHERE execution_id = ?",
}

#[cfg(feature = "postgres")]
sqlx_execution_store! {
    /// Durable [`ExecutionStore`](crate::storage::ExecutionStore) backed by PostgreSQL.
    ///
    /// ```no_run
    /// use szal::sql_store::PostgresExecutionStore;
    ///
    /// # async fn demo() -> szal::Result<()> {
    /// let store = PostgresExecutionStore::connect("postgres://user:pass@localhost/db").await?;
    /// store.migrate().await?;
    /// # Ok(())
    /// # }
    /// ```
    store: PostgresExecutionStore,
    pool: sqlx::PgPool,
    connect: sqlx::PgPool::connect,
    upsert: "INSERT INTO szal_executions (execution_id, flow_name, data) VALUES ($1, $2, $3) \
             ON CONFLICT(execution_id) DO UPDATE SET flow_name = excluded.flow_name, data = excluded.data",
    select: "SELECT data FROM szal_executions WHERE execution_id = $1",
    list_flow: "SELECT execution_id FROM szal_executions WHERE flow_name = $1",
    delete: "DELETE FROM szal_executions WHERE execution_id = $1",
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::engine::{Engine, EngineConfig, handler_fn};
    use crate::flow::{FlowDef, FlowMode};
    use crate::state::WorkflowState;
    use crate::step::StepDef;

    async fn temp_store() -> (SqliteExecutionStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exec.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let store = SqliteExecutionStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        (store, dir)
    }

    fn record(id: &str, flow: &str, state: WorkflowState) -> ExecutionRecord {
        ExecutionRecord {
            execution_id: id.into(),
            flow_name: flow.into(),
            state,
            result: None,
            started_at: "2026-06-10T00:00:00Z".into(),
            finished_at: None,
        }
    }

    #[tokio::test]
    async fn save_get_roundtrip() {
        let (store, _dir) = temp_store().await;
        store
            .save(&record("e1", "deploy", WorkflowState::Running))
            .await
            .unwrap();
        let got = store.get("e1").await.unwrap().unwrap();
        assert_eq!(got.flow_name, "deploy");
        assert_eq!(got.state, WorkflowState::Running);
        assert!(store.get("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        let (store, _dir) = temp_store().await;
        store
            .save(&record("e1", "deploy", WorkflowState::Running))
            .await
            .unwrap();
        store
            .save(&record("e1", "deploy", WorkflowState::Completed))
            .await
            .unwrap();
        let got = store.get("e1").await.unwrap().unwrap();
        assert_eq!(got.state, WorkflowState::Completed);
        // Still a single row.
        assert_eq!(store.list(None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_filters_by_flow() {
        let (store, _dir) = temp_store().await;
        store
            .save(&record("e1", "deploy", WorkflowState::Completed))
            .await
            .unwrap();
        store
            .save(&record("e2", "test", WorkflowState::Failed))
            .await
            .unwrap();
        store
            .save(&record("e3", "deploy", WorkflowState::RolledBack))
            .await
            .unwrap();
        assert_eq!(store.list(None).await.unwrap().len(), 3);
        assert_eq!(store.list(Some("deploy")).await.unwrap().len(), 2);
        assert_eq!(store.list(Some("test")).await.unwrap().len(), 1);
        assert_eq!(store.list(Some("missing")).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn remove_returns_and_deletes() {
        let (store, _dir) = temp_store().await;
        store
            .save(&record("e1", "x", WorkflowState::Created))
            .await
            .unwrap();
        let removed = store.remove("e1").await.unwrap().unwrap();
        assert_eq!(removed.execution_id, "e1");
        assert!(store.get("e1").await.unwrap().is_none());
        assert!(store.remove("e1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn engine_sink_persists_execution() {
        let (store, _dir) = temp_store().await;
        let sink = store.engine_sink();

        let mut flow = FlowDef::new("persist", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        let exec_id = flow.id.to_string();

        let engine = Engine::new(
            EngineConfig {
                execution_store: Some(sink),
                ..Default::default()
            },
            handler_fn(|s| async move { Ok(serde_json::json!({"step": s.name})) }),
        );
        assert!(engine.run(&flow).await.unwrap().success);

        // The final fire-and-forget DB write may still be in flight; poll briefly.
        let mut found = None;
        for _ in 0..50 {
            if let Some(r) = store.get(&exec_id).await.unwrap()
                && r.state == WorkflowState::Completed
            {
                found = Some(r);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let rec = found.expect("execution not persisted to sqlite");
        assert_eq!(rec.flow_name, "persist");
        assert!(rec.finished_at.is_some());
    }
}
