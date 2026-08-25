//! 中ディレイ減少をもたらすキャラのパッシブ・マスタリー。
//!
//! 出典: wiki「ステータス」の `中ディレイ倍率B (初期値:100%、下限30%)` の表(取得 2026-08-25)。
//! 同じ表にある共通の供給源(フルスロットル / カフスのランダムオプション / シエナのオーラ)は
//! それぞれ `UltimateSkills` / `RandomOptionTotals` / `SienaAura` から入るので、ここには入れない。

use domain::ActualDelaySkillDef;

use crate::Source;

pub const ACTUAL_DELAY_SKILLS_SOURCE: Source = Source {
    page: "ステータス「中ディレイ倍率B」",
    retrieved_on: "2026-08-25",
    note: "キャラ固有のパッシブ・マスタリーのみ。共通の供給源(フルスロットル / \
           カフスのランダムオプション / シエナのオーラ)は別経路で入る",
};

/// 1 段だけのパッシブはすべて −5%(wiki の表)。
const FIVE: &[f64] = &[5.0];

const ACTUAL_DELAY_SKILLS: &[ActualDelaySkillDef] = &[
    ActualDelaySkillDef {
        id: "boris_sword_priest",
        name: "剣の司祭",
        game_character_id: "boris",
        percents: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "boris_mastery_issen",
        name: "マスタリー【一閃】",
        game_character_id: "boris",
        percents: FIVE,
        note: "マスタリー",
    },
    ActualDelaySkillDef {
        id: "ispin_rivalry",
        name: "ライバルリー",
        game_character_id: "ispin",
        percents: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "maximin_clumsy_pair",
        name: "ドタバタペア",
        game_character_id: "maximin",
        percents: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "mira_spurt",
        name: "スパート",
        game_character_id: "mira",
        // wiki の表記は `-25%/-15%/-5%/-0%`
        percents: &[25.0, 15.0, 5.0, 0.0],
        note: "パッシブ。段階で減少値が変わる",
    },
    ActualDelaySkillDef {
        id: "mira_mastery_good_face",
        name: "マスタリー【グッドフェイス】",
        game_character_id: "mira",
        percents: FIVE,
        note: "マスタリー(スパートとは別枠で加算)",
    },
    ActualDelaySkillDef {
        id: "chloe_rivalry",
        name: "ライバルリー",
        game_character_id: "chloe",
        percents: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "anais_loki_specialization",
        name: "ロキ特化",
        game_character_id: "anais",
        percents: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "isolet_corona_gale",
        name: "コロナゲイル",
        game_character_id: "isolet",
        percents: FIVE,
        note: "パッシブ",
    },
];

/// 中ディレイ減少スキルのカタログ。
pub fn actual_delay_skill_catalog() -> &'static [ActualDelaySkillDef] {
    ACTUAL_DELAY_SKILLS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikiの表と同じ9件で_id_は一意() {
        assert_eq!(ACTUAL_DELAY_SKILLS.len(), 9);
        let mut ids: Vec<&str> = ACTUAL_DELAY_SKILLS.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 9);
    }

    #[test]
    fn 収録キャラはすべてプレイアブル一覧にある() {
        for d in ACTUAL_DELAY_SKILLS {
            assert!(crate::find_character(d.game_character_id).is_some(), "{} が一覧に無い", d.id);
        }
    }

    #[test]
    fn ミラだけ段階選択で他は5パーセント1段() {
        for d in ACTUAL_DELAY_SKILLS {
            if d.id == "mira_spurt" {
                assert_eq!(d.percents, [25.0, 15.0, 5.0, 0.0]);
            } else {
                assert_eq!(d.percents, [5.0], "{}", d.id);
            }
        }
        let mira: Vec<&str> = ACTUAL_DELAY_SKILLS
            .iter()
            .filter(|d| d.game_character_id == "mira")
            .map(|d| d.id)
            .collect();
        assert_eq!(mira, ["mira_spurt", "mira_mastery_good_face"]);
    }
}
