-- 管理画面(admin.tw-context.dev)用のトレース。002 を入れたあとに 1 回だけ。
-- npx wrangler d1 execute tw-wiki --remote -y --file migrations/003-ask-trace.sql
ALTER TABLE ask_log ADD COLUMN understanding TEXT;
ALTER TABLE ask_log ADD COLUMN candidates TEXT;

CREATE TABLE ask_call (               -- /ask 1 回につき、LLM への往復ごとに 1 行(理解・選択・回す道)
  id                    INTEGER PRIMARY KEY,
  ask_log_id            INTEGER NOT NULL,
  seq                   INTEGER NOT NULL,  -- ask_log 内の呼び出し順
  kind                  TEXT NOT NULL,     -- understand | select | loop
  model                 TEXT NOT NULL,
  input_tokens          INTEGER NOT NULL,
  cache_read_tokens     INTEGER NOT NULL,
  cache_creation_tokens INTEGER NOT NULL,
  output_tokens         INTEGER NOT NULL,
  ms                    INTEGER NOT NULL
);
CREATE INDEX ask_call_ask_log_id ON ask_call(ask_log_id);
