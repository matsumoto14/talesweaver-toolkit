"""Tale Wiki の各キャラクターページから通常スキルのアイコンを取り込む。

`crates/gamedata/src/skills.rs` の SkillRecord と Wiki のリンク表示名を照合し、
`apps/desktop/src/assets/icons/skills/<skill-id>.png` に保存する。

取得済みの Wiki ソースを使う例:
    python tools/gamedata/import_skill_icons.py --source-dir C:\\tmp\\talewiki-skills

Wiki ソースも取得する例:
    python tools/gamedata/import_skill_icons.py
"""

from __future__ import annotations

import argparse
import html
import re
import sys
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SKILLS_RS = ROOT / "crates/gamedata/src/skills.rs"
CHARACTER_SKILLS_RS = ROOT / "crates/gamedata/src/character_skills.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/skills"

CHARACTERS = {
    "lucian": "ルシアン",
    "boris": "ボリス",
    "ispin": "イスピン",
    "maximin": "マキシミン",
    "tichiel": "ティチエル",
    "nayatorei": "ナヤトレイ",
    "siberin": "シベリン",
    "mira": "ミラ",
    "joshua": "ジョシュア",
    "chloe": "クロエ",
    "ranjie": "ランジエ",
    "isaac": "イサック",
    "anais": "アナイス",
    "isolet": "イソレット",
    "benya": "ベンヤ",
    "roamini": "ロアミニ",
    "nocturne": "ノクターン",
    "leeche": "リーチェ",
    "yefnen": "イェフネン",
}

SKILL_RE = re.compile(r'^\s*s\("([^"]+)", "([^"]+)", "([^"]+)"', re.MULTILINE)
CHARACTER_SKILL_RE = re.compile(
    r'CharacterSkillDef\s*\{\s*id:\s*"([^"]+)",\s*game_character_id:\s*"([^"]+)",'
    r'\s*name:\s*"([^"]+)"',
    re.DOTALL,
)
REF_LINK_RE = re.compile(
    r"&ref\(([^,)]+)(?:,[^)]*)?\);[^|\n]*?\[\[([^]>]+)>#([^]]+)]]"
)


@dataclass(frozen=True)
class WikiIcon:
    page: str
    source_name: str
    anchor: str


# 1つの Wiki リンク名に対応しない集約行・状態行だけ、根拠となる添付を指定する。
# 通常スキルとキャラスキルの大半は表示名から機械照合する。
ICON_OVERRIDES = {
    # 3スキル共通のマスタリー効果【力を込めた連撃】
    "lucian_powered_streak": "Mastary3_3.png",
    # カタログは憑依モードと呼ぶが、Wiki のスキル名は剣闘士 / 魔法師
    "joshua_possession_swordsman": "Gladiator.png",
    "joshua_possession_mage": "Wizard.png",
    # ロキ召喚中にだけ付く効果
    "anais_loki_specialization": "LokiSummons.png",
    # 自身用と味方用を別行にした同一スキル
    "benya_altruistic_spirit_party": "ScytheSkill16.png",
    "leeche_attack_fever_party": "AnaroseSkill_3.png",
    # ミラクルスピリットで付く状態。Wiki に独立したスキルアイコンはない
    "benya_dark_blessing": "HammerSkill7.png",
    # フラグへのマスタリー別追加効果
    "yefnen_sharp_shard": "Mastary3_2.png",
    "yefnen_sticky_shard": "Mastary3_3.png",
}


def decode_euc_jp_with_nec(data: bytes) -> str:
    """Tale Wiki の EUC-JP と NEC 拡張文字を文字境界を壊さず復号する。"""
    result: list[str] = []
    buffer = bytearray()
    index = 0

    def flush() -> None:
        if buffer:
            result.append(buffer.decode("euc_jp", errors="replace"))
            buffer.clear()

    while index < len(data):
        first = data[index]
        if first < 0x80:
            buffer.append(first)
            index += 1
        elif first == 0x8E:
            buffer += data[index : index + 2]
            index += 2
        elif first == 0x8F:
            buffer += data[index : index + 3]
            index += 3
        elif first == 0xAD and index + 1 < len(data):
            flush()
            cell = data[index + 1] - 0x80
            trail = cell + 0x1F if cell <= 0x5F else cell + 0x21
            result.append(bytes([0x87, trail]).decode("cp932", errors="replace"))
            index += 2
        else:
            buffer += data[index : index + 2]
            index += 2
    flush()
    return "".join(result)


def fetch_source(page: str) -> str:
    url = f"https://talewiki.com/?cmd=source&page={encoded(page)}"
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        raw = response.read()
    decoded = decode_euc_jp_with_nec(raw)
    match = re.search(r"<pre[^>]*>(.*?)</pre>", decoded, re.DOTALL)
    if match is None:
        raise RuntimeError(f"Wiki ソースを取得できません: {page}")
    return html.unescape(match.group(1))


def catalog() -> dict[str, list[tuple[str, str]]]:
    result = {character_id: [] for character_id in CHARACTERS}
    source = SKILLS_RS.read_text(encoding="utf-8")
    for character_id, local_id, name in SKILL_RE.findall(source):
        if character_id in result:
            result[character_id].append((f"{character_id}_{local_id}", name))
    character_skills = CHARACTER_SKILLS_RS.read_text(encoding="utf-8")
    for skill_id, character_id, name in CHARACTER_SKILL_RE.findall(character_skills):
        if character_id in result:
            result[character_id].append((skill_id, name))
    return result


def wiki_source(character_id: str, source_dir: Path | None) -> str:
    if source_dir is not None:
        return (source_dir / f"{character_id}.txt").read_text(encoding="utf-8")
    return fetch_source(f"Skill/{CHARACTERS[character_id]}")


def clean_label(label: str) -> str:
    # Wiki の強調記法だけを除く。ゲーム内名称の記号はそのまま照合に使う。
    return html.unescape(label).replace("''", "").strip()


def canonical_name(name: str) -> str:
    """効果カタログ側で補われた接頭辞・条件名を除き、Wiki のスキル名に寄せる。"""
    name = clean_label(name).replace("†", "")
    if name.startswith("極・"):
        name = name[2:]
    name = re.sub(r"[<＜][^>＞]+[>＞]$", "", name)
    name = re.sub(r"[（(](?:ペナルティ|チーム)[）)]$", "", name)
    return name.strip()


def icon_index(character_page: str, source: str) -> dict[str, WikiIcon]:
    result: dict[str, WikiIcon] = {}
    for ref, label, anchor in REF_LINK_RE.findall(source):
        ref = ref.strip()
        label = clean_label(label)
        if "/" in ref:
            page, source_name = ref.rsplit("/", 1)
        else:
            page, source_name = character_page, ref
        result.setdefault(label, WikiIcon(page=page, source_name=source_name, anchor=anchor))
    canonical: dict[str, list[WikiIcon]] = {}
    for label, icon in result.items():
        canonical.setdefault(canonical_name(label), []).append(icon)
    for label, icons in canonical.items():
        unique = list(dict.fromkeys(icons))
        if len(unique) == 1:
            result.setdefault(label, unique[0])
    return result


def encoded(value: str) -> str:
    return urllib.parse.quote_from_bytes(value.encode("euc_jp"))


def download(icon: WikiIcon) -> bytes:
    url = (
        "https://talewiki.com/?plugin=ref"
        f"&page={encoded(icon.page)}&src={encoded(icon.source_name)}"
    )
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read()
        content_type = response.headers.get_content_type()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(
            f"PNG ではありません: {icon.page}/{icon.source_name} "
            f"({content_type}, {len(data)} bytes)"
        )
    return data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source-dir",
        type=Path,
        help="fetch_page.py で取得済みの <character-id>.txt があるディレクトリ",
    )
    parser.add_argument("--dry-run", action="store_true", help="照合だけ行い画像を取得しない")
    parser.add_argument(
        "--overwrite", action="store_true", help="すでにある画像も Wiki の内容で置き換える"
    )
    args = parser.parse_args()

    expected = catalog()
    matched: list[tuple[str, WikiIcon]] = []
    missing: list[tuple[str, str]] = []

    for character_id, skills in expected.items():
        page = f"Skill/{CHARACTERS[character_id]}"
        source = wiki_source(character_id, args.source_dir)
        icons = icon_index(page, source)
        for skill_id, name in skills:
            icon = icons.get(name) or icons.get(canonical_name(name))
            if icon is None and skill_id in ICON_OVERRIDES:
                icon = WikiIcon(page, ICON_OVERRIDES[skill_id], anchor="override")
            if icon is None:
                missing.append((skill_id, name))
            else:
                matched.append((skill_id, icon))

    print(f"catalog={sum(map(len, expected.values()))} matched={len(matched)} missing={len(missing)}")
    for skill_id, name in missing:
        print(f"MISSING {skill_id}: {name}")
    if missing:
        return 1
    if args.dry_run:
        return 0

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    downloaded = 0
    skipped = 0
    for skill_id, icon in matched:
        destination = OUTPUT_DIR / f"{skill_id}.png"
        if destination.exists() and not args.overwrite:
            skipped += 1
            continue
        destination.write_bytes(download(icon))
        downloaded += 1
        print(f"GET {skill_id}.png <- {icon.page}/{icon.source_name}")
    print(f"downloaded={downloaded} skipped={skipped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
