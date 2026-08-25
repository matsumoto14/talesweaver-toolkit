//! 中ディレイ減少をもたらすキャラのパッシブ・マスタリー。
//!
//! 出典: wiki「ステータス」の `中ディレイ倍率B (初期値:100%、下限30%)` の表(取得 2026-08-25)。
//! 同じ表にある共通の供給源(フルスロットル / カフスのランダムオプション / シエナのオーラ)は
//! それぞれ `UltimateSkills` / `RandomOptionTotals` / `SienaAura` から入るので、ここには入れない。
//!
//! ミラの「極・スパート」は素のままだと **25% から 9.6 秒かけて 0% まで減衰**するので
//! 定常値として扱えない(wiki Skill/ミラ #Spurt)。マスタリー【グッドフェイス】を取ると
//! **中ディレイ低下率が 5% に固定**される(移動速度増加が消える代わりに持続 5 分)ので、
//! **グッドフェイス前提の 5% 固定**として 1 件だけ収録する(ユーザー確定 2026-08-25)。

use domain::ActualDelaySkillDef;

use crate::Source;

pub const ACTUAL_DELAY_SKILLS_SOURCE: Source = Source {
    page: "ステータス「中ディレイ倍率B」",
    retrieved_on: "2026-08-25",
    note: "キャラ固有のパッシブ・マスタリーのみ。共通の供給源(フルスロットル / \
           カフスのランダムオプション / シエナのオーラ)は別経路で入る",
};

/// wiki の表はすべて −5%。
const FIVE: f64 = 5.0;

const ACTUAL_DELAY_SKILLS: &[ActualDelaySkillDef] = &[
    ActualDelaySkillDef {
        id: "boris_sword_priest",
        name: "剣の司祭",
        game_character_id: "boris",
        percent: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "boris_mastery_issen",
        name: "マスタリー【一閃】",
        game_character_id: "boris",
        percent: FIVE,
        note: "マスタリー",
    },
    ActualDelaySkillDef {
        id: "ispin_rivalry",
        name: "ライバルリー",
        game_character_id: "ispin",
        percent: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "maximin_clumsy_pair",
        name: "ドタバタペア",
        game_character_id: "maximin",
        percent: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "mira_spurt",
        name: "極・スパート【グッドフェイス】",
        game_character_id: "mira",
        percent: FIVE,
        note: "マスタリー【グッドフェイス】前提(素は 25% から 9.6 秒で 0% まで減衰する)",
    },
    ActualDelaySkillDef {
        id: "chloe_rivalry",
        name: "ライバルリー",
        game_character_id: "chloe",
        percent: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "anais_loki_specialization",
        name: "ロキ特化",
        game_character_id: "anais",
        percent: FIVE,
        note: "パッシブ",
    },
    ActualDelaySkillDef {
        id: "isolet_corona_gale",
        name: "コロナゲイル",
        game_character_id: "isolet",
        percent: FIVE,
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
    fn 収録は8件で_id_は一意() {
        assert_eq!(ACTUAL_DELAY_SKILLS.len(), 8);
        let mut ids: Vec<&str> = ACTUAL_DELAY_SKILLS.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn 収録キャラはすべてプレイアブル一覧にある() {
        for d in ACTUAL_DELAY_SKILLS {
            assert!(crate::find_character(d.game_character_id).is_some(), "{} が一覧に無い", d.id);
        }
    }

    /// wiki ステータス「中ディレイ倍率B」はキャラ固有ぶんがすべて −5%。
    /// ミラの極・スパートも【グッドフェイス】前提の 5% 固定として収録している。
    #[test]
    fn 全件5パーセントでミラはスパート1件だけ() {
        for d in ACTUAL_DELAY_SKILLS {
            assert_eq!(d.percent, 5.0, "{}", d.id);
        }
        let mira: Vec<&str> = ACTUAL_DELAY_SKILLS
            .iter()
            .filter(|d| d.game_character_id == "mira")
            .map(|d| d.id)
            .collect();
        assert_eq!(mira, ["mira_spurt"]);
    }
}
