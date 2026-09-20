"""取込パイプラインのテスト。ネットワークには出ない。

    python -m unittest discover -s tools/gamedata/wiki -t tools/gamedata/wiki
"""
from __future__ import annotations

import sys
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from store import Store  # noqa: E402
from sync import plan_targets  # noqa: E402
from talewiki import (  # noqa: E402
    PageMissing, decode_euc_jp_with_nec, parse_list, parse_recent_changes, parse_source,
    quote_page, unquote_page,
)

JST = timezone(timedelta(hours=9))


def nec(n: int) -> bytes:
    """丸数字 ①=1 … を EUC-JP の NEC 拡張(0xAD 区)のバイト列にする。"""
    trail = 0x40 + (n - 1)  # cp932 0x8740 = ①
    cell = trail - 0x1F
    return bytes([0xAD, 0x80 + cell])


class Decode(unittest.TestCase):
    def test_nec_circled_digits_survive(self):
        raw = "レイヤー".encode("euc_jp") + nec(1) + "と".encode("euc_jp") + nec(2)
        self.assertEqual(decode_euc_jp_with_nec(raw), "レイヤー①と②")

    def test_plain_euc_jp_codec_would_lose_them(self):
        """素の euc_jp では読めないことの確認(この復元がある理由)。"""
        raw = "あ".encode("euc_jp") + nec(1) + "い".encode("euc_jp")
        self.assertNotIn("①", raw.decode("euc_jp", errors="replace"))
        self.assertEqual(decode_euc_jp_with_nec(raw), "あ①い")

    def test_half_width_kana_and_jisx0212(self):
        raw = "ｱ".encode("euc_jp") + "全角".encode("euc_jp")
        self.assertEqual(decode_euc_jp_with_nec(raw), "ｱ全角")

    def test_ascii_and_newlines_untouched(self):
        raw = b"|Lv|HP|\n|1|100|\n"
        self.assertEqual(decode_euc_jp_with_nec(raw), "|Lv|HP|\n|1|100|\n")

    def test_plus_in_href_is_a_space(self):
        self.assertEqual(unquote_page("Chapter/Secret+Chapter"), "Chapter/Secret Chapter")
        self.assertEqual(unquote_page("A%2BB"), "A+B")

    def test_page_name_round_trip(self):
        for name in ["ステータス", "Skill/ボリス", "Item/アクセサリー用装備/アーティファクト", "Chapter/Secret Chapter", "A+B"]:
            self.assertEqual(unquote_page(quote_page(name)), name)


class ParseSource(unittest.TestCase):
    def test_extracts_pre_and_unescapes(self):
        html = '<html><pre class="x">|a|&amp;b|\n**見出し</pre></html>'
        self.assertEqual(parse_source(html, "X"), "|a|&b|\n**見出し")

    def test_missing_pre_raises(self):
        with self.assertRaises(PageMissing):
            parse_source("<html>そんなページはありません</html>", "X")


class ParseList(unittest.TestCase):
    HTML = (
        '<div id="navigator"><a href="./?cmd=list">一覧</a>'
        '<a href="./?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9">ナビの中は数えない</a></div>'
        '<div id="body"><ul>'
        '<li><a href="./?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9">ステータス</a></li>'
        '<li><a href="./?Skill/%A5%DC%A5%EA%A5%B9">Skill/ボリス</a></li>'
        '<li><a href="./?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9">重複</a></li>'
        '<li><a href="./?cmd=rss&amp;ver=1.0">RSS</a></li>'
        '</ul></div>'
    )

    def test_body_only_deduped_and_no_query_links(self):
        self.assertEqual(parse_list(self.HTML), ["ステータス", "Skill/ボリス"])


class ParseRecentChanges(unittest.TestCase):
    SRC = (
        "#norelated\n"
        '-2026-09-20 (日) 10:06:53 - [ &pageaction("Skill/共通",diff); | '
        '&pageaction("Skill/共通",backup); ] [[Skill/共通]]\n'
        '-2026-09-19 (土) 05:28:55 - [ ] [[装備システム/継承]]\n'
        "これは行の形が違うので無視される\n"
    )

    def test_name_and_jst_timestamp(self):
        got = parse_recent_changes(self.SRC)
        self.assertEqual(
            got,
            [
                ("Skill/共通", datetime(2026, 9, 20, 10, 6, 53, tzinfo=JST)),
                ("装備システム/継承", datetime(2026, 9, 19, 5, 28, 55, tzinfo=JST)),
            ],
        )

    def test_iso_compares_lexically_for_freshness(self):
        """mtime の新旧判定は文字列比較でやっているので、その前提を固定する。"""
        a, b = (dt.isoformat() for _, dt in parse_recent_changes(self.SRC))
        self.assertGreater(a, b)


class StoreBehaviour(unittest.TestCase):
    def setUp(self):
        self.s = Store(Path(":memory:"))

    def tearDown(self):
        self.s.close()

    def test_needs_fetch_when_unknown_or_newer(self):
        self.assertTrue(self.s.needs_fetch("X", None))
        self.s.save("X", "本文", "2026-09-01T00:00:00+09:00")
        self.assertFalse(self.s.needs_fetch("X", None))
        self.assertFalse(self.s.needs_fetch("X", "2026-09-01T00:00:00+09:00"))
        self.assertTrue(self.s.needs_fetch("X", "2026-09-02T00:00:00+09:00"))

    def test_failure_keeps_last_good_source(self):
        self.s.save("X", "本文", "2026-09-01T00:00:00+09:00")
        self.s.fail("X", "timeout")
        row = self.s.db.execute("SELECT * FROM page WHERE name='X'").fetchone()
        self.assertEqual(row["source"], "本文")
        self.assertEqual(row["status"], "error")
        self.assertEqual(row["error"], "timeout")
        self.assertTrue(self.s.needs_fetch("X", "2026-09-02T00:00:00+09:00"))

    def test_seen_does_not_clobber_fetched_page(self):
        self.s.save("X", "本文", "2026-09-01T00:00:00+09:00")
        self.s.seen("X", "2026-09-05T00:00:00+09:00")
        row = self.s.db.execute("SELECT * FROM page WHERE name='X'").fetchone()
        self.assertEqual(row["status"], "ok")
        self.assertEqual(row["source"], "本文")
        self.assertEqual(row["mtime"], "2026-09-05T00:00:00+09:00")

    def test_unlisted_is_not_yet_gone(self):
        """一覧に無いだけでは gone にしない(`FrontPage` は ?cmd=list に載らない)。"""
        self.s.save("FrontPage", "本文", None)
        self.s.save("残る", "本文", None)
        self.assertEqual(self.s.unlisted({"残る"}), ["FrontPage"])
        self.assertEqual(self.s.counts(), {"ok": 2})  # 取得で確かめるまで ok のまま

    def test_mark_gone_then_relisted_comes_back(self):
        self.s.save("消える", "本文", None)
        self.s.mark_gone("消える")
        self.assertEqual(self.s.counts(), {"gone": 1})
        self.s.seen("消える", None)  # 一覧に戻ってきた(改名の戻し等)
        row = self.s.db.execute("SELECT * FROM page WHERE name='消える'").fetchone()
        self.assertEqual(row["status"], "new")
        self.assertEqual(row["source"], "本文")

    def test_relisted_page_is_refetched_even_without_a_new_mtime(self):
        """gone から一覧に戻ったページは古い source を持つので、mtime 無しでも取り直す。"""
        self.s.save("戻る", "古い本文", "2024-01-01T00:00:00+09:00")
        self.s.mark_gone("戻る")
        self.s.seen("戻る", None)  # 一覧に戻ってきた。source は古いまま残っている
        self.assertEqual(self.s.status("戻る"), "new")
        self.assertTrue(self.s.needs_fetch("戻る", None))

    def test_full_retries_a_listed_missing_page(self):
        """一覧に載っている = 実在するので、missing でも --full では取り直す。"""
        self.s.fail("実在するのに失敗", "<pre> が無い", status="missing",
                    mtime="2026-09-01T00:00:00+09:00")
        self.assertFalse(self.s.needs_fetch("実在するのに失敗", "2026-09-01T00:00:00+09:00"))
        self.assertTrue(self.s.needs_fetch("実在するのに失敗", "2026-09-01T00:00:00+09:00",
                                           retry_missing=True))

    def test_unfinished_covers_interrupted_and_transient_failures(self):
        self.s.seen("未取得", None)                    # --full を途中で止めた
        self.s.save("落ちた", "本文", None)
        self.s.fail("落ちた", "timeout")               # ok だったが通信で落ちた
        self.s.save("無事", "本文", None)
        self.s.fail("壊れた名前", "<pre> が無い", status="missing", mtime="2024-01-01T00:00:00+09:00")
        self.assertEqual(sorted(self.s.unfinished()), ["未取得", "落ちた"])

    def test_missing_is_not_retried_until_mtime_moves(self):
        """wiki 側が壊れた名前で記録した行(`???á?ó?È/1LineBBS`)を毎回取りに行かない。"""
        mt = "2024-11-15T08:54:19+09:00"
        self.s.fail("壊れた名前", "<pre> が無い", status="missing", mtime=mt)
        self.assertFalse(self.s.needs_fetch("壊れた名前", mt))
        self.assertTrue(self.s.needs_fetch("壊れた名前", "2026-01-01T00:00:00+09:00"))

    def test_gone_is_not_retried_from_recent_changes(self):
        """消えたページは RecentChanges に残っていても取りに行かない。"""
        mt = "2024-11-15T08:54:19+09:00"
        self.s.fail("壊れた名前", "<pre> が無い", status="missing", mtime=mt)
        self.s.mark_gone("壊れた名前")
        self.assertFalse(self.s.needs_fetch("壊れた名前", mt))

    def test_error_is_retried(self):
        """一時的な失敗(通信)は次回また取る。"""
        self.s.fail("落ちた", "timeout", mtime="2026-09-01T00:00:00+09:00")
        self.assertTrue(self.s.needs_fetch("落ちた", "2026-09-01T00:00:00+09:00"))

    def test_new_page_is_always_fetched(self):
        self.s.seen("未取得", "2026-09-01T00:00:00+09:00")
        self.assertTrue(self.s.needs_fetch("未取得", "2026-09-01T00:00:00+09:00"))

    def test_status_of_unknown_page(self):
        self.assertIsNone(self.s.status("知らない"))

    def test_run_is_recorded(self):
        run = self.s.start_run("full")
        self.s.finish_run(run, listed=3438, fetched=10, failed=1, gone=0)
        r = self.s.db.execute("SELECT * FROM sync_run WHERE id=?", (run,)).fetchone()
        self.assertEqual((r["mode"], r["listed"], r["fetched"], r["failed"]), ("full", 3438, 10, 1))
        self.assertIsNotNone(r["finished_at"])


if __name__ == "__main__":
    unittest.main()


class PlanTargets(unittest.TestCase):
    def setUp(self):
        self.s = Store(Path(":memory:"))

    def tearDown(self):
        self.s.close()

    def test_diff_run_includes_pages_that_errored_after_success(self):
        """指摘の要: ok の後に error になったページが差分実行から漏れないこと。"""
        self.s.save("落ちた", "本文", "2026-09-01T00:00:00+09:00")
        self.s.fail("落ちた", "timeout")
        targets = plan_targets(self.s, {}, None)  # RecentChanges に載っていない
        self.assertIn("落ちた", targets)

    def test_diff_run_finishes_an_interrupted_full_run(self):
        self.s.seen("未取得", None)
        self.assertIn("未取得", plan_targets(self.s, {}, None))

    def test_full_run_covers_list_and_recent_changes_only_pages(self):
        listed = {"あ", "い"}
        targets = plan_targets(self.s, {"FrontPage": "2026-09-01T00:00:00+09:00"}, listed)
        self.assertEqual(targets, ["あ", "い", "FrontPage"])

    def test_no_duplicates(self):
        self.s.seen("あ", None)
        targets = plan_targets(self.s, {"あ": "2026-09-01T00:00:00+09:00"}, {"あ", "い"})
        self.assertEqual(sorted(targets), ["あ", "い"])
        self.assertEqual(len(targets), len(set(targets)))

    def test_gone_pages_are_not_revisited(self):
        self.s.save("消えた", "本文", None)
        self.s.mark_gone("消えた")
        self.assertNotIn("消えた", plan_targets(self.s, {}, None))
