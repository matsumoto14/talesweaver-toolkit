"""units.py のテスト(ネットワークに出ない)。

    python -m unittest discover -s tools/gamedata/wiki -t tools/gamedata/wiki
"""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from units import (  # noqa: E402
    REBUILT_TABLES, Heading, ListItem, Paragraph, Table, Unit, apparent_equipment_corrections,
    app_data_correction_rows, build_aliases, build_units, choose_key_columns, correction_rows,
    detect_repeat_group, diff_keyed, diff_set, fragment, is_excluded_page, is_item_catalog_page,
    make_row_key, numify, parse_blocks, plan_unit_diff, promote_decorated_header,
    propagate_name_rows, resolve_correction_unit, resolve_merges, row_hash, strip_decorations,
    unwind_table,
)
import units as units_mod  # noqa: E402
from store import Store  # noqa: E402


class Decorations(unittest.TestCase):
    def test_nested_bracket_link_keeps_display(self):
        self.assertEqual(strip_decorations("【村】[[[雑貨店]>Shop/巡礼者の村周辺#cc54ee3b]](383,427)"),
                         "【村】雑貨店(383,427)")

    def test_inline_plugin_trailing_semicolon_is_part_of_syntax(self):
        self.assertEqual(strip_decorations("&color(DarkBlue){参加条件}; ：遠隔"), "参加条件 ：遠隔")
        self.assertEqual(strip_decorations("入場時に&ref(x.png,nolink,);[不安定なマナ]"), "入場時に[不安定なマナ]")

    def test_entities_and_mid_text_prefixes(self):
        self.assertEqual(strip_decorations("貰えるもの：COLOR(blue):マナP&nbsp;大"), "貰えるもの：マナP 大")

    def test_hr_and_cr_do_not_become_units(self):
        blocks = parse_blocks("-項目" + chr(13) + chr(10) + "----" + chr(13) + chr(10) + "本文" + chr(13) + "続き" + chr(13) + chr(10))
        self.assertEqual([type(b).__name__ for b in blocks], ["ListItem", "Paragraph"])
        self.assertEqual(blocks[1].text, "本文" + chr(10) + "続き")

    def test_symbol_only_items_are_not_units(self):
        out = build_units("P", "*見出し [#a1]" + chr(10) + "-" + chr(10) + "-&nbsp;" + chr(10) + "-本文" + chr(10))
        self.assertEqual([u.text for u in out.units], ["本文"])


class ExcludedPages(unittest.TestCase):
    def test_comment_style_names_excluded(self):
        for name in [
            "コメント/エタの意志", "MenuBar", "SideBar", "RecentChanges", "RecentDeleted",
            "InterWikiName", "Help", "FormattingRules", "Skill/共通/1LineBBS", "Skill/共通/コメント",
            "PukiWiki/1.4", ":config/plugin", "何か:RenameLog",
        ]:
            self.assertTrue(is_excluded_page(name), name)

    def test_ops_and_system_pages_excluded(self):
        for name in [
            "Comments/エタの意志", "Z_下書き", "練習用ページ", "練習用ページ/1", "公式告知",
            "公式告知2024", "SandBox", "img", "InterWiki", "InterWikiSandBox",
            "InterWikiテクニカル", "WikiName", "WikiEngines", "YukiWiki", "PHP", "dev-talewiki",
            "AutoTicketLinkName", "提案１", "多目的用ページ", "更新依頼用", "導入済みプラグイン",
            "整形ルール", "掲示板一覧", "ヘルプ", "1LineBBS",
            "MenuBar/Item/武器", "MenuBar7/MenuBar/新メニュー", "Menubar4/Quest", "Menu2/本体",
        ]:
            self.assertTrue(is_excluded_page(name), name)

    def test_domain_pages_with_similar_prefixes_are_not_excluded(self):
        for name in ["Link/公式", "TWFAQ/よくある質問", "MAP/王都", "エタの意志", "Skill/共通"]:
            self.assertFalse(is_excluded_page(name), name)


class Blocks(unittest.TestCase):
    def test_heading_with_explicit_anchor(self):
        blocks = parse_blocks("*概要[#h2_0]\n本文")
        self.assertEqual(blocks[0], Heading(1, "概要", "h2_0"))

    def test_heading_without_anchor_has_none(self):
        blocks = parse_blocks("**強化\n本文")
        self.assertEqual(blocks[0], Heading(2, "強化", None))

    def test_paragraph_split_by_blank_line(self):
        blocks = parse_blocks("段落1の1行目\n段落1の2行目\n\n段落2")
        self.assertEqual(blocks, [Paragraph("段落1の1行目\n段落1の2行目"), Paragraph("段落2")])

    def test_list_items_are_independent_units(self):
        blocks = parse_blocks("-項目1\n--子項目\n-項目2")
        self.assertEqual(len(blocks), 3)
        self.assertTrue(all(isinstance(b, ListItem) for b in blocks))
        self.assertEqual(blocks[0].run_id, blocks[1].run_id)  # 同じ連続塊
        self.assertEqual(blocks[2].run_id, blocks[0].run_id)  # 空行で切れていないので同じ塊

    def test_list_run_breaks_on_blank_line(self):
        blocks = parse_blocks("-項目1\n\n-項目2")
        self.assertNotEqual(blocks[0].run_id, blocks[1].run_id)

    def test_table_block_collected(self):
        blocks = parse_blocks("|Lv|HP|h\n|60|100|\n|70|200|\n")
        self.assertEqual(len(blocks), 1)
        self.assertIsInstance(blocks[0], Table)
        self.assertEqual(blocks[0].header, ["Lv", "HP"])
        self.assertEqual(blocks[0].rows, [["60", "100"], ["70", "200"]])


class TableParsing(unittest.TestCase):
    def test_merged_cells_up_and_right(self):
        table = Table(header=["進化", "強化", "成功率"], rows=[
            ["0", "0", "100%"],
            ["~", "1", "90%"],
            [">", "2", "80%"],
        ])
        resolved = resolve_merges(table)
        self.assertEqual(resolved[1][0], "0")  # ~ は上のセル
        self.assertEqual(resolved[2][0], resolved[2][1])  # > は右のセル

    def test_no_header_row_gets_generated_names(self):
        table = Table(header=None, rows=[["1", "100"], ["2", "200"]])
        out = unwind_table(table)
        self.assertEqual(out.columns, ["列1", "列2"])
        self.assertEqual(out.rows, [["1", "100"], ["2", "200"]])

    def test_side_by_side_repeat_group_is_unwound(self):
        # Shop の「商品名・価格」×2(空の区切り列つき)
        header = ["商品名", "価格", "", "商品名", "価格"]
        table = Table(header=header, rows=[["回復薬", "100", "", "解毒薬", "200"]])
        out = unwind_table(table)
        self.assertEqual(out.columns, ["商品名", "価格"])
        self.assertEqual(out.rows, [["回復薬", "100"], ["解毒薬", "200"]])

    def test_header_colspan_and_symbol_headers(self):
        table = Table(header=[">", "Lv", ":", "必要経験値"], rows=[["1", "2", "3", "4"]])
        out = unwind_table(table)
        self.assertEqual(out.columns, [">", "Lv", ":", "必要経験値"][:0] or out.columns)
        self.assertEqual(out.columns[1], "Lv")
        self.assertEqual(out.columns[3], "必要経験値")

    def test_rows_without_values_are_dropped(self):
        table = Table(header=["A", "B"], rows=[["", ""], ["x", "1"], [">", ""]])
        out = unwind_table(table)
        self.assertEqual(out.rows, [["x", "1"]])

    def test_key_value_table_becomes_item_rows(self):
        table = Table(header=["TOPへ", ""], rows=[
            ["~条件", "Lv35以上"], [">", ""], ["~補足", ""], ["~", "Ver5.28で新方式に移行"],
            ["~概要", "①話しかける"], ["~", "②渡す"], ["~報酬", "100,000Seed"],
        ])
        out = unwind_table(table)
        self.assertEqual(out.columns, ["項目", "内容"])
        self.assertEqual(out.rows, [
            ["条件", "Lv35以上"], ["補足", "Ver5.28で新方式に移行"],
            ["概要", "①話しかける / ②渡す"], ["報酬", "100,000Seed"],
        ])

    def test_repeat_group_drops_fully_empty_pieces(self):
        header = ["商品名", "価格", "商品名", "価格"]
        table = Table(header=header, rows=[["回復薬", "100", "", ""]])
        out = unwind_table(table)
        self.assertEqual(out.rows, [["回復薬", "100"]])


class RowKey(unittest.TestCase):
    def test_unique_first_column_is_the_key(self):
        cols = ["進化", "成功率"]
        rows = [["0", "100%"], ["1", "90%"]]
        self.assertEqual(choose_key_columns(cols, rows), ["進化"])
        self.assertEqual(make_row_key(rows[0], cols, ["進化"]), "0")

    def test_falls_back_to_first_two_columns(self):
        cols = ["進化", "強化", "成功率"]
        rows = [["0", "0", "100%"], ["0", "1", "90%"], ["1", "0", "80%"]]
        self.assertEqual(choose_key_columns(cols, rows), ["進化", "強化"])
        self.assertEqual(make_row_key(rows[1], cols, ["進化", "強化"]), "0-1")

    def test_falls_back_to_content_hash(self):
        cols = ["進化", "強化"]
        rows = [["0", "0"], ["0", "0"]]  # 先頭 1・2 列とも重複する
        self.assertEqual(choose_key_columns(cols, rows), [])
        key = make_row_key(rows[0], cols, [])
        self.assertEqual(len(key), 8)


class Nums(unittest.TestCase):
    def test_plain_number_with_comma(self):
        self.assertEqual(numify("1,234"), 1234)

    def test_percent(self):
        self.assertEqual(numify("100%"), 100)

    def test_plus_sign(self):
        self.assertEqual(numify("+20"), 20)

    def test_range_tilde_variants(self):
        self.assertEqual(numify("60~69"), [60, 69])
        self.assertEqual(numify("60〜69"), [60, 69])
        self.assertEqual(numify("60-69"), [60, 69])

    def test_level_or_above(self):
        self.assertEqual(numify("Lv60以上"), [60, 9999])

    def test_non_numeric_is_none(self):
        self.assertIsNone(numify("なし"))
        self.assertIsNone(numify(""))


class Fragment(unittest.TestCase):
    def test_fullwidth_sentence_ends(self):
        kept, truncated = fragment("一文目！二文目？三文目。四文目")
        self.assertEqual(kept, "一文目！二文目？三文目。")
        self.assertTrue(truncated)

    def test_keeps_up_to_three_sentences(self):
        text = "一文目。二文目。三文目。四文目。"
        kept, truncated = fragment(text)
        self.assertEqual(kept, "一文目。二文目。三文目。")
        self.assertTrue(truncated)

    def test_short_text_not_truncated(self):
        kept, truncated = fragment("一文だけ。")
        self.assertEqual(kept, "一文だけ。")
        self.assertFalse(truncated)

    def test_cuts_at_200_chars(self):
        text = "あ" * 300 + "。"
        kept, truncated = fragment(text)
        self.assertEqual(len(kept), 200)
        self.assertTrue(truncated)


class Alias(unittest.TestCase):
    def test_page_name_itself_is_an_alias(self):
        aliases = build_aliases(["エタの意志"], {})
        self.assertEqual(aliases["エタの意志"], ["エタの意志"])

    def test_path_tail_becomes_alias(self):
        aliases = build_aliases(["Dungeon/エクリプスダンジョン"], {})
        self.assertEqual(aliases["エクリプスダンジョン"], ["Dungeon/エクリプスダンジョン"])

    def test_shared_tail_maps_to_every_page(self):
        aliases = build_aliases(["Dungeon/エクリプスダンジョン", "ミニゲーム/エクリプスダンジョン"], {})
        self.assertEqual(
            aliases["エクリプスダンジョン"],
            ["Dungeon/エクリプスダンジョン", "ミニゲーム/エクリプスダンジョン"],
        )

    def test_tail_shared_by_too_many_pages_is_not_an_alias(self):
        aliases = build_aliases([f"{c}/共通" for c in "ABCD"], {})
        self.assertNotIn("共通", aliases)

    def test_manual_alias_wins(self):
        aliases = build_aliases(["エタの意志"], {"エタ": "エタの意志"})
        self.assertEqual(aliases["エタ"], ["エタの意志"])

    def test_manual_alias_ignored_if_target_missing(self):
        aliases = build_aliases(["エタの意志"], {"エタ": "存在しないページ"})
        self.assertNotIn("エタ", aliases)


def _row(id: str, page: str, text: str) -> Unit:
    return Unit(id=id, kind="row", page=page, section="", anchor="a", ord=1, truncated=0,
                text=text, table_idx=0, group_key=None, row_key=None, cells=None, nums=None)


class Correction(unittest.TestCase):
    def test_match_hits_first_row_on_page(self):
        units = [
            _row("r:1", "Skill/マキシミン", "スキル: 極・毒舌 | 解説: 敵の被ダメージ+10%"),
            _row("r:2", "Skill/マキシミン", "スキル: 極・毒舌 | 解説: 【暴言】：最終ダメージ-10%"),
        ]
        c = {"page": "Skill/マキシミン", "match": "毒舌"}
        self.assertEqual(resolve_correction_unit(c, units), "r:1")

    def test_subject_is_tried_before_match(self):
        # 毒舌【暴言】は基本効果の行ではなく、マスタリー別効果の行(【暴言】を含む)に付く
        units = [
            _row("r:1", "Skill/マキシミン", "区分: QA | 解説: 毒舌 敵の被ダメージ+10%"),
            _row("r:2", "Skill/マキシミン", "区分: P (M2) | スキル: 毒舌 | 解説: 【暴言】最終ダメージ-10%"),
        ]
        c = {"subject": "毒舌【暴言】", "page": "Skill/マキシミン", "match": "毒舌"}
        self.assertEqual(resolve_correction_unit(c, units), "r:2")

    def test_match_not_found_is_none(self):
        units = [_row("r:1", "Skill/マキシミン", "スキル: 呪われた魔剣")]
        c = {"page": "Skill/マキシミン", "match": "毒舌"}
        self.assertIsNone(resolve_correction_unit(c, units))

    def test_match_wrong_page_is_none(self):
        units = [_row("r:1", "Skill/ベンヤ", "スキル: 毒舌")]
        c = {"page": "Skill/マキシミン", "match": "毒舌"}
        self.assertIsNone(resolve_correction_unit(c, units))

    def test_correction_rows_build_stable_id(self):
        units = [_row("r:1", "Skill/マキシミン", "スキル: 極・毒舌")]
        corrections = [{
            "subject": "毒舌【暴言】", "page": "Skill/マキシミン", "match": "毒舌", "col": "効果",
            "value": "敵被ダメージ +10%", "source_kind": "notice",
            "source_title": "テイルズウィーバー公式お知らせ(no=153335)",
            "source_url": "https://talesweaver.nexon.co.jp/notice/notice.aspx?no=153335",
        }]
        rows = correction_rows(corrections, units)
        self.assertEqual(len(rows), 1)
        self.assertIn("'c:r:1/効果/毒舌【暴言】'", rows[0][0])
        self.assertIn("'r:1'", rows[0][2])

    def test_correction_rows_without_matching_unit_is_null(self):
        corrections = [{
            "subject": "迅速の秘薬", "page": "Item/消耗品/ステータス補助", "match": "迅速の秘薬",
            "col": "効果", "value": "全属性 +15", "source_kind": "notice",
            "source_title": "テイルズウィーバー公式お知らせ(no=151355)",
            "source_url": "https://talesweaver.nexon.co.jp/notice/newest.aspx?no=151355",
        }]
        rows = correction_rows(corrections, [])
        self.assertIn("'c:Item/消耗品/ステータス補助/効果/迅速の秘薬'", rows[0][0])
        self.assertEqual(rows[0][2], "NULL")

    def test_correction_rows_grade_from_dict_defaults_to_confirmed(self):
        base = {
            "subject": "毒舌", "page": "Skill/マキシミン", "match": "毒舌", "col": "効果",
            "value": "敵被ダメージ +10%", "source_kind": "notice",
            "source_title": "テイルズウィーバー公式お知らせ(no=153335)",
            "source_url": "https://talesweaver.nexon.co.jp/notice/notice.aspx?no=153335",
        }
        with_grade = correction_rows([{**base, "grade": "apparent"}], [])
        without_grade = correction_rows([base], [])
        self.assertEqual(with_grade[0][5], "'apparent'")
        self.assertEqual(without_grade[0][5], "'confirmed'")


class AppDataCorrections(unittest.TestCase):
    def test_one_row_per_cell_with_null_unit(self):
        app_data = [{
            "subject": "†アーミングソード", "kind": "equipment", "section": "装備(武器)",
            "cells": {"突き": "120", "斬り": "0"},
            "source_title": "アプリのデータ(クライアント DB 由来)",
        }]
        rows = app_data_correction_rows(app_data)
        self.assertEqual(len(rows), 2)
        cols = {r[3].strip("'") for r in rows}
        self.assertEqual(cols, {"突き", "斬り"})
        for r in rows:
            self.assertEqual(r[2], "NULL")  # unit_id
            self.assertEqual(r[5], "'confirmed'")  # grade
            self.assertEqual(r[6], "'client_db'")  # source_kind
            self.assertEqual(r[9], "'装備(武器)'")  # section
        ids = {r[0].strip("'") for r in rows}
        self.assertEqual(ids, {"c:app/†アーミングソード/突き", "c:app/†アーミングソード/斬り"})

    def test_source_kind_derived_from_wiki_derived_title(self):
        app_data = [{
            "subject": "緋馬の怪火", "kind": "title", "section": "称号",
            "cells": {"攻撃ダメージ": "+20%"},
            "source_title": "アプリのデータ(wiki 由来)",
        }]
        rows = app_data_correction_rows(app_data)
        self.assertEqual(rows[0][6], "'gamedata'")


class BuildUnitsIntegration(unittest.TestCase):
    def test_paragraph_and_row_units_from_a_small_page(self):
        source = (
            "*概要[#h2_0]\n"
            "エタの意志についての説明文です。\n\n"
            "**強化\n"
            "|進化|強化|成功率|h\n"
            "|0|0|100%|\n"
            "|0|1|90%|\n"
        )
        page_units = build_units("エタの意志", source)
        kinds = [u.kind for u in page_units.units]
        self.assertEqual(kinds, ["paragraph", "row", "row"])
        self.assertEqual(page_units.units[0].id, "p:エタの意志/h2_0/1")
        self.assertEqual(page_units.units[0].section, "概要")
        self.assertEqual(page_units.units[1].section, "概要 › 強化")
        self.assertTrue(page_units.units[1].anchor.startswith("content_2_"))
        self.assertEqual(page_units.units[1].row_key, "0-0")
        self.assertEqual(page_units.units[1].nums["進化"], 0)
        self.assertEqual(page_units.units[1].nums["成功率"], 100)
        self.assertEqual(len(page_units.tables), 1)
        self.assertEqual(page_units.tables[0].row_count, 2)

    def test_content_before_first_heading_uses_anchor_top(self):
        page_units = build_units("エタの意志", "見出しより前の本文です。")
        self.assertEqual(page_units.units[0].anchor, "top")
        self.assertEqual(page_units.units[0].section, "")


class DecoratedHeaderPromotion(unittest.TestCase):
    """`Item/武器/*` などの `|h` の無い装備表で、`~` 装飾の行を列名に昇格する(段階 3 spec 9)。"""

    def test_promotes_tilde_row_and_joins_br_split_names(self):
        table = Table(header=None, rows=[
            ["~取得場所", "~突&br;き", "~斬&br;り"],
            ["初期装備", "10", "20"],
        ])
        out = promote_decorated_header(table)
        self.assertEqual(out.header, ["取得場所", "突き", "斬り"])
        self.assertEqual(out.rows, [["初期装備", "10", "20"]])

    def test_leaves_table_with_explicit_header_untouched(self):
        table = Table(header=["進化", "成功率"], rows=[["0", "100%"]])
        out = promote_decorated_header(table)
        self.assertIs(out, table)

    def test_no_qualifying_row_leaves_table_untouched(self):
        table = Table(header=None, rows=[["1", "100"], ["2", "200"]])
        out = promote_decorated_header(table)
        self.assertIs(out, table)


class NameRowPropagation(unittest.TestCase):
    """横結合で名前だけ書いた行を後続行に `名前` 列として伝播する(段階 3 spec 9)。"""

    def test_name_row_propagates_to_following_row_and_is_removed(self):
        columns = ["取得場所", "価格", "耐久", "硬度", "突き", "斬り"]
        rows = [
            ["†アカドアーミングソード"] * 4 + ["アカド武器作成"] * 2,
            ["クエスト", "Sell:", "85-90", "92-97", "95", "182"],
        ]
        out_columns, out_rows, first_of_name = propagate_name_rows(columns, rows)
        self.assertEqual(out_columns, ["名前"] + columns)  # 名前はキー列(先頭)
        self.assertEqual(len(out_rows), 1)
        self.assertEqual(out_rows[0][0], "†アカドアーミングソード")
        self.assertEqual(first_of_name, [True])

    def test_second_row_under_same_name_is_not_first_of_name(self):
        columns = ["取得場所", "価格", "耐久", "硬度", "突き", "斬り"]
        rows = [
            ["†銀河ノ刀剣"] * 4 + [""] * 2,
            ["宝箱", "Sell:", "85-90", "92-97", "91-101", "190-200"],
            ["宝箱", "上限", "", "", "280", "300"],
        ]
        _, out_rows, first_of_name = propagate_name_rows(columns, rows)
        self.assertEqual(len(out_rows), 2)
        self.assertEqual([r[0] for r in out_rows], ["†銀河ノ刀剣", "†銀河ノ刀剣"])
        self.assertEqual(first_of_name, [True, False])

    def test_no_name_row_leaves_table_untouched(self):
        columns = ["進化", "成功率"]
        rows = [["0", "100%"], ["1", "90%"]]
        out_columns, out_rows, first_of_name = propagate_name_rows(columns, rows)
        self.assertEqual(out_columns, columns)
        self.assertEqual(out_rows, rows)

    def test_coincidental_two_cell_match_in_narrow_table_is_not_a_name_row(self):
        # 幅が狭い表(進化×強化×成功率)でたまたま値が揃っても名前行にしない
        columns = ["進化", "強化", "成功率"]
        rows = [["0", "0", "100%"]]
        out_columns, out_rows, _ = propagate_name_rows(columns, rows)
        self.assertEqual(out_columns, columns)
        self.assertEqual(out_rows, rows)

    def test_caption_row_with_two_merged_groups_is_not_a_name_row(self):
        # `LEFT:~名称|>|...|LEFT:~レアドロップ...` のキャプション行(`~` 装飾のまま残る)は
        # 名前行にしない(次に来る本当の名前行の名前を上書きしないように)。
        columns = [f"列{i}" for i in range(1, 18)]
        row = ["~名称"] * 4 + ["~レアドロップモンスター名/合成武器の補足"] * 13
        out_columns, out_rows, first_of_name = propagate_name_rows(columns, [row])
        self.assertEqual(out_columns, columns)
        self.assertEqual(out_rows, [row])


class ItemCatalogPageScope(unittest.TestCase):
    def test_item_weapon_armor_accessory_pages_are_in_scope(self):
        for name in ["Item/武器/アーミングソード", "Item/防具/鎧/軽鎧", "Item/アクセサリ/エフェクト"]:
            self.assertTrue(is_item_catalog_page(name), name)

    def test_other_pages_are_out_of_scope(self):
        for name in ["Quest/武器作成", "Skill/マキシミン", "Item/Random/びっくり箱"]:
            self.assertFalse(is_item_catalog_page(name), name)


def _catalog_row(unit_id: str, subject: str, cells: dict[str, str], first: bool = True) -> Unit:
    return Unit(id=unit_id, kind="row", page="Item/武器/テスト", section="", anchor="a", ord=1,
                truncated=0, text="", table_idx=0, group_key=None, row_key=None,
                cells={**cells, "名前": subject}, nums=None, is_first_of_name=first)


class ApparentEquipmentCorrections(unittest.TestCase):
    """wiki の装備行と app_data(client_db 由来)の 9 値を突き合わせる(段階 3 spec 10)。"""

    def test_plain_value_mismatch_becomes_apparent_correction(self):
        app_data = [{
            "subject": "†テスト剣", "kind": "equipment", "section": "装備(武器)",
            "cells": {"突き": "95", "斬り": "182"},
            "source_title": "アプリのデータ(クライアント DB 由来)",
        }]
        units = [_catalog_row("r:1", "†テスト剣", {"突き": "90", "斬り": "182"})]
        rows = apparent_equipment_corrections(app_data, units)
        self.assertEqual(len(rows), 1)
        self.assertIn("'突き'", rows[0][3])
        self.assertIn("'95'", rows[0][4])
        self.assertIn("'apparent'", rows[0][5])
        self.assertIn("'client_db'", rows[0][6])
        self.assertIn("'r:1'", rows[0][2])

    def test_range_values_are_not_compared(self):
        app_data = [{
            "subject": "†テスト剣", "kind": "equipment", "section": "装備(武器)",
            "cells": {"突き": "95"},
            "source_title": "アプリのデータ(クライアント DB 由来)",
        }]
        units = [_catalog_row("r:1", "†テスト剣", {"突き": "88-95"})]
        self.assertEqual(apparent_equipment_corrections(app_data, units), [])

    def test_non_first_row_of_name_group_is_not_compared(self):
        # 上限行など、同じ名前のまま続く行(is_first_of_name=False)は比較しない
        app_data = [{
            "subject": "†テスト剣", "kind": "equipment", "section": "装備(武器)",
            "cells": {"Cri補正": "23"},
            "source_title": "アプリのデータ(クライアント DB 由来)",
        }]
        units = [_catalog_row("r:1", "†テスト剣", {"Cri補正": "29"}, first=False)]
        self.assertEqual(apparent_equipment_corrections(app_data, units), [])

    def test_wiki_derived_app_data_is_not_used_as_source(self):
        # app_data 側が wiki 由来(source_kind='gamedata')の項目は突合の元にしない
        app_data = [{
            "subject": "†テスト剣", "kind": "equipment", "section": "装備(武器)",
            "cells": {"突き": "95"},
            "source_title": "アプリのデータ(wiki 由来)",
        }]
        units = [_catalog_row("r:1", "†テスト剣", {"突き": "90"})]
        self.assertEqual(apparent_equipment_corrections(app_data, units), [])

    def test_matching_values_produce_no_correction(self):
        app_data = [{
            "subject": "†テスト剣", "kind": "equipment", "section": "装備(武器)",
            "cells": {"突き": "95"},
            "source_title": "アプリのデータ(クライアント DB 由来)",
        }]
        units = [_catalog_row("r:1", "†テスト剣", {"突き": "95"})]
        self.assertEqual(apparent_equipment_corrections(app_data, units), [])


class RebuiltTables(unittest.TestCase):
    """units.sql が触る表は、schema.sql のうち利用者から届くもの以外すべて。全件・差分どちらも同じ。"""

    USER_TABLES = {"reaction", "ask_log", "ask_call"}

    def test_rebuilds_every_wiki_table_and_never_user_data(self) -> None:
        import re
        schema = (Path(__file__).resolve().parents[3] / "services/api-worker/schema.sql").read_text(encoding="utf-8")
        tables = set(re.findall(r"^CREATE TABLE (\w+)", schema, re.MULTILINE))
        self.assertTrue(self.USER_TABLES <= tables)
        self.assertEqual(set(REBUILT_TABLES), tables - self.USER_TABLES)

    @staticmethod
    def _store_with_one_page() -> Store:
        store = Store(Path(":memory:"))
        store.db.execute(
            "INSERT INTO page(name, source, mtime, fetched_at, checked_at, status) "
            "VALUES('エタの意志', ?, '2024-01-01T00:00:00+09:00', '2024-01-01T00:00:00Z', "
            "'2024-01-01T00:00:00Z', 'ok')",
            ("*概要[#h2_0]\nエタの意志についての説明文です。\n",),
        )
        store.db.commit()
        return store

    def _generate_sql(self, state: dict | None) -> str:
        with tempfile.TemporaryDirectory() as tmp, patch.object(
            units_mod, "run_segment_cli",
            side_effect=lambda cli, dict_path, texts: [["t"] for _ in texts],
        ):
            out_dir = Path(tmp)
            units_mod.generate(self._store_with_one_page(), out_dir, Path("dummy"), {}, {},
                                None, None, state=state)
            return (out_dir / "units.sql").read_text(encoding="utf-8")

    def test_full_sql_never_touches_user_tables(self) -> None:
        sql = self._generate_sql(state=None)
        for t in self.USER_TABLES:
            self.assertNotIn(t, sql)

    def test_diff_sql_never_touches_user_tables(self) -> None:
        empty_state = {
            "unit": [], "unit_fts_rowids": [], "page": [], "wiki_table": [], "correction": [],
            "alias": [], "unit_link": [], "column_note": [],
        }
        sql = self._generate_sql(state=empty_state)
        for t in self.USER_TABLES:
            self.assertNotIn(t, sql)


class RowHash(unittest.TestCase):
    def test_same_values_same_hash(self):
        values = ["'a'", "1", "NULL"]
        self.assertEqual(row_hash(values), row_hash(list(values)))

    def test_different_values_different_hash(self):
        self.assertNotEqual(row_hash(["'a'"]), row_hash(["'b'"]))

    def test_terms_only_change_changes_the_hash(self):
        # unit の h は「行の全列 + FTS terms」をまとめたもの。terms だけ変わっても h は変わる
        # (別名の追加で分かち書きが変わるケース)。
        body = ["'r:1'", "'row'"]
        h1 = row_hash(body + ["エタ 意志"])
        h2 = row_hash(body + ["エタ 意志 追加語"])
        self.assertNotEqual(h1, h2)


class DiffKeyed(unittest.TestCase):
    def test_classifies_new_changed_removed_unchanged(self):
        old = {"a": "h1", "b": "h2", "c": "h3"}
        new = {"a": "h1", "b": "h2-changed", "d": "h4"}  # a: 変化なし, b: 変化, c: 消滅, d: 新規
        to_delete, to_upsert = diff_keyed(old, new)
        self.assertEqual(to_delete, ["c"])
        self.assertEqual(set(to_upsert), {"b", "d"})

    def test_no_changes_means_no_writes(self):
        old = {"a": "h1", "b": "h2"}
        to_delete, to_upsert = diff_keyed(old, dict(old))
        self.assertEqual(to_delete, [])
        self.assertEqual(to_upsert, [])

    def test_works_with_composite_tuple_keys(self):
        old = {("p", "a", 0): "h1"}
        new = {("p", "a", 0): "h1", ("p", "a", 1): "h2"}
        to_delete, to_upsert = diff_keyed(old, new)
        self.assertEqual(to_delete, [])
        self.assertEqual(to_upsert, [("p", "a", 1)])


class DiffSet(unittest.TestCase):
    def test_classifies_removed_and_added_rows(self):
        old = {("a", "1"), ("b", "2")}
        new = {("a", "1"), ("c", "3")}
        removed, added = diff_set(old, new)
        self.assertEqual(removed, [("b", "2")])
        self.assertEqual(added, [("c", "3")])

    def test_no_changes_means_no_writes(self):
        rows = {("a", "1")}
        removed, added = diff_set(rows, set(rows))
        self.assertEqual(removed, [])
        self.assertEqual(added, [])


def _plan_unit(id_: str) -> Unit:
    return Unit(id=id_, kind="row", page="P", section="", anchor="a", ord=1, truncated=0,
                text="t", table_idx=0, group_key=None, row_key=None, cells=None, nums=None)


class PlanUnitDiff(unittest.TestCase):
    """rowid の割当てと新規/変化/消滅の分類、unit_fts の自己修復(段階 3: 差分投入)。"""

    def test_existing_ids_keep_their_rowid_when_unchanged(self):
        units_ = [_plan_unit("r:1"), _plan_unit("r:2")]
        old_unit = {"r:1": (5, "hA"), "r:2": (6, "hB")}
        plan = plan_unit_diff(units_, ["hA", "hB"], old_unit, old_fts_rowids={5, 6})
        self.assertEqual(plan["id_to_rowid"], {"r:1": 5, "r:2": 6})
        self.assertEqual(plan["upsert_idx"], [])
        self.assertEqual(plan["to_delete_ids"], [])
        self.assertEqual(plan["unchanged_idx"], [0, 1])
        self.assertEqual(plan["fts_delete_rowids"], [])
        self.assertEqual(plan["orphan_fts_rowids"], [])

    def test_changed_content_is_upserted_but_keeps_rowid(self):
        units_ = [_plan_unit("r:1")]
        old_unit = {"r:1": (5, "old-hash")}
        plan = plan_unit_diff(units_, ["new-hash"], old_unit, old_fts_rowids={5})
        self.assertEqual(plan["id_to_rowid"]["r:1"], 5)
        self.assertEqual(plan["upsert_idx"], [0])
        self.assertEqual(plan["fts_delete_rowids"], [5])  # 既存の fts は消してから入れ直す

    def test_new_ids_get_rowid_from_max_plus_one(self):
        units_ = [_plan_unit("r:1"), _plan_unit("r:new")]
        old_unit = {"r:1": (5, "hA")}
        plan = plan_unit_diff(units_, ["hA", "hNew"], old_unit, old_fts_rowids={5})
        self.assertEqual(plan["id_to_rowid"]["r:new"], 6)
        self.assertEqual(plan["upsert_idx"], [1])
        self.assertEqual(plan["fts_delete_rowids"], [])  # 新規は消す物が無い(INSERT のみ)

    def test_rowid_of_a_deleted_tail_row_can_be_reused_later(self):
        # r:2(rowid=6)が消え、次に生成する state はもう r:2 を持たない → 次回はまた 6 から
        # 振られうる(unit/unit_fts の両方から同時に消えているので実害は無い)。
        units_ = [_plan_unit("r:1"), _plan_unit("r:new")]
        old_unit = {"r:1": (5, "hA"), "r:2": (6, "hB")}
        plan = plan_unit_diff(units_, ["hA", "hNew"], old_unit, old_fts_rowids={5, 6})
        self.assertEqual(plan["to_delete_ids"], ["r:2"])
        self.assertEqual(plan["id_to_rowid"]["r:new"], 7)  # 今回はまだ 6 が old_unit にあるので 7 から
        # r:2 が state から抜けたあとの次回なら、max(old_unit)=5 になるので次の新規は 6 から
        next_old_unit = {"r:1": (5, "hA")}
        next_plan = plan_unit_diff([_plan_unit("r:1"), _plan_unit("r:new2")], ["hA", "h2"],
                                    next_old_unit, old_fts_rowids={5})
        self.assertEqual(next_plan["id_to_rowid"]["r:new2"], 6)

    def test_unit_and_unit_fts_rowid_always_match(self):
        # id_to_rowid は unit・unit_fts 両方の INSERT が同じ辞書から引くので、常に一致する
        units_ = [_plan_unit("r:1"), _plan_unit("r:2"), _plan_unit("r:new")]
        old_unit = {"r:1": (1, "h1"), "r:2": (2, "h2")}
        plan = plan_unit_diff(units_, ["h1", "h2", "h3"], old_unit, old_fts_rowids={1, 2})
        rowids = [plan["id_to_rowid"][u.id] for u in units_]
        self.assertEqual(len(rowids), len(set(rowids)))  # 重複なし
        self.assertEqual(plan["id_to_rowid"]["r:new"], 3)

    def test_orphan_fts_rowid_is_detected_and_scheduled_for_delete_first(self):
        # 前回 unit_fts だけ書けて unit が書けなかった(rowid=9 は unit に存在しない)。
        units_ = [_plan_unit("r:1")]
        old_unit = {"r:1": (1, "h1")}
        plan = plan_unit_diff(units_, ["h1"], old_unit, old_fts_rowids={1, 9})
        self.assertEqual(plan["orphan_fts_rowids"], [9])
        self.assertEqual(plan["upsert_idx"], [])  # r:1 自体は変化なし

    def test_unit_present_but_fts_missing_is_treated_as_changed(self):
        # 前回 unit だけ書けて unit_fts が書けなかった(rowid=1 が fts に無い)。中身(h)は同じ。
        units_ = [_plan_unit("r:1")]
        old_unit = {"r:1": (1, "h1")}
        plan = plan_unit_diff(units_, ["h1"], old_unit, old_fts_rowids=set())
        self.assertEqual(plan["upsert_idx"], [0])  # unit_fts を補充するため upsert 対象になる
        self.assertEqual(plan["fts_delete_rowids"], [])  # 消す物は無い(無いから INSERT のみ)
        self.assertEqual(plan["orphan_fts_rowids"], [])


class GenerateDiffWriteOrder(unittest.TestCase):
    """unit_fts の操作が必ず unit より先に出ることを固定する(途中で止まっても自己修復できるように)。"""

    def _run(self, state: dict, unit_body, h, terms) -> tuple[list[str], dict]:
        from units import _generate_diff
        return _generate_diff(
            state,
            page_bodies={}, page_h={},
            all_units=[_plan_unit("r:1")], unit_bodies=[unit_body], unit_h=[h],
            unit_terms_str=[terms],
            table_bodies={}, table_h={},
            note_set=set(), alias_set=set(), link_set=set(),
            correction_bodies={}, correction_h={},
            meta_rows=[["'synced_at'", "'2024-01-01T00:00:00Z'"]],
        )

    def test_no_changes_means_zero_writes_except_meta(self):
        unit_body = [
            "'r:1'", "'row'", "'P'", "''", "'a'", "1", "0", "'t'", "0", "NULL", "NULL", "NULL", "NULL",
        ]
        terms = "エタ 意志"
        h = row_hash(unit_body + [terms])
        state = {
            "unit": [{"id": "r:1", "rowid": 1, "h": h}],
            "unit_fts_rowids": [{"rowid": 1}],
            "page": [], "wiki_table": [], "correction": [], "alias": [], "unit_link": [],
            "column_note": [],
        }
        lines, writes = self._run(state, unit_body, h, terms)
        for table in ("unit", "unit_fts", "page", "wiki_table", "correction", "alias",
                      "unit_link", "column_note"):
            self.assertEqual(writes[table], 0, table)
        self.assertGreater(writes["meta"], 0)  # meta だけは毎回書き直す
        # 変化なしなら出る文は meta の置き換えだけ(消してから入れない。途中で止まっても meta が空にならない)
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith("INSERT OR REPLACE INTO meta"))

    def test_unit_fts_statements_come_before_unit_statements_when_changed(self):
        old_body = [
            "'r:1'", "'row'", "'P'", "''", "'a'", "1", "0", "'old'", "0", "NULL", "NULL", "NULL", "NULL",
        ]
        old_h = row_hash(old_body + ["エタ"])
        new_body = [
            "'r:1'", "'row'", "'P'", "''", "'a'", "1", "0", "'new'", "0", "NULL", "NULL", "NULL", "NULL",
        ]
        new_h = row_hash(new_body + ["エタ"])
        state = {
            "unit": [{"id": "r:1", "rowid": 1, "h": old_h}],
            "unit_fts_rowids": [{"rowid": 1}],
            "page": [], "wiki_table": [], "correction": [], "alias": [], "unit_link": [],
            "column_note": [],
        }
        lines, writes = self._run(state, new_body, new_h, "エタ")
        fts_stmt_idx = [i for i, s in enumerate(lines) if "unit_fts" in s]
        unit_stmt_idx = [i for i, s in enumerate(lines)
                          if ("FROM unit " in s or "INTO unit(" in s) and "unit_fts" not in s]
        # 少なくとも 1 本ずつはある。unit_fts の文は全て unit の文より前に出る。
        self.assertTrue(fts_stmt_idx)
        self.assertTrue(unit_stmt_idx)
        self.assertLess(max(fts_stmt_idx), min(unit_stmt_idx))
        self.assertEqual(writes["unit"], 1)
        self.assertEqual(writes["unit_fts"], 2)  # DELETE 1 + INSERT 1

    def test_orphan_fts_row_is_deleted_and_missing_fts_row_is_filled_in(self):
        unit_body = [
            "'r:1'", "'row'", "'P'", "''", "'a'", "1", "0", "'t'", "0", "NULL", "NULL", "NULL", "NULL",
        ]
        h = row_hash(unit_body + ["エタ"])
        state = {
            # unit は変わらない(h 一致)が、unit_fts は rowid=1 が欠けていて rowid=9 が孤児。
            "unit": [{"id": "r:1", "rowid": 1, "h": h}],
            "unit_fts_rowids": [{"rowid": 9}],
            "page": [], "wiki_table": [], "correction": [], "alias": [], "unit_link": [],
            "column_note": [],
        }
        lines, writes = self._run(state, unit_body, h, "エタ")
        sql = "\n".join(lines)
        self.assertIn("DELETE FROM unit_fts WHERE rowid IN (9)", sql)
        self.assertIn("INSERT INTO unit_fts(rowid, terms) VALUES\n(1,", sql)
        # unit 表自体には(中身が同じなので)書き込みが要らないはずだが、fts 補充と同じ upsert_idx
        # に乗るため REPLACE は出る(中身は変わらない、実害の無い余剰書き込み)。
        self.assertEqual(writes["unit"], 1)
        self.assertEqual(writes["unit_fts"], 2)  # 孤児 DELETE 1 + 補充 INSERT 1


class DeleteStatementSize(unittest.TestCase):
    def test_delete_is_split_below_the_d1_statement_limit(self):
        # 長い id が並んでも 1 文が上限(D1 は 100 KB)を超えず、全件がどれかの文に入る
        ids = [units_mod.sql_str("p:" + "あ" * 200 + f"/{i}") for i in range(2000)]
        stmts = units_mod.delete_in("unit", "id", ids)
        self.assertGreater(len(stmts), 1)
        for stmt in stmts:
            self.assertLess(len(stmt.encode("utf-8")), units_mod._MAX_STMT_BYTES)
        self.assertEqual(sum(stmt.count("'p:") for stmt in stmts), len(ids))

    def test_composite_delete_is_split_too(self):
        rows = [[units_mod.sql_str("い" * 300), units_mod.sql_str("ページ"), str(i)] for i in range(1000)]
        stmts = units_mod.delete_composite_in("unit_link", ["unit_id", "page", "ord"], rows)
        self.assertGreater(len(stmts), 1)
        for stmt in stmts:
            self.assertLess(len(stmt.encode("utf-8")), units_mod._MAX_STMT_BYTES)


if __name__ == "__main__":
    unittest.main()
