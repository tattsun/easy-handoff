# easy-handoff

[English](README.md) | 日本語

AirPods や Sony・Bose などの Bluetooth イヤホン/ヘッドホンを、Windows のタスクトレイからワンクリックで接続/切断するツールです。
スマホと Windows でイヤホンを行き来させるときに、毎回 Bluetooth 設定を開いて「接続」を押す手間をなくします。

## インストール

<a href="https://apps.microsoft.com/detail/9pdh9f3fsq2s?mode=direct"><img src="https://get.microsoft.com/images/ja%20dark.svg" width="200" alt="Microsoft から入手"/></a>

[Microsoft Store](https://apps.microsoft.com/detail/9pdh9f3fsq2s) からインストールできます（おすすめ。自動で更新されます）。

または、[Releases](https://github.com/tattsun/easy-handoff/releases) から `easy-handoff-setup-x.y.z.exe` をダウンロードして実行してください（管理者権限不要）。
インストーラーを使わない場合は `easy-handoff-x.y.z-portable.exe` を好きな場所に置いて起動します。

> 事前に Windows の Bluetooth 設定で機器をペアリングしておく必要があります。

## 使い方

| 操作 | 動作 |
| --- | --- |
| 左クリック | 接続 / 切断を切り替え |
| 右クリック | メニュー（接続 / 切断、デバイス選択、スタートアップに登録、終了） |

アイコンの見かた:

- 青い丸: 接続中
- 灰色の輪: 切断中
- 回転する弧: 切り替え中（青 = 接続中、灰 = 切断中）

対象の機器は右クリックメニューの「デバイス」から選べます（未選択なら名前に `AirPods` を含む機器、なければ最初の Bluetooth オーディオ機器）。選んだ機器は次回起動時も引き継がれます。

表示言語は、Windows の表示言語が日本語なら日本語、それ以外なら英語になります。環境変数 `EASY_HANDOFF_LANG=en` / `ja` で切り替えられます。

### CLI

```
easy-handoff.exe list | connect | disconnect | toggle | tray  [デバイス名]
```

GUI アプリとしてビルドしているため、PowerShell で出力を見るには `easy-handoff.exe list | Out-String` のようにパイプしてください。

## 仕組み

Bluetooth 設定の「接続」ボタンと同じく、Bluetooth オーディオの KS フィルタに
`KSPROPSETID_BtAudio` の `KSPROPERTY_ONESHOT_RECONNECT` / `KSPROPERTY_ONESHOT_DISCONNECT` を送っています。

## 開発

ツールチェーンは [mise](https://mise.jdx.dev/) で管理しています。ビルドには Visual Studio Build Tools（MSVC）が必要です。

```
mise install
cargo build --release
```

インストーラーのビルドには [Inno Setup 6](https://jrsoftware.org/isinfo.php) を使います。

```
iscc /DAppVersion=0.1.0 installer\easy-handoff.iss
```

### リリース

`Cargo.toml` の `version` を更新してから、同じバージョンのタグを push すると GitHub Actions が Release を作ります。

```
git tag v0.1.0
git push origin v0.1.0
```

## ライセンス

[MIT](LICENSE)
