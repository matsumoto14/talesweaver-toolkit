# リリース手順

## 初回だけ必要な設定

### 1. 自動更新の署名鍵

```sh
cd apps/desktop
npx tauri signer generate -w ~/.tauri/talesweaver-toolkit.key
```

- **公開鍵**(`.pub` の中身)→ `tauri.conf.json` の `plugins.updater.pubkey` に置く
- **秘密鍵**(`.key` の中身)→ GitHub の Secrets に `TAURI_SIGNING_PRIVATE_KEY`
- パスフレーズ → `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

秘密鍵はリポジトリに入れない。失うと**既存ユーザーへ更新を配れなくなる**ので別途保管する。

### 2. コード署名(Azure Trusted Signing)

Azure ポータルで Trusted Signing アカウントと証明書プロファイルを作り、
サービスプリンシパルに「Trusted Signing Certificate Profile Signer」ロールを与える。

GitHub の **Secrets** に:

| 名前 | 中身 |
|---|---|
| `AZURE_CLIENT_ID` | サービスプリンシパルのアプリ ID |
| `AZURE_CLIENT_SECRET` | そのシークレット |
| `AZURE_TENANT_ID` | テナント ID |

GitHub の **Variables** に:

| 名前 | 例 |
|---|---|
| `AZURE_SIGNING_ENDPOINT` | `https://eus.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT` | Trusted Signing アカウント名 |
| `AZURE_SIGNING_PROFILE` | 証明書プロファイル名 |

`AZURE_CLIENT_ID` が空のときは署名ステップを飛ばして未署名でビルドする
(ローカルや設定前でもワークフローが通る)。

## 毎回の手順

```sh
python tools/release/bump.py 0.2.0        # 3 ファイルの版を揃える
# CHANGELOG.md の [未リリース] を [0.2.0] — 2026-09-15 に書き換える
git commit -am "release: v0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

タグを push すると Actions がビルドして **下書きの Release** を作る。
中身(インストーラが署名されているか、`latest.json` があるか)を確認してから公開する。
公開した瞬間に、既存ユーザーの次回起動で更新のお知らせが出る。

## リリースの間隔

goal ごとにタグを打たず、**2〜4 週で 1 本**に束ねる。
0.x のうちは更新が多く、毎回お知らせが出るとユーザーが疲れるため。

## 版の意味

- `0.MINOR.PATCH` — MINOR が機能追加、PATCH が修正
- `1.0.0` — ダメージ計算・キャラ管理・強化提案・ロードマップ・索引の 5 機能がそろい、
  ゲーム内の実測値と突き合わせて検証できた時点

## 注意

**このアプリは後方互換を保たない**(AGENTS.md)。新しい版で開いた DB は古い版では開けない。
起動時にマイグレーション前の状態を自動バックアップしているが(直近 3 世代)、
**ダウングレードの手順をユーザーに案内する予定がないなら、DB スキーマを変える版は慎重に出す**。
