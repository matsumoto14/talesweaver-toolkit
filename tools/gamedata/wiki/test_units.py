"""units.py のテスト(ネットワークに出ない)。

    python -m unittest discover -s tools/gamedata/wiki -t tools/gamedata/wiki
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from units import (  # noqa: E402
    Heading, ListItem, Paragraph, Table, Unit, apparent_equipment_corrections,
    app_data_correction_rows, build_aliases, build_units, choose_key_columns, correction_rows,
    detect_repeat_group, fragment, is_excluded_page, is_item_catalog_page, make_row_key, numify,
    parse_blocks, promote_decorated_header, propagate_name_rows, resolve_correction_unit,
    resolve_merges, strip_decorations, unwind_table,
)


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


if __name__ == "__main__":
    unittest.main()
