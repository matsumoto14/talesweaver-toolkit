//! イェフネンの <フラグ>(技とは別枠のダメージ)。
//!
//! 出典: 韓国公式スキル情報「귀환(帰還)」
//! <https://tw.dn.nexoncdn.co.kr/ActionInfo/18_Yevgnen/3008482.htm>(2026-09-21 取得)、
//! Tale Wiki「Skill/イェフネン」#Flag・#YevgnenSkill4、「ステータス」#z4747f51(同日)。
//!
//! 連 / 爆(全形態)が敵に <フラグ> を積み、スレイ / クラッシュ(全形態)が爆発させる。
//! 持続も爆発も**技と同じ材料**(能力値・装備・バフ・敵デバフ・割合追加ダメージ)で計算する
//! 2 本目のダメージで、技の与ダメージ式のカテゴリには合流しない。
//!
//! 形態限定の効果(速剣・後方攻撃・最大チャージ)は <フラグ> には効かないので、
//! ここで作る技データは形態の印(`Skill::form` ほか)を持たない。

use domain::{
    CharacterSkills, DamageCategory, DamageContribution, Element, Masteries, Skill,
    SkillDependency, SkillForm,
};

use crate::character_skills::character_skill_catalog;

/// <フラグ> のスタック数を持つキャラスキルの id(`CharacterSkillDef`)。
/// ON = 敵に <フラグ> が付いている、SLv = スタック数(0 = OFF)。
pub const FLAG_SKILL_ID: &str = "yefnen_flag";

/// スタック数(1〜10)→ スキル倍率。持続・爆発とも同じ表(極限版)。
/// 韓国公式「귀환」の <フラグ> 行(320 / 323 / 326 / 330 / 336 / 342 / 349 / 365 / 381 / 400%)。
pub const FLAG_MULTIPLIERS: [f64; 10] = [
    3.20, 3.23, 3.26, 3.30, 3.36, 3.42, 3.49, 3.65, 3.81, 4.00,
];

/// 持続ダメージの周期(秒)。韓国公式は 1 秒(wiki は 1.2 秒と書いているが、
/// 公式を正とする — ユーザー判断 2026-09-21。docs/damage-formula.md に記録)。
pub const FLAG_TICK_SECONDS: f64 = 1.0;

/// <フラグ> の持続時間(秒)。
pub const FLAG_DURATION_SECONDS: f64 = 120.0;

/// 連 / 爆 1 回で積む <フラグ> の数(wiki スキル性能一覧、2026-09-21 取得)。
/// ウルミだけ 4〜6 と幅があるので中央の 5 とみなす(docs/damage-formula.md に簡略化として記載)。
/// マスタリー4 のシャード系(極・シャードバリア / シャードスパイク / コンバインドシャード)が
/// 活性化している間は半減するが、その入力は未収録。
pub fn flag_stacks_per_use(form: SkillForm) -> u8 {
    match form {
        SkillForm::Urumi => 5,
        _ => 2,
    }
}

/// 爆発したあとに残る <フラグ>(wiki「Skill/イェフネン」#Flag、2026-09-21 取得)。
/// ウルミのスレイ / クラッシュは全消費せず**現在スタックの半分(切り捨て)**が残る。
/// 他の形態は全消費。
pub fn flag_stacks_left_after_burst(form: SkillForm, stacks: u8) -> u8 {
    match form {
        SkillForm::Urumi => stacks / 2,
        _ => 0,
    }
}

/// 次の爆発までに積み直す量 = スタック数 − 爆発で残る量。
pub fn flag_stacks_to_apply(form: SkillForm, stacks: u8) -> u8 {
    stacks - flag_stacks_left_after_burst(form, stacks)
}

/// 積み直しに使う技(同じ形態の 連 / 爆)。
///
/// 主軸が**単体**(スレイ)なら 連、**範囲**(クラッシュ)なら 爆 を選ぶ。形態と対象指定は
/// 技データの印(`Skill::form` / `Skill::target` / `Skill::applies_flag`)だけを見るので、
/// id の対応表を持たない。<フラグ> を積む技が無い(= 爆発させる技ではない)なら `None`。
pub fn flag_applier_for(skill: &Skill) -> Option<Skill> {
    let form = skill.form?;
    let target = skill.target?;
    crate::skills::all_skills()
        .find(|s| s.applies_flag && s.form == Some(form) && s.target == Some(target))
}

/// マスタリー3【鋭い欠片】: 持続 +20% / 爆発 −20%(<フラグ> にだけ効く E1)。
const SHARP_SHARD_MASTERY_ID: &str = "yefnen_m3_2";
/// マスタリー3【べたつく欠片】: 持続 −10%(敵の最終ダメージ −10% は防御側なので収録しない)。
const STICKY_SHARD_MASTERY_ID: &str = "yefnen_m3_3";

/// <フラグ> の 2 本のダメージ。どちらも 1 回ぶん。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagPart {
    /// 1 秒ごとに入る持続ダメージ(倍率 × 1 段、Cri倍率 2.0)
    Duration,
    /// スレイ / クラッシュで起きる爆発(倍率 × 5 段、Cri倍率 2.5)
    Burst,
}

impl FlagPart {
    pub fn label(self) -> &'static str {
        match self {
            FlagPart::Duration => "<フラグ> 持続",
            FlagPart::Burst => "<フラグ> 爆発",
        }
    }

    fn id_suffix(self) -> &'static str {
        match self {
            FlagPart::Duration => "duration",
            FlagPart::Burst => "burst",
        }
    }

    fn hit_count(self) -> u32 {
        match self {
            FlagPart::Duration => 1,
            FlagPart::Burst => 5,
        }
    }

    fn critical_multiplier(self) -> f64 {
        match self {
            FlagPart::Duration => 2.0,
            FlagPart::Burst => 2.5,
        }
    }
}

/// いま敵に付いている <フラグ> のスタック数。ON にしていなければ 0。
///
/// `FLAG_SKILL_ID` は自分のスキル(イェフネン)なので、`CharacterSkills::validate` が
/// 他キャラで ON になるのを弾く。commands 側にキャラ id の分岐を置かない。
pub fn flag_stacks(character_skills: &CharacterSkills) -> u8 {
    if !character_skills.skill_ids.iter().any(|id| id == FLAG_SKILL_ID) {
        return 0;
    }
    character_skill_catalog()
        .iter()
        .find(|d| d.id == FLAG_SKILL_ID)
        .map_or(0, |def| character_skills.level_of(def))
}

/// スタック数 → スキル倍率。0 や上限超えは `None`。
pub fn flag_multiplier(stacks: u8) -> Option<f64> {
    FLAG_MULTIPLIERS.get(usize::from(stacks).checked_sub(1)?).copied()
}

/// <フラグ> の技データ 1 本。`base` は <フラグ> を積んだ / 爆発させた技で、
/// **命中・Cri値・対象指定だけ**を引き継ぐ(<フラグ> 単独の出典が無いため)。
/// 依存・属性・倍率・段数・Cri倍率は <フラグ> のもの。スタック 0 なら `None`。
pub fn flag_skill(base: &Skill, stacks: u8, part: FlagPart) -> Option<Skill> {
    let multiplier = flag_multiplier(stacks)?;
    let hit_count = part.hit_count();
    Some(Skill {
        id: format!("{FLAG_SKILL_ID}_{}", part.id_suffix()),
        name: part.label().to_string(),
        // HACK 依存・物理・無属性(韓国公式「귀환」)
        dependency: SkillDependency::Hack,
        multiplier,
        hit_count,
        critical_multiplier: part.critical_multiplier(),
        element: Element::Neutral,
        weapon_classes: base.weapon_classes.clone(),
        target: base.target,
        accuracy: base.accuracy,
        critical_rate: base.critical_rate,
        level: base.level,
        single_target_channeling: false,
        // 別枠のダメージなので中ディレイを持たない。DPS は commands が
        // 周期(持続)/ 技 1 回の所要時間(爆発)で出す
        base_actual_delay: None,
        actual_delay_fixed: false,
        normal_attack: false,
        combo_interval: None,
        combo_variants: Vec::new(),
        power: Skill::compute_power(multiplier, hit_count),
        power_per_second: None,
        attacker: base.attacker,
        summon_form: None,
        // 形態限定の効果(速剣・後方攻撃・最大チャージ)を <フラグ> に効かせない
        form: None,
        swift_sword: None,
        full_charge: None,
        charge_seconds: 0.0,
        applies_flag: false,
        detonates_flag: false,
        cooldown_seconds: None,
    })
}

/// マスタリー3(欠片の三択)による <フラグ> 限定のスキル倍率増加(割合)= カテゴリ E1。
/// **技には一切効かない**ので、技の `damage_contributions_of` には混ぜない。
/// マスタリー側の定義は `RecordOnly` のままなので二重計上にならない。
pub fn flag_damage_contributions(masteries: &Masteries, part: FlagPart) -> Vec<DamageContribution> {
    let picked = |id: &str| masteries.picked.iter().any(|m| m == id);
    let mut out = Vec::new();
    let mut push = |source: &str, percent: f64| {
        out.push(DamageContribution {
            source: source.to_string(),
            category: DamageCategory::SkillMultiplierRate,
            value: percent / 100.0,
        });
    };
    if picked(SHARP_SHARD_MASTERY_ID) {
        push(
            "鋭い欠片",
            match part {
                FlagPart::Duration => 20.0,
                FlagPart::Burst => -20.0,
            },
        );
    }
    if picked(STICKY_SHARD_MASTERY_ID) && part == FlagPart::Duration {
        push("べたつく欠片", -10.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::find_skill;

    fn masteries(ids: &[&str]) -> Masteries {
        Masteries {
            picked: ids.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn スタックごとの倍率が出典と一致する() {
        // 韓国公式「귀환」の <フラグ> 行(1〜10 スタック)
        let expected = [320, 323, 326, 330, 336, 342, 349, 365, 381, 400];
        for (i, percent) in expected.iter().enumerate() {
            let stacks = u8::try_from(i + 1).unwrap();
            let multiplier = flag_multiplier(stacks).unwrap();
            assert!((multiplier * 100.0 - f64::from(*percent)).abs() < 1e-9, "stacks={stacks}");
        }
        assert_eq!(flag_multiplier(0), None);
        assert_eq!(flag_multiplier(11), None);
    }

    #[test]
    fn 持続と爆発の段数とcri倍率が出典どおり() {
        let base = find_skill("yefnen_slay").unwrap();
        let duration = flag_skill(&base, 10, FlagPart::Duration).unwrap();
        let burst = flag_skill(&base, 10, FlagPart::Burst).unwrap();
        assert!((duration.multiplier - 4.0).abs() < 1e-9);
        assert_eq!(duration.hit_count, 1);
        assert!((duration.critical_multiplier - 2.0).abs() < 1e-9);
        assert!((burst.multiplier - 4.0).abs() < 1e-9);
        assert_eq!(burst.hit_count, 5);
        assert!((burst.critical_multiplier - 2.5).abs() < 1e-9);
        // 形態限定の効果が乗らないように、形態の印は落とす
        for skill in [&duration, &burst] {
            assert_eq!(skill.form, None);
            assert_eq!(skill.swift_sword, None);
            assert_eq!(skill.full_charge, None);
            assert_eq!(skill.dependency, SkillDependency::Hack);
            assert_eq!(skill.element, Element::Neutral);
            assert_eq!(skill.base_actual_delay, None);
        }
    }

    #[test]
    fn 欠片のマスタリーはフラグのe1にだけ入る() {
        let rate = |ms: &Masteries, part| {
            flag_damage_contributions(ms, part)
                .iter()
                .map(|c| c.value)
                .sum::<f64>()
        };
        let none = masteries(&[]);
        assert_eq!(rate(&none, FlagPart::Duration), 0.0);
        assert_eq!(rate(&none, FlagPart::Burst), 0.0);

        let sharp = masteries(&[SHARP_SHARD_MASTERY_ID]);
        assert!((rate(&sharp, FlagPart::Duration) - 0.20).abs() < 1e-12);
        assert!((rate(&sharp, FlagPart::Burst) + 0.20).abs() < 1e-12);
        for part in [FlagPart::Duration, FlagPart::Burst] {
            assert!(flag_damage_contributions(&sharp, part)
                .iter()
                .all(|c| c.category == DamageCategory::SkillMultiplierRate));
        }

        let sticky = masteries(&[STICKY_SHARD_MASTERY_ID]);
        assert!((rate(&sticky, FlagPart::Duration) + 0.10).abs() < 1e-12);
        assert_eq!(rate(&sticky, FlagPart::Burst), 0.0);
    }

    #[test]
    fn 連と爆が積みスレイとクラッシュが爆発させる() {
        for id in ["yefnen_continuous", "yefnen_explosion_chisel"] {
            let skill = find_skill(id).unwrap();
            assert!(skill.applies_flag, "{id}");
            assert!(!skill.detonates_flag, "{id}");
        }
        for id in [
            "yefnen_slay",
            "yefnen_slay_pike",
            "yefnen_slay_axe",
            "yefnen_slay_urumi",
            "yefnen_slay_chisel",
            "yefnen_crash",
            "yefnen_crash_pike",
            "yefnen_crash_axe",
            "yefnen_crash_urumi",
            "yefnen_crash_chisel",
        ] {
            let skill = find_skill(id).unwrap();
            assert!(skill.detonates_flag, "{id}");
            assert!(!skill.applies_flag, "{id}");
        }
        // 他キャラの技には印が立たない
        let other = find_skill("lucian_butt").unwrap();
        assert!(!other.applies_flag && !other.detonates_flag);
    }

    /// 積み直しの材料(1 回で積む数・爆発で残る量・CT・積む技)が wiki と一致する。
    #[test]
    fn 積み直しの材料が出典どおり() {
        // 1 回で積む数: 連 / 爆は +2、ウルミだけ +4〜6(中央の 5 とみなす)
        for form in [SkillForm::Sword, SkillForm::Pike, SkillForm::Axe, SkillForm::Chisel] {
            assert_eq!(flag_stacks_per_use(form), 2, "{form:?}");
            // 全消費 → スタック数ぶん積み直す
            assert_eq!(flag_stacks_left_after_burst(form, 10), 0);
            assert_eq!(flag_stacks_to_apply(form, 10), 10);
        }
        assert_eq!(flag_stacks_per_use(SkillForm::Urumi), 5);
        // ウルミは爆発しても半分(切り捨て)残る
        assert_eq!(flag_stacks_left_after_burst(SkillForm::Urumi, 10), 5);
        assert_eq!(flag_stacks_to_apply(SkillForm::Urumi, 10), 5);
        assert_eq!(flag_stacks_left_after_burst(SkillForm::Urumi, 7), 3);
        assert_eq!(flag_stacks_to_apply(SkillForm::Urumi, 7), 4);

        // CT: スレイ / クラッシュ は全 5 形態とも 10 秒。連 / 爆 は CT なし
        for id in [
            "yefnen_slay", "yefnen_crash",
            "yefnen_slay_pike", "yefnen_crash_pike",
            "yefnen_slay_axe", "yefnen_crash_axe",
            "yefnen_slay_urumi", "yefnen_crash_urumi",
            "yefnen_slay_chisel", "yefnen_crash_chisel",
        ] {
            assert_eq!(find_skill(id).unwrap().cooldown_seconds, Some(10.0), "{id}");
        }
        for id in ["yefnen_continuous", "yefnen_explosion_urumi", "lucian_butt"] {
            assert_eq!(find_skill(id).unwrap().cooldown_seconds, None, "{id}");
        }
    }

    /// 積み直しに使う技は**同じ形態**の、主軸が単体なら 連、範囲なら 爆。
    #[test]
    fn 積み直しに使う技は同じ形態の連と爆() {
        for (main, applier) in [
            ("yefnen_slay", "yefnen_continuous"),
            ("yefnen_crash", "yefnen_explosion"),
            ("yefnen_slay_pike", "yefnen_continuous_pike"),
            ("yefnen_crash_axe", "yefnen_explosion_axe"),
            ("yefnen_slay_urumi", "yefnen_continuous_urumi"),
            ("yefnen_crash_chisel", "yefnen_explosion_chisel"),
        ] {
            let skill = find_skill(main).unwrap();
            assert_eq!(flag_applier_for(&skill).unwrap().id, applier, "{main}");
        }
        // 形態を持たない技には積み直す技が無い
        assert!(flag_applier_for(&find_skill("lucian_butt").unwrap()).is_none());
    }

    #[test]
    fn スタック数はキャラスキルの選択から引く() {
        let mut skills = CharacterSkills::default();
        assert_eq!(flag_stacks(&skills), 0);
        skills.skill_ids.push(FLAG_SKILL_ID.to_string());
        // 明示が無ければ上限(他のキャラスキルと同じ「ON = 満額」)
        assert_eq!(flag_stacks(&skills), 10);
        skills.skill_levels.insert(FLAG_SKILL_ID.to_string(), 3);
        assert_eq!(flag_stacks(&skills), 3);
    }
}
