"""Tale Wiki のアーティファクトページから収録 AF のアイコンを取り込む。

取得済みの Wiki ソースを使う例:
    python tools/gamedata/import_artifact_icons.py --source C:\\tmp\\talewiki-artifact.txt

Wiki ソースも取得する例:
    python tools/gamedata/import_artifact_icons.py
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
CATALOG_RS = ROOT / "crates/gamedata/src/equipment_catalog.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/equipment"
WIKI_PAGE = "Item/アクセサリー用装備/アーティファクト"

HELPER_ITEM_RE = re.compile(
    r'(?:artifact_item|defensio_artifact)\("([^"]+)",\s*"(†[^"]+)"'
)
STRUCT_ITEM_RE = re.compile(
    r'id:\s*"([^"]+)",\s*slot:\s*PartSlot::Artifact,.*?'
    r'name:\s*"(†[^"]+)"',
    re.DOTALL,
)
LOCAL_REF_RE = re.compile(r"&ref\((.+?\.png)(?:,[^)]*)?\)")

# Wiki の表示名と添付名の語順が異なる唯一の行。
FILENAME_OVERRIDES = {
    "ethereal-hack-int": "エーテリアルチューブ(斬魔力).png",
}


@dataclass(frozen=True)
class Artifact:
    item_id: str
    name: str


def decode_euc_jp_with_nec(data: bytes) -> str:
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


def encoded(value: str) -> str:
    return urllib.parse.quote_from_bytes(value.encode("euc_jp"))


def fetch_source() -> str:
    url = f"https://talewiki.com/?cmd=source&page={encoded(WIKI_PAGE)}"
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        raw = response.read()
    decoded = decode_euc_jp_with_nec(raw)
    match = re.search(r"<pre[^>]*>(.*?)</pre>", decoded, re.DOTALL)
    if match is None:
        raise RuntimeError(f"Wiki ソースを取得できません: {WIKI_PAGE}")
    return html.unescape(match.group(1))


def catalog() -> list[Artifact]:
    source = CATALOG_RS.read_text(encoding="utf-8")
    pairs = HELPER_ITEM_RE.findall(source) + STRUCT_ITEM_RE.findall(source)
    unique = {item_id: Artifact(item_id, name) for item_id, name in pairs}
    return list(unique.values())


def canonical_filename(filename: str) -> str:
    return Path(filename).stem.removeprefix("†").strip()


def icon_index(source: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for line in source.splitlines():
        refs = [ref for ref in LOCAL_REF_RE.findall(line) if "/" not in ref]
        for ref in refs:
            stem = canonical_filename(ref)
            if f"†{stem}" in line:
                result.setdefault(stem, ref)
    return result


def download(filename: str) -> bytes:
    url = (
        "https://talewiki.com/?plugin=ref"
        f"&page={encoded(WIKI_PAGE)}&src={encoded(filename)}"
    )
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read()
        content_type = response.headers.get_content_type()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(f"PNG ではありません ({content_type}, {len(data)} bytes)")
    return data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, help="fetch_page.py で取得済みの Wiki ソース")
    parser.add_argument("--dry-run", action="store_true", help="照合だけ行い画像を取得しない")
    parser.add_argument("--overwrite", action="store_true", help="既存画像も置き換える")
    args = parser.parse_args()

    source = args.source.read_text(encoding="utf-8") if args.source else fetch_source()
    index = icon_index(source)
    artifacts = catalog()
    matched: list[tuple[Artifact, str, bool]] = []
    for artifact in artifacts:
        plain_name = artifact.name.removeprefix("†")
        filename = FILENAME_OVERRIDES.get(artifact.item_id) or index.get(plain_name)
        guessed = filename is None
        matched.append((artifact, filename or f"{plain_name}.png", guessed))

    print(f"catalog={len(artifacts)} referenced={sum(not guessed for _, _, guessed in matched)}")
    for artifact, filename, guessed in matched:
        marker = "PROBE" if guessed else "MATCH"
        print(f"{marker} {artifact.item_id}: {artifact.name} <- {filename}")
    if args.dry_run:
        return 0

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    downloaded = 0
    skipped = 0
    missing: list[tuple[Artifact, str, str]] = []
    for artifact, filename, _ in matched:
        destination = OUTPUT_DIR / f"{artifact.item_id}.png"
        if destination.exists() and not args.overwrite:
            skipped += 1
            continue
        try:
            destination.write_bytes(download(filename))
        except Exception as error:  # URL ごとの欠落を一覧化して残りを続行する
            missing.append((artifact, filename, str(error)))
            continue
        downloaded += 1
        print(f"GET {destination.name} <- {filename}")

    for artifact, filename, error in missing:
        print(f"MISSING {artifact.item_id}: {artifact.name} ({filename}: {error})")
    print(f"downloaded={downloaded} skipped={skipped} missing={len(missing)}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
