//! `CharacterSkillDef` / `BuffDefinition` のうち出典が公式お知らせ(`talesweaver.nexon.co.jp`)の
//! ものを、wiki 取込の「訂正(confirmed)」ユニットとして JSON 配列で stdout に出す。
//!
//! ADR-017(出典の格付け: 公式お知らせ > Tale Wiki)・docs/adr/020-wiki-ask.md の段階 1・
//! scratchpad/design/stage1-spec.md の 13 を実装する。値は gamedata の効果値から機械で作り、
//! 手では書かない。ここで生成した wiki 値との食い違い(`wiki_value`)だけは、ADR-017 の表に
//! 書いてある確認済みの値をコード上の定数として持つ(表自体が一次資料)。
//!
//!     cargo run -p gamedata --bin export_corrections > tools/gamedata/wiki/corrections.json

use domain::{BuffDefinition, CharacterSkillDef, DamageCategory, SkillEffect};
use gamedata::{buff_catalog, character_skill_catalog, characters};
use serde::Serialize;

const NOTICE_DOMAIN: &str = "talesweaver.nexon.co.jp";

/// 消費アイテムのバフが載る wiki ページ(docs/adr/017)。
const ITEM_BUFF_PAGE: &str = "Item/消耗品/ステータス補助";

/// キャラスキルの効果列(wiki Skill/<キャラ名> の Summary 表の列名。2026-09-22 にローカル D1 で実測:
/// `SELECT id, cells FROM unit WHERE page='Skill/マキシミン' AND text LIKE '%毒舌%'` 等。
/// wiki のソースでは `解説(COLOR(blue){自身・味方}/COLOR(red){敵})` で、units.py が装飾を落とした形)。
const CHARACTER_SKILL_EFFECT_COL: &str = "解説(自身・味方/敵)";

/// 訂正の対象が持たない列名の既定(spec 13: 「無ければ `効果`」)。
const DEFAULT_COL: &str = "効果";

#[derive(Serialize)]
struct Correction {
    subject: String,
    page: String,
    #[serde(rename = "match")]
    match_: String,
    col: String,
    value: String,
    wiki_value: Option<String>,
    source_kind: &'static str,
    source_title: String,
    source_url: String,
}

fn is_notice(source_url: &str) -> bool {
    source_url.contains(NOTICE_DOMAIN)
}

/// URL の `no=NNNNN` からお知らせのタイトルを機械で作る(手書きしない)。
fn notice_title(source_url: &str) -> String {
    let no = source_url
        .split("no=")
        .nth(1)
        .unwrap_or("?")
        .split(['&', '#'])
        .next()
        .unwrap_or("?");
    format!("テイルズウィーバー公式お知らせ(no={no})")
}

/// マスタリー変種の suffix(`【暴言】` 等)を落とし、wiki の行を見つける文字列にする。
fn strip_mastery_suffix(name: &str) -> String {
    name.split('【').next().unwrap_or(name).trim().to_string()
}

/// `SkillEffect::Damage` / `DamagePerLevel` の `TakenDamageReduction` を
/// 「敵被ダメージ +X%」に読み替える(ADR-017: Σ が負値のとき 1-Σ = 敵被ダメージ増加)。
/// それ以外のカテゴリは今のところ NOTICE_153335 のデータに出てこないので、種別と値をそのまま出す。
fn describe_damage(category: DamageCategory, percent: f64) -> String {
    match category {
        DamageCategory::TakenDamageReduction => {
            let increase = -percent;
            let sign = if increase >= 0.0 { "+" } else { "" };
            format!("敵被ダメージ {sign}{increase}%")
        }
        other => {
            let sign = if percent >= 0.0 { "+" } else { "" };
            format!("{other:?} {sign}{percent}%")
        }
    }
}

fn describe_skill_effects(effects: &[SkillEffect]) -> Option<String> {
    let parts: Vec<String> = effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::Damage { category, percent } => Some(describe_damage(*category, *percent)),
            SkillEffect::DamagePerLevel { category, percent } => {
                Some(format!("{} (SLv ×)", describe_damage(*category, *percent)))
            }
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" / "))
    }
}

fn describe_buff(buff: &BuffDefinition) -> Option<String> {
    if buff.element_bonus != 0 {
        return Some(format!("全属性 +{}", buff.element_bonus));
    }
    describe_skill_effects(buff.damage_effects)
}

/// ADR-017 の表に書いてある、確認済みの wiki 側の値(訂正前)。無いものは None。
fn known_wiki_value(id: &str) -> Option<&'static str> {
    match id {
        "maximin_invective_abuse" => Some("最終ダメージ −10%"),
        "benya_force_of_rubert" => Some("能力値倍率B −10%"),
        _ => None,
    }
}

fn character_skill_correction(def: &CharacterSkillDef) -> Option<Correction> {
    if !is_notice(def.source_url) {
        return None;
    }
    let value = describe_skill_effects(def.effects)?;
    let char_name = characters()
        .iter()
        .find(|c| c.id == def.game_character_id)
        .map(|c| c.name)
        .unwrap_or(def.game_character_id);
    Some(Correction {
        subject: def.name.to_string(),
        page: format!("Skill/{char_name}"),
        match_: strip_mastery_suffix(def.name),
        col: CHARACTER_SKILL_EFFECT_COL.to_string(),
        value,
        wiki_value: known_wiki_value(def.id).map(str::to_string),
        source_kind: "notice",
        source_title: notice_title(def.source_url),
        source_url: def.source_url.to_string(),
    })
}

fn buff_correction(def: &BuffDefinition) -> Option<Correction> {
    if !is_notice(def.source_url) {
        return None;
    }
    let value = describe_buff(def)?;
    Some(Correction {
        subject: def.name.to_string(),
        page: ITEM_BUFF_PAGE.to_string(),
        match_: def.name.to_string(),
        col: DEFAULT_COL.to_string(),
        value,
        wiki_value: known_wiki_value(def.id).map(str::to_string),
        source_kind: "notice",
        source_title: notice_title(def.source_url),
        source_url: def.source_url.to_string(),
    })
}

fn main() {
    let mut out: Vec<Correction> = Vec::new();
    out.extend(character_skill_catalog().iter().filter_map(character_skill_correction));
    out.extend(buff_catalog().iter().filter_map(|d| buff_correction(d)));

    let json = serde_json::to_string_pretty(&out).expect("corrections を JSON にできませんでした");
    println!("{json}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_parses_as_json_array() {
        let mut out: Vec<Correction> = Vec::new();
        out.extend(character_skill_catalog().iter().filter_map(character_skill_correction));
        out.extend(buff_catalog().iter().filter_map(|d| buff_correction(d)));
        let json = serde_json::to_string(&out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert!(!parsed.as_array().unwrap().is_empty());
    }

    #[test]
    fn strips_mastery_suffix() {
        assert_eq!(strip_mastery_suffix("毒舌【暴言】"), "毒舌");
        assert_eq!(strip_mastery_suffix("そよ風"), "そよ風");
    }
}
