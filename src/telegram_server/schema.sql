CREATE TABLE IF NOT EXISTS bots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    about_text  TEXT NOT NULL DEFAULT '',
    photo_path  TEXT,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS updates (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    bot_id     INTEGER NOT NULL REFERENCES bots(id),
    payload    TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    bot_id       INTEGER NOT NULL REFERENCES bots(id),
    message_id   INTEGER NOT NULL,
    chat_id      INTEGER NOT NULL,
    is_from_bot  INTEGER NOT NULL DEFAULT 0,
    content      TEXT,
    media_type   TEXT,
    media_path   TEXT,
    caption      TEXT,
    reply_markup TEXT,
    timestamp    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(bot_id, message_id)
);

CREATE TABLE IF NOT EXISTS media (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id        TEXT NOT NULL UNIQUE,
    file_unique_id TEXT NOT NULL,
    file_path      TEXT NOT NULL,
    mime_type      TEXT,
    file_size      INTEGER
);

CREATE INDEX IF NOT EXISTS idx_updates_bot ON updates(bot_id);
CREATE INDEX IF NOT EXISTS idx_messages_bot_chat ON messages(bot_id, chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
