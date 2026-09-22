-- schema.sql を投入済みの D1 に ask_log を足す(2026-09-23)。
-- npx wrangler d1 execute tw-wiki --remote --file migrations/001-ask-log.sql -y
CREATE TABLE IF NOT EXISTS ask_log (
  id         INTEGER PRIMARY KEY,
  at         TEXT NOT NULL,
  user       TEXT NOT NULL,
  question   TEXT NOT NULL,
  prev_page  TEXT,
  state      TEXT NOT NULL,
  kind       TEXT NOT NULL,
  reason     TEXT,
  route      TEXT,
  answer_id  TEXT,
  lead       TEXT,
  body       TEXT NOT NULL,
  ms         INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ask_log_user ON ask_log(user, at);
