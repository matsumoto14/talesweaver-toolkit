-- 差分投入(units.py --state)の下地。h 列を足すだけ。unit_fts の contentless_delete=1 化は
-- この migration ではなく、この後もう一度流す全件投入(DROP TABLE IF EXISTS unit_fts → CREATE)が行う。
-- npx wrangler d1 execute tw-wiki --remote --file migrations/004-diff-import.sql -y
ALTER TABLE page ADD COLUMN h TEXT;
ALTER TABLE unit ADD COLUMN h TEXT;
ALTER TABLE wiki_table ADD COLUMN h TEXT;
ALTER TABLE correction ADD COLUMN h TEXT;
