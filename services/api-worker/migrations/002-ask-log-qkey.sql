-- ask_log に qkey(答えのキャッシュのキー)を足す(2026-09-23)。001 を入れたあとに 1 回だけ。
-- npx wrangler d1 execute tw-wiki --remote -y --command "ALTER TABLE ask_log ADD COLUMN qkey TEXT"
ALTER TABLE ask_log ADD COLUMN qkey TEXT;
