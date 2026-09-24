// 「試し変更」(sim)— この計算だけの変更。保存はしない = ラベンダー(§02)。
// 何を 1 操作として戻すか(KNOBS)と、同時にいくつまで試せるか(§14 決定 6)をここが持つ。
// 画面(計算タブの右カラムの帯・印と、「足りない分をどう埋める?」)はこの 1 つを共有する。
import { errorMessage } from "../../api/commands";
import type { EquipmentValues, NewCharacter, UltimateSkill } from "../../api/types";
import { selectedWeapon } from "../../equipment";
import { t } from "../../i18n";
import { PART_SLOTS, PART_SLOT_LABELS, ULTIMATE_SKILL_LABELS } from "../../labels";
import {
  app, enqueueCharacterSave, payloadOf, selectedCharacter, simIsDirty, upsertCharacter,
} from "../../state.svelte";
import { reportError } from "../../toast.svelte";

/** 1 変更 = 1 印。✕ でその変更だけ戻せる単位で束ねる */
export interface Knob {
  id: string;
  label: (p: NewCharacter, saved: NewCharacter | null) => string;
  get: (p: NewCharacter) => string;
  set: (p: NewCharacter, v: string) => void;
}
export const KNOBS: Knob[] = [
  {
    id: "pw",
    label: (p) => `${t("パワーW")} ${p.common_skills.power_weapon ? "ON" : "OFF"}`,
    get: (p) => String(p.common_skills.power_weapon),
    set: (p, v) => (p.common_skills.power_weapon = v === "true"),
  },
  {
    id: "sw",
    label: (p) => `${t("ストロングW")} ${p.common_skills.strong_weapon_level > 0 ? `Lv${p.common_skills.strong_weapon_level}` : t("なし")}`,
    get: (p) => String(p.common_skills.strong_weapon_level),
    set: (p, v) => (p.common_skills.strong_weapon_level = Number(v)),
  },
  {
    // 部位・ステをまたぐので 1 チップに束ねる(全部位ぶんをまとめて 1 操作として戻す)
    // 登録 ID で持つ(装着中の 1 件ではなく)。装着を切り替えても、切り替え先のエンチャントが
    // 「変えた」ことにならないようにする — 切り替えは equipment_select の 1 チップで戻す
    id: "enchant",
    label: () => t("エンチャント"),
    get: (p) => JSON.stringify(PART_SLOTS.map((s) => p.equipment.parts[s].registered.map((x) => [x.id, x.enchant]))),
    set: (p, v) => {
      const values = JSON.parse(v) as [number, EquipmentValues][][];
      PART_SLOTS.forEach((s, i) => {
        for (const [id, enchant] of values[i]) {
          const part = p.equipment.parts[s].registered.find((x) => x.id === id);
          if (part) part.enchant = enchant;
        }
      });
    },
  },
  {
    // 登録済み装備の装着切り替え(部位ごとの selected_id)。全部位まとめて 1 チップで戻す
    id: "equipment_select",
    label: (p, saved) => {
      const changed = saved
        ? PART_SLOTS.filter((s) => p.equipment.parts[s].selected_id !== saved.equipment.parts[s].selected_id)
        : [];
      return `${t("装備切替")} ${changed.map((s) => PART_SLOT_LABELS[s]).join("・")}`.trim();
    },
    get: (p) => JSON.stringify(PART_SLOTS.map((s) => p.equipment.parts[s].selected_id)),
    set: (p, v) => {
      const ids = JSON.parse(v) as (number | null)[];
      PART_SLOTS.forEach((s, i) => (p.equipment.parts[s].selected_id = ids[i]));
    },
  },
  {
    id: "title",
    label: (p) => {
      const title = app.titles.find((x) => x.id === p.equipment.title);
      return t("称号 {name}", { name: title ? t(title.name) : t("なし") });
    },
    get: (p) => String(p.equipment.title),
    set: (p, v) => (p.equipment.title = v === "null" ? null : v),
  },
  {
    id: "ultimate",
    label: (p) =>
      t("極限 {v}", {
        v: p.common_skills.ultimate.slots
          .filter((s): s is UltimateSkill => s !== null)
          .map((s) => ULTIMATE_SKILL_LABELS[s])
          .join("・") || t("未選択"),
      }),
    get: (p) => JSON.stringify(p.common_skills.ultimate.slots),
    set: (p, v) => (p.common_skills.ultimate.slots = JSON.parse(v)),
  },
  {
    // 覚醒段階とエタの意志 Lv は 1 つの育ち方(エタは覚醒 5 の先)なので 1 チップで戻す
    id: "awakening",
    label: (p) => t("覚醒 {stage} / エタ Lv{lv}", { stage: p.awakening.stage, lv: p.awakening.eternal_level }),
    get: (p) => JSON.stringify(p.awakening),
    set: (p, v) => (p.awakening = JSON.parse(v)),
  },
  {
    id: "sharpness",
    label: (p) =>
      p.common_skills.sharpness_vision_level > 0
        ? t("シャープネス Lv{lv}", { lv: p.common_skills.sharpness_vision_level })
        : t("シャープネス 未習得"),
    get: (p) => String(p.common_skills.sharpness_vision_level),
    set: (p, v) => (p.common_skills.sharpness_vision_level = Number(v)),
  },
  {
    // リンクステータスは 8 種まとめて 1 チップ。この画面で触るのはダメージ式に効く 5〜7 だけ
    id: "soul_link",
    label: () => t("ソウルリンク"),
    get: (p) => JSON.stringify(p.stat_sources.soul_link),
    set: (p, v) => (p.stat_sources.soul_link = JSON.parse(v)),
  },
  {
    // 「次に変えるなら」の武器更新。基本値まで一緒に替わる 1 操作なので 1 チップで戻す。
    id: "weapon_item",
    label: (p) => {
      const weapon = selectedWeapon(p);
      const catalogName = app.equipmentCatalog.find((i) => i.id === weapon.item_id)?.name;
      const name = catalogName ? t(catalogName) : (weapon.custom_name ?? t("未装着"));
      return t("武器 {name}", { name });
    },
    // 登録 ID ごとに持つ(装着の切り替えで「武器が変わった」ことにしない。enchant と同じ)
    get: (p) => JSON.stringify(p.equipment.parts.weapon.registered.map((w) => [w.id, w.item_id, w.custom_name, w.base])),
    set: (p, v) => {
      for (const [id, itemId, customName, base] of JSON.parse(v) as [number, string | null, string | null, EquipmentValues][]) {
        const weapon = p.equipment.parts.weapon.registered.find((w) => w.id === id);
        if (!weapon) continue;
        weapon.item_id = itemId;
        weapon.custom_name = customName;
        weapon.base = base;
      }
    },
  },
  // 以下は計算タブの編集 UI からは変わらないが、sim が他の経路で差分を持ったときに
  // 「試し変更中なのにチップが空」にならないよう網羅する(独立レビュー指摘)。
  {
    id: "base_stats",
    label: () => t("素ステータス"),
    get: (p) => JSON.stringify(p.base_stats),
    set: (p, v) => (p.base_stats = JSON.parse(v)),
  },
  {
    id: "permanent",
    label: () => t("恒常補正(ペット/ルーン/クラウン/聖物)"),
    get: (p) =>
      JSON.stringify([
        p.stat_sources.pet_skills,
        p.stat_sources.rune_levels,
        p.stat_sources.crown,
        p.stat_sources.sacred_relic,
      ]),
    set: (p, v) => {
      const [pet, rune, crown, relic] = JSON.parse(v);
      p.stat_sources.pet_skills = pet;
      p.stat_sources.rune_levels = rune;
      p.stat_sources.crown = crown;
      p.stat_sources.sacred_relic = relic;
    },
  },
  {
    id: "identity",
    label: (p) => t("名前・キャラ種({name})", { name: p.name }),
    get: (p) => JSON.stringify([p.name, p.game_character_id]),
    set: (p, v) => {
      const [name, gid] = JSON.parse(v);
      p.name = name;
      p.game_character_id = gid;
    },
  },
];
/**
 * 同時に試せる変更の上限(design-system §14 決定 6)。
 * 5〜6 個同時に動くとチップ列が読めなくなる。「試しセットに名前を付けて保存」に逃げると
 * ラベンダー = 保存されない の意味が壊れるので、機能側に制約を置く。
 * 上限に達したことは**色ではなく文言**で伝える(ラベンダーに 2 つ目の意味を持たせない)。
 */
export const SIM_LIMIT = 3;

export class SimStore {
  /** 上限で弾いた直後だけ立てる。次の操作が通ったら下ろす */
  limited = $state(false);
  saving = $state(false);
  /** 「ぜんぶ戻す」を押した回数。伸びしろの土台(登録値)が戻るので、
   *  エンチャント一覧の「見えたことがある」記録を撮り直す合図になる */
  resetCount = $state(0);

  character = $derived(selectedCharacter());
  saved = $derived(this.character ? payloadOf(this.character) : null);
  /** いま計算に使うキャラ。試し変更中はその値 */
  payload = $derived(app.sim ?? this.saved);
  // 保存値との差分があるかどうかの判定は state.svelte.ts の simIsDirty に一本化
  // (JSON.stringify の比較をここでもう一度書かない。上部バーと同じ関数を読む)。
  dirty = $derived(simIsDirty());
  /** いま変えているもの(1 変更 = 1 印、✕ でその変更だけ戻す) */
  changed = $derived(
    app.sim !== null && this.saved !== null
      ? KNOBS.filter((k) => k.get(app.sim!) !== k.get(this.saved!))
      : [],
  );

  /** 上限を超える変更は載せない(弾いたことは文言で伝える) */
  private commit(p: NewCharacter): void {
    const saved = this.saved;
    if (saved !== null && KNOBS.filter((k) => k.get(p) !== k.get(saved)).length > SIM_LIMIT) {
      this.limited = true;
      return;
    }
    this.limited = false;
    app.sim = p;
  }

  edit(fn: (p: NewCharacter) => void) {
    const base = this.payload;
    if (!base) return;
    const p = JSON.parse(JSON.stringify(base)) as NewCharacter;
    fn(p);
    this.commit(p);
  }

  /** 「もし〜だったら」の候補を当てる(列挙時点の payload + 候補 1 件ぶんの変更) */
  applyCandidate(applied: NewCharacter) {
    this.commit(applied);
  }

  reset() {
    app.sim = null;
    this.limited = false;
    this.resetCount += 1;
  }

  revert(k: Knob) {
    const saved = this.saved;
    if (!saved || !app.sim) return;
    const p = JSON.parse(JSON.stringify(app.sim)) as NewCharacter;
    k.set(p, k.get(saved));
    app.sim = JSON.stringify(p) === JSON.stringify(saved) ? null : p;
    this.limited = false;
  }

  async save() {
    const character = this.character;
    if (!character || !app.sim) return;
    this.saving = true;
    try {
      // 試し変更の保存もキャラ単位の保存キューへ通す(ホームの直更新・キャラタブの保存と
      // 同じ full-overwrite なので、直列化しないと互いの変更を巻き戻す)。payload はユーザーが
      // 明示した試し変更のスナップショットなので、ここで確定させてからキューに載せる。
      const payload = JSON.parse(JSON.stringify(app.sim)) as NewCharacter;
      const saved = await enqueueCharacterSave(character.id, () => payload);
      upsertCharacter(saved);
      app.sim = null;
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      this.saving = false;
    }
  }
}
