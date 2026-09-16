"""Tale Wiki の「Skill/共通」「Skill/極限」から共通スキル・極限スキルのアイコンを取り込む。

出力は `apps/desktop/src/assets/icons/skills/<id>.png`。UI(`ui/Icon.svelte`)は id から機械的に解決する。
- 共通スキルは `common_<名前>`(CommonSkillPane の項目名。gamedata に id が無いのでここで決める)
- 極限スキルは domain `UltimateSkill` の id(scope_eye / full_throttle / wide_focus)

wiki の表記との対応: ハイパーアタック(HA) = スーパーリミット、エクストリームアタック(EA) = ハイパーリミット
(日本クライアントの名前が違う。crates/domain/src/common_skill.rs 参照)。

使い方:
    python tools/gamedata/import_common_skill_icons.py [--overwrite]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from import_buff_icons import download

ROOT = Path(__file__).resolve().parents[2]
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/skills"

ICONS: dict[str, tuple[str, str]] = {
    "common_augment": ("Skill/共通", "オーグメント.png"),
    "common_unleash": ("Skill/共通", "アンリーシュ_STAB.png"),
    "common_sharpness_vision": ("Skill/共通", "シャープネスビジョン.png"),
    "common_power_weapon": ("Skill/共通", "powerWeapon.png"),
    "common_strong_weapon": ("Skill/共通", "strongWeapon.png"),
    "common_coat_armor": ("Skill/共通", "coatArmor.png"),
    "common_protect_armor": ("Skill/共通", "protectArmor.png"),
    "common_kai_protect_armor": ("Skill/共通", "改・プロテクトアーマー.png"),
    "common_super_limit": ("Skill/共通", "hyperAttack.png"),
    "common_hyper_limit": ("Skill/共通", "extremeAttack.png"),
    "common_reinforce": ("Skill/共通", "レインフォース.png"),
    # 極限スキル(Skill/極限 の表は Skill/ゲージスキル の添付を参照している)
    "scope_eye": ("Skill/ゲージスキル", "02-08スコープアイ.png"),
    "full_throttle": ("Skill/ゲージスキル", "02-02フルスロットル.png"),
    "wide_focus": ("Skill/ゲージスキル", "02-01ワイドフォーカス.png"),
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--overwrite", action="store_true")
    args = parser.parse_args()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    downloaded = skipped = 0
    for icon_id, (page, source_name) in ICONS.items():
        destination = OUTPUT_DIR / f"{icon_id}.png"
        if destination.exists() and not args.overwrite:
            skipped += 1
            continue
        destination.write_bytes(download(page, source_name))
        downloaded += 1
        print(f"GET {icon_id}.png <- {page}/{source_name}")
    print(f"downloaded={downloaded} skipped={skipped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
