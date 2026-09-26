-- unit_link にリンク先の節アンカーを足す(get_section でリンク先の節を丸ごと辿るための下地)。
-- 既存行は anchor 無し扱い("" )。次の units.py の投入(全件・差分どちらでも)で正しい値に埋まる。
-- npx wrangler d1 execute tw-wiki --remote --file migrations/005-unit-link-anchor.sql -y
-- npx wrangler d1 execute tw-wiki --local  --file migrations/005-unit-link-anchor.sql -y
ALTER TABLE unit_link ADD COLUMN anchor TEXT NOT NULL DEFAULT '';
