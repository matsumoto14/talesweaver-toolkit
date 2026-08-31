//! 画面(TypeScript)とブラウザ版の入口(`lib.rs`)で、コマンド名と引数名が食い違っていないかを
//! テストで突き合わせる。
//!
//! この 2 つは JSON を挟んで繋がっているので、`invoke("preview_damage", { skilId })` のような
//! 打ち間違いは Rust の型検査を素通りし、その画面を開いた人だけが実行時エラーに出会う。
//! デスクトップ版は Tauri がコマンドを解決するので気づけず、静かに壊れるのはブラウザ版だけになる。
//!
//! やり方は「全コマンドを一度ずつ実際に呼ぶ」ではなく、名前の突き合わせに留めている。実呼び出しには
//! `NewCharacter` のような完全な入力一式が要り、そのフィクスチャ自体が古びて別の嘘をつくため。
//! ここで見たいのは計算結果ではなく名前の一致なので、
//! - Rust 側は serde に「この引数 struct が期待するフィールド名」を直接吐かせる(`arg_fields`)
//! - 画面側は `commands.ts` / `invoke.wasm.ts` の呼び出しを読んで実際に渡すキーを取る
//! の 2 つを集合として比べる。`cargo test --workspace` で毎回走る。

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;

const COMMANDS_TS: &str = include_str!("../../../apps/desktop/src/api/commands.ts");
const INVOKE_WASM_TS: &str = include_str!("../../../apps/desktop/src/api/invoke.wasm.ts");
const LIB_RS: &str = include_str!("lib.rs");

// ---------------------------------------------------------------- Rust 側の引数名

/// serde の derive は `deserialize_struct` に「期待するフィールド名の一覧」を渡してくる。
/// `rename_all = "camelCase"` 適用後の名前なので、画面が渡すキーとそのまま比べられる。
struct FieldsProbe<'a>(&'a mut Vec<String>);

/// 名前を受け取った時点で用は済むので、値を作らずに打ち切るための番兵。
#[derive(Debug)]
struct Stop;

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("フィールド名を取り終えた")
    }
}
impl std::error::Error for Stop {}
impl serde::de::Error for Stop {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Stop
    }
}

impl<'de> serde::Deserializer<'de> for FieldsProbe<'_> {
    type Error = Stop;

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Stop> {
        self.0.extend(fields.iter().map(|f| (*f).to_string()));
        Err(Stop)
    }

    fn deserialize_any<V: serde::de::Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Stop> {
        Err(Stop)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

fn arg_fields<T: DeserializeOwned>() -> BTreeSet<String> {
    let mut fields = Vec::new();
    let _ = T::deserialize(FieldsProbe(&mut fields));
    assert!(!fields.is_empty(), "引数 struct のフィールド名を取れなかった");
    fields.into_iter().collect()
}

/// コマンドごとに、Rust 側が期待する引数名。`None` は「ここに載っていない」= 登録漏れ。
/// 引数を取らないコマンドは空集合。
fn expected_args(command: &str) -> Option<BTreeSet<String>> {
    use super::*;
    let fields = match command {
        // 静的データのカタログ(引数なし)
        "list_game_characters"
        | "list_enemies"
        | "list_buff_catalog"
        | "list_element_sources"
        | "list_contents"
        | "list_equipment_catalog"
        | "list_equipment_abilities"
        | "list_random_options"
        | "list_masteries"
        | "list_siena_kinds"
        | "list_character_skills"
        | "list_titles"
        | "get_stat_limits"
        | "get_new_character_stat_sources" => BTreeSet::new(),

        "list_skills" => arg_fields::<ListSkillsArgs>(),
        "summarize_buff_selection" => arg_fields::<BuffsArgs>(),
        "equipment_element_values" => arg_fields::<EquipmentElementValuesArgs>(),
        "preview_elements" | "validate_character" => arg_fields::<CharacterArgs>(),
        "validate_buff_set" => arg_fields::<ValidateBuffSetArgs>(),
        "resolve_character_skill_effects" => arg_fields::<MasteriesArgs>(),
        "preview_effective_stats" => arg_fields::<PreviewEffectiveStatsArgs>(),
        "buff_target_stat_gains" => arg_fields::<BuffTargetStatGainsArgs>(),
        "preview_defense" => arg_fields::<PreviewDefenseArgs>(),
        "preview_damage" => arg_fields::<PreviewDamageArgs>(),
        "evaluate_contents" => arg_fields::<EvaluateContentsArgs>(),
        "list_upgrade_candidates" | "list_enchant_gains" => arg_fields::<CandidateArgs>(),

        _ => return None,
    };
    Some(fields)
}

// ---------------------------------------------------------------- TypeScript 側の読み取り

fn flush(token: &mut String, expecting_key: bool, keys: &mut Vec<String>) {
    if expecting_key && !token.is_empty() {
        keys.push(token.clone());
    }
    token.clear();
}

/// `{` から始まるオブジェクトリテラルの、最上位のキーだけを拾う。
/// 値の中の `,` や括弧に釣られないよう深さを数え、文字列は読み飛ばす。
fn object_keys(src: &str, brace: usize) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut keys = Vec::new();
    let mut token = String::new();
    let mut expecting_key = true;
    let mut depth = 0usize;
    let mut i = brace + 1;
    while i < bytes.len() {
        match bytes[i] as char {
            quote @ ('"' | '\'' | '`') => {
                i += 1;
                while i < bytes.len() && bytes[i] as char != quote {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                expecting_key = false;
            }
            '{' | '[' | '(' => {
                depth += 1;
                expecting_key = false;
            }
            ']' | ')' => depth = depth.saturating_sub(1),
            '}' if depth == 0 => {
                flush(&mut token, expecting_key, &mut keys);
                break;
            }
            '}' => depth -= 1,
            ',' if depth == 0 => {
                flush(&mut token, expecting_key, &mut keys);
                expecting_key = true;
            }
            ':' if depth == 0 => {
                flush(&mut token, expecting_key, &mut keys);
                expecting_key = false;
            }
            c if expecting_key && (c.is_ascii_alphanumeric() || c == '_') => token.push(c),
            _ => {}
        }
        i += 1;
    }
    keys
}

/// `("コマンド名"` の位置から、そのコマンド名と引数オブジェクトのキーを取る。
/// 引数を渡していない呼び出しは空のキー集合になる。
fn call_at(src: &str, open_paren: usize) -> Option<(String, BTreeSet<String>)> {
    let rest = &src[open_paren + 1..];
    let name_end = rest[1..].find('"')? + 1;
    let name = &rest[1..name_end];
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
        return None;
    }
    let after = rest[name_end + 1..].trim_start();
    let keys = match after.strip_prefix(',') {
        Some(args) => {
            let args = args.trim_start();
            if !args.starts_with('{') {
                return None;
            }
            object_keys(src, src.len() - args.len())
        }
        None => Vec::new(),
    };
    Some((name.to_string(), keys.into_iter().collect()))
}

/// TS ファイル中の `("コマンド名", { ... })` をすべて拾う。
/// 同じコマンドを複数箇所から呼んでいれば、その数だけ返す(どれも一致していてほしい)。
fn calls_in(src: &str) -> Vec<(String, BTreeSet<String>)> {
    src.match_indices("(\"")
        .filter_map(|(i, _)| call_at(src, i))
        .collect()
}

/// ブラウザ版で TS 側(IndexedDB)が受け持つコマンド。`invoke.wasm.ts` の `stored` の見出し。
fn stored_commands() -> BTreeSet<String> {
    let start = INVOKE_WASM_TS
        .find("const stored")
        .expect("invoke.wasm.ts に stored が無い");
    let block = &INVOKE_WASM_TS[start..];
    let end = block.find("\n};").expect("stored の閉じが見つからない");
    block[..end]
        .lines()
        .filter_map(|line| {
            let name = line.strip_prefix("  ")?.split_once(':')?.0;
            (!name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .then(|| name.to_string())
        })
        .collect()
}

/// `lib.rs` の `match command` が受けているコマンド名。
fn dispatched_commands() -> BTreeSet<String> {
    LIB_RS
        .lines()
        .filter_map(|line| {
            let arm = line.strip_prefix("        \"")?;
            let name = arm.split_once('"')?.0;
            arm.contains("=>").then(|| name.to_string())
        })
        .collect()
}

// ---------------------------------------------------------------- テスト

#[test]
fn 画面が呼ぶコマンドはすべて受け口がある() {
    let dispatched = dispatched_commands();
    let stored = stored_commands();
    let missing: BTreeSet<_> = calls_in(COMMANDS_TS)
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| !dispatched.contains(name) && !stored.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "commands.ts が呼ぶのに、WASM の match にも invoke.wasm.ts の stored にも無い: {missing:?}"
    );

    // 逆向き。どこからも呼ばれない match の腕は、改名の取り残しを疑う
    let unused: BTreeSet<_> = dispatched
        .iter()
        .filter(|name| {
            let quoted = format!("\"{name}\"");
            !COMMANDS_TS.contains(&quoted) && !INVOKE_WASM_TS.contains(&quoted)
        })
        .collect();
    assert!(unused.is_empty(), "誰も呼ばない match の腕がある: {unused:?}");
}

#[test]
fn 画面が渡す引数名と受け口の引数名が一致する() {
    let stored = stored_commands();
    let mut calls: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
    for (name, keys) in calls_in(COMMANDS_TS)
        .into_iter()
        .chain(calls_in(INVOKE_WASM_TS))
    {
        // 保存が要るコマンドは TS 側で完結するので、Rust の引数 struct が無い
        if stored.contains(&name) {
            continue;
        }
        calls.entry(name).or_default().push(keys);
    }
    assert!(
        !calls.is_empty(),
        "呼び出しを 1 つも読み取れていない(読み取りが壊れた疑い)"
    );

    let mut problems = Vec::new();
    for (command, callsites) in calls {
        let Some(expected) = expected_args(&command) else {
            problems.push(format!("{command}: args_check.rs の expected_args に未登録"));
            continue;
        };
        for keys in callsites {
            if keys != expected {
                problems.push(format!(
                    "{command}: 画面が渡すのは {keys:?} / Rust が待つのは {expected:?}"
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "引数名が食い違っている:\n{}",
        problems.join("\n")
    );
}
