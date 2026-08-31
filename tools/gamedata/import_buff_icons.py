"""Tale Wiki のバフ表から、直接対応が確認できるアイコンを取り込む。

出力名は gamedata の buff id と一致させるため、UI は対応表を持たず
`assets/icons/buffs/<id>.png` を機械的に解決できる。
"""

from __future__ import annotations

import argparse
import sys
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/buffs"

# Tale Wiki「ステータス」の各バフ行に表示されている添付画像。
# 同じ画像が明記されている別名・派生バフだけ共有する。
ICONS: dict[str, tuple[str, str]] = {
    "illumination_drink": ("ステータス", "イルミネーション祭りのドリンク.png"),
    "snowman_potion": ("ステータス", "ユキダルマン族の特製ポーション(30分).png"),
    "charge_potion": ("ステータス", "充填の秘薬.png"),
    "buff_concentrate": ("ステータス", "バフ濃縮液.png"),
    "guardian_potion": ("ステータス", "守護者のためのポーション.png"),
    "isabelle_ratio": ("ステータス", "イザベルの秘法 (比率能力値).png"),
    "isabelle_fixed": ("ステータス", "イザベルの秘法 (固定能力値).png"),
    "isabelle_rare_percent": ("ステータス", "イザベルの秘法 (比率能力値).png"),
    "isabelle_rare_fixed": ("ステータス", "イザベルの秘法 (固定能力値).png"),
    "event_buff": ("ステータス", "能力値10%上昇.png"),
    "trust_potion": ("ステータス", "改・信頼の薬.png"),
    "fixed_increase": ("ステータス", "メバルのフライ.png"),
    "club_s_effect": ("ステータス", "scr10.png"),
    "club_s_effect_single_stat": ("ステータス", "scr10.png"),
    "club_s_effect_all_stats": ("ステータス", "scr10.png"),
    "unleash": ("Skill/共通", "能力開放系.png"),
    "isabel_damage": ("ステータス", "イザベルの秘法 (ダメージ).png"),
    "isabel_special_damage": ("ステータス", "イザベルの秘法 (ダメージ).png"),
    "moonlight_potion": ("ステータス", "月光・怪力のポーション.png"),
    "silver_sword_stew": ("ステータス", "＜シルバーソード＞のクリームシチュー.png"),
    "festival_food": ("ステータス", "おいしいフェスティバル料理.png"),
    "awakening_elixir": ("ステータス", "覚醒の秘薬.png"),
    "improved_awakening_elixir": ("Item/消耗品/ステータス補助", "改・覚醒の秘薬.png"),
    "strength_ham": ("ステータス", "怪力のハム.png"),
    "ancient_ganapoly_mana": ("ミニゲーム/メルカルト研究所", "Fragment.png"),
    "attendance_buff": ("ステータス", "スペシャル出席チェックバフ.png"),
    "daily_burning_buff": ("ステータス", "スペシャル出席チェックバフ.png"),
    "soul_link_explore": ("ソウルリンク", "BuffAttack.png"),
    "berserker_rune": ("ルーンマスター", "狂戦士のルーン.png"),
    "fever": ("ステータス", "フィーバー3段階.png"),
    "deep_rune_attack": ("ルーンマスター", "深化ルーン攻撃.png"),
    "growth_support_potion": ("ステータス", "リノベーション成長バフ.png"),
    "blessing_potion": ("ステータス", "祝福のポーション.png"),
    "demon_slayer_blessing": ("ステータス", "退魔師の恵み.png"),
    "karill_buff_scroll": ("ステータス", "カリル家のバフスクロール.png"),
    "sakuraeda_hitokata": ("ステータス", "桜枝のヒトカタ.png"),
    "oborozuka_cream_bread": ("ステータス", "朧塚商店街のクリームパン.png"),
    "club_shop_buff_type_p": ("ステータス", "バフスクロール.png"),
    "wednesday_attack_c_rank": ("ステータス", "AF攻撃.png"),
    "twin_dango": ("ステータス", "双子のお団子.png"),
}


def encoded(value: str) -> str:
    return urllib.parse.quote_from_bytes(value.encode("euc_jp"))


def download(page: str, source_name: str) -> bytes:
    url = (
        "https://talewiki.com/?plugin=ref"
        f"&page={encoded(page)}&src={encoded(source_name)}"
    )
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read()
        content_type = response.headers.get_content_type()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(
            f"PNG ではありません: {page}/{source_name} ({content_type}, {len(data)} bytes)"
        )
    return data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--overwrite", action="store_true")
    args = parser.parse_args()

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    downloaded = 0
    skipped = 0
    for buff_id, (page, source_name) in ICONS.items():
        destination = OUTPUT_DIR / f"{buff_id}.png"
        if destination.exists() and not args.overwrite:
            skipped += 1
            continue
        destination.write_bytes(download(page, source_name))
        downloaded += 1
        print(f"GET {buff_id}.png <- {page}/{source_name}")
    print(f"downloaded={downloaded} skipped={skipped} source-backed={len(ICONS)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
