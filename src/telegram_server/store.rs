//! SQLite-backed storage for bot data, messages, and update tracking.

use crate::telegram_server::error::BotApiError;
use crate::telegram_server::types::{BotInfo, BotUpdate};
use rusqlite::{params, Connection};
use std::sync::Mutex;

const CURRENT_SCHEMA_VERSION: i32 = 1;

/// Configuration for the bot store.
#[derive(Debug, Clone)]
pub struct BotStoreConfig {
    /// Path to the SQLite database file (`:memory:` for in-memory).
    pub db_path: String,
    /// Base directory for stored media files.
    pub media_dir: String,
}

/// SQLite-backed store for bot metadata, messages, and updates.
pub struct BotStore {
    conn: Mutex<Connection>,
    media_dir: String,
}

impl BotStore {
    /// Opens or creates the database at the configured path.
    pub fn open(config: &BotStoreConfig) -> Result<Self, BotApiError> {
        let conn = if config.db_path == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(&config.db_path)
        }
        .map_err(|e| BotApiError::DatabaseError(e.to_string()))?;

        let store = Self {
            conn: Mutex::new(conn),
            media_dir: config.media_dir.clone(),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            )",
        )?;

        let version: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;

        if version < CURRENT_SCHEMA_VERSION {
            conn.execute_batch(include_str!("schema.sql"))?;
            conn.execute(
                "INSERT OR REPLACE INTO schema_version (rowid, version) \
                 VALUES (1, ?1)",
                params![CURRENT_SCHEMA_VERSION],
            )?;
        }
        Ok(())
    }

    /// Creates a new bot and returns its info including the generated token.
    pub fn create_bot(
        &self,
        name: &str,
        username: &str,
    ) -> Result<BotInfo, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let token_hex = uuid::Uuid::new_v4().simple().to_string();

        // Insert with a placeholder token, then update with the real
        // one that includes the auto-generated id.
        conn.execute(
            "INSERT INTO bots (name, username, token) VALUES (?1, ?2, ?3)",
            params![name, username, "placeholder"],
        )
        .map_err(|e| {
            if let rusqlite::Error::SqliteFailure(_, Some(ref msg)) = e {
                if msg.contains("UNIQUE") {
                    return BotApiError::InvalidRequest(format!(
                        "Username '{username}' already taken"
                    ));
                }
            }
            BotApiError::from(e)
        })?;

        let id = conn.last_insert_rowid();
        let token = format!("{id}:{token_hex}");
        conn.execute(
            "UPDATE bots SET token = ?1 WHERE id = ?2",
            params![token, id],
        )?;

        Self::get_bot_by_id_inner(&conn, id)
    }

    /// Finds a bot by its API token.
    pub fn get_bot_by_token(
        &self,
        token: &str,
    ) -> Result<Option<BotInfo>, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let result = conn.query_row(
            "SELECT id, token, name, username, description, about_text, \
             photo_path, created_at FROM bots WHERE token = ?1",
            params![token],
            Self::row_to_bot_info,
        );
        match result {
            Ok(bot) => Ok(Some(bot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BotApiError::from(e)),
        }
    }

    /// Lists all bots ordered by creation time.
    pub fn list_bots(&self) -> Result<Vec<BotInfo>, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, token, name, username, description, about_text, \
             photo_path, created_at FROM bots ORDER BY created_at ASC",
        )?;
        let bots = stmt
            .query_map([], Self::row_to_bot_info)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(bots)
    }

    /// Updates selected properties of a bot.
    pub fn update_bot(
        &self,
        token: &str,
        update: &BotUpdate,
    ) -> Result<BotInfo, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let id: i64 = conn
            .query_row(
                "SELECT id FROM bots WHERE token = ?1",
                params![token],
                |row| row.get(0),
            )
            .map_err(|_| BotApiError::BotNotFound)?;

        if let Some(ref name) = update.name {
            conn.execute(
                "UPDATE bots SET name = ?1, updated_at = CURRENT_TIMESTAMP \
                 WHERE id = ?2",
                params![name, id],
            )?;
        }
        if let Some(ref desc) = update.description {
            conn.execute(
                "UPDATE bots SET description = ?1, \
                 updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![desc, id],
            )?;
        }
        if let Some(ref about) = update.about_text {
            conn.execute(
                "UPDATE bots SET about_text = ?1, \
                 updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![about, id],
            )?;
        }
        if let Some(ref photo) = update.photo_path {
            conn.execute(
                "UPDATE bots SET photo_path = ?1, \
                 updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![photo, id],
            )?;
        }

        Self::get_bot_by_id_inner(&conn, id)
    }

    /// Deletes a bot and all associated data (messages, updates).
    pub fn delete_bot(&self, token: &str) -> Result<(), BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let id: i64 = conn
            .query_row(
                "SELECT id FROM bots WHERE token = ?1",
                params![token],
                |row| row.get(0),
            )
            .map_err(|_| BotApiError::BotNotFound)?;

        conn.execute("DELETE FROM updates WHERE bot_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM messages WHERE bot_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM bots WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Revokes the current token and generates a new one.
    pub fn revoke_token(
        &self,
        old_token: &str,
    ) -> Result<String, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let id: i64 = conn
            .query_row(
                "SELECT id FROM bots WHERE token = ?1",
                params![old_token],
                |row| row.get(0),
            )
            .map_err(|_| BotApiError::BotNotFound)?;

        let new_hex = uuid::Uuid::new_v4().simple().to_string();
        let new_token = format!("{id}:{new_hex}");
        conn.execute(
            "UPDATE bots SET token = ?1, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?2",
            params![new_token, id],
        )?;
        Ok(new_token)
    }

    /// Inserts an update record and returns the assigned `update_id`.
    pub fn insert_update(
        &self,
        bot_id: i64,
        payload_json: &str,
    ) -> Result<i64, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        conn.execute(
            "INSERT INTO updates (bot_id, payload) VALUES (?1, ?2)",
            params![bot_id, payload_json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Gets pending updates with `update_id >= offset`.
    pub fn get_updates(
        &self,
        bot_id: i64,
        offset: i64,
        limit: u32,
    ) -> Result<Vec<(i64, String)>, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, payload FROM updates \
             WHERE bot_id = ?1 AND id >= ?2 \
             ORDER BY id ASC LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![bot_id, offset, limit], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Stores a message and returns the per-bot `message_id`.
    pub fn store_message(
        &self,
        bot_id: i64,
        chat_id: i64,
        is_from_bot: bool,
        content: Option<&str>,
        media_type: Option<&str>,
        media_path: Option<&str>,
        caption: Option<&str>,
        reply_markup_json: Option<&str>,
    ) -> Result<i64, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let next_msg_id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(message_id), 0) + 1 FROM messages \
             WHERE bot_id = ?1",
            params![bot_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO messages (bot_id, message_id, chat_id, is_from_bot, \
             content, media_type, media_path, caption, reply_markup) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                bot_id,
                next_msg_id,
                chat_id,
                is_from_bot,
                content,
                media_type,
                media_path,
                caption,
                reply_markup_json,
            ],
        )?;
        Ok(next_msg_id)
    }

    /// Returns the configured media directory path.
    pub fn media_dir(&self) -> &str {
        &self.media_dir
    }

    /// Stores file metadata for later retrieval via `getFile`.
    pub fn store_media(
        &self,
        file_id: &str,
        file_unique_id: &str,
        file_path: &str,
        mime_type: Option<&str>,
        file_size: Option<i64>,
    ) -> Result<(), BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO media \
             (file_id, file_unique_id, file_path, mime_type, file_size) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_id, file_unique_id, file_path, mime_type, file_size],
        )?;
        Ok(())
    }

    /// Gets media metadata by `file_id`.
    /// Returns `(file_path, mime_type)` if found.
    pub fn get_media(
        &self,
        file_id: &str,
    ) -> Result<Option<(String, Option<String>)>, BotApiError> {
        let conn = self.conn.lock().expect("store lock poisoned");
        let result = conn.query_row(
            "SELECT file_path, mime_type FROM media WHERE file_id = ?1",
            params![file_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        );
        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BotApiError::from(e)),
        }
    }

    fn get_bot_by_id_inner(
        conn: &Connection,
        id: i64,
    ) -> Result<BotInfo, BotApiError> {
        conn.query_row(
            "SELECT id, token, name, username, description, about_text, \
             photo_path, created_at FROM bots WHERE id = ?1",
            params![id],
            Self::row_to_bot_info,
        )
        .map_err(|_| BotApiError::BotNotFound)
    }

    fn row_to_bot_info(
        row: &rusqlite::Row<'_>,
    ) -> Result<BotInfo, rusqlite::Error> {
        Ok(BotInfo {
            id: row.get(0)?,
            token: row.get(1)?,
            name: row.get(2)?,
            username: row.get(3)?,
            description: row.get::<_, String>(4)?,
            about_text: row.get::<_, String>(5)?,
            photo_path: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> BotStore {
        BotStore::open(&BotStoreConfig {
            db_path: ":memory:".into(),
            media_dir: String::new(),
        })
        .expect("open in-memory db")
    }

    #[test]
    fn test_create_and_get_bot() {
        let store = test_store();
        let bot = store.create_bot("TestBot", "test_bot").unwrap();
        assert_eq!(bot.name, "TestBot");
        assert_eq!(bot.username, "test_bot");
        assert!(bot.token.contains(':'));

        let found =
            store.get_bot_by_token(&bot.token).unwrap().unwrap();
        assert_eq!(found.id, bot.id);
    }

    #[test]
    fn test_duplicate_username_rejected() {
        let store = test_store();
        store.create_bot("Bot1", "same_bot").unwrap();
        let err = store.create_bot("Bot2", "same_bot").unwrap_err();
        assert!(matches!(err, BotApiError::InvalidRequest(_)));
    }

    #[test]
    fn test_list_bots() {
        let store = test_store();
        store.create_bot("A", "a_bot").unwrap();
        store.create_bot("B", "b_bot").unwrap();
        store.create_bot("C", "c_bot").unwrap();
        assert_eq!(store.list_bots().unwrap().len(), 3);
    }

    #[test]
    fn test_update_bot() {
        let store = test_store();
        let bot = store.create_bot("Old", "old_bot").unwrap();
        store
            .update_bot(
                &bot.token,
                &BotUpdate {
                    name: Some("New".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let updated =
            store.get_bot_by_token(&bot.token).unwrap().unwrap();
        assert_eq!(updated.name, "New");
    }

    #[test]
    fn test_delete_bot_cascades() {
        let store = test_store();
        let bot = store.create_bot("Del", "del_bot").unwrap();
        store
            .store_message(
                bot.id, 1, false, Some("hi"), None, None, None, None,
            )
            .unwrap();
        store.insert_update(bot.id, "{}").unwrap();
        store.delete_bot(&bot.token).unwrap();
        assert!(store.get_bot_by_token(&bot.token).unwrap().is_none());
    }

    #[test]
    fn test_revoke_token() {
        let store = test_store();
        let bot = store.create_bot("Rev", "rev_bot").unwrap();
        let new_token = store.revoke_token(&bot.token).unwrap();
        assert_ne!(new_token, bot.token);
        assert!(store.get_bot_by_token(&bot.token).unwrap().is_none());
        assert!(store.get_bot_by_token(&new_token).unwrap().is_some());
    }

    #[test]
    fn test_update_id_global_increment() {
        let store = test_store();
        let a = store.create_bot("A", "aa_bot").unwrap();
        let b = store.create_bot("B", "bb_bot").unwrap();
        let id1 = store.insert_update(a.id, "{}").unwrap();
        let id2 = store.insert_update(b.id, "{}").unwrap();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_get_updates_with_offset() {
        let store = test_store();
        let bot = store.create_bot("U", "u_bot").unwrap();
        let _id1 = store.insert_update(bot.id, r#"{"a":1}"#).unwrap();
        let id2 = store.insert_update(bot.id, r#"{"a":2}"#).unwrap();
        let _id3 = store.insert_update(bot.id, r#"{"a":3}"#).unwrap();

        let updates = store.get_updates(bot.id, id2, 100).unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].0, id2);
    }

    #[test]
    fn test_store_message_auto_increment() {
        let store = test_store();
        let bot = store.create_bot("M", "m_bot").unwrap();
        let id1 = store
            .store_message(
                bot.id, 1, true, Some("hello"), None, None, None, None,
            )
            .unwrap();
        let id2 = store
            .store_message(
                bot.id, 1, false, Some("world"), None, None, None, None,
            )
            .unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_store_and_get_media() {
        let store = test_store();
        store
            .store_media("f1", "fu1", "/path/file", Some("image/png"), Some(1024))
            .unwrap();
        let (path, mime) = store.get_media("f1").unwrap().unwrap();
        assert_eq!(path, "/path/file");
        assert_eq!(mime.as_deref(), Some("image/png"));
    }
}
