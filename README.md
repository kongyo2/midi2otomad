# midi2otomad

単体で完結する GUI を備えた、MIDI 音MAD作成デスクトップアプリ。
`.mid` を読み込み、トラックごとに WAV/MP3 などの音声素材を割り当て、ノートの音高に合わせて
ピッチ（再生速度）を計算して発音し、1 本の高音質 WAV / MP3 として一瞬で書き出します。

**純 Rust / Tauri 2** 製。合成エンジン・MIDI/音声入出力からフロントエンドまで、すべて Rust で書かれています。

## 構成

Cargo ワークスペースの 3 クレート構成です。

| クレート | 役割 |
| --- | --- |
| `core` (`midi2otomad-core`) | GUI 非依存のドメインロジック。プロジェクトスキーマ、音楽理論、音声 DSP（エンベロープ・フィルター・リバーブ・ピッチ変調・ボイス管理・オフラインミキサー）、MIDI 取り込み（midly）、音声デコード（symphonia）/エンコード（WAV・libmp3lame）。 |
| `src-tauri` (`midi2otomad`) | Tauri 2 バックエンド。デコード済み音声バンクと cpal 再生エンジンを保持し、フロントエンドからのコマンドを処理する。 |
| `ui` (`midi2otomad-ui`) | Leptos (Rust → WASM) フロントエンド。`window.__TAURI__` 経由でバックエンドのコマンドを呼ぶ。 |

## 開発

### 前提

- Rust (stable) と `wasm32-unknown-unknown` ターゲット
  ```sh
  rustup target add wasm32-unknown-unknown
  ```
- [Trunk](https://trunkrs.dev/) と Tauri CLI
  ```sh
  cargo install --locked trunk tauri-cli
  ```
- Linux のみ: WebKitGTK / GTK / ALSA などのシステムライブラリ
  ```sh
  sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev libasound2-dev
  ```

### 実行・ビルド

```sh
cargo tauri dev      # 開発（trunk serve + ウィンドウ起動）
cargo tauri build    # 配布用にビルド
```

### テスト・lint

```sh
cargo test -p midi2otomad-core                                  # DSP / スキーマ / MIDI / 入出力のテスト
cargo clippy -p midi2otomad-core --all-targets -- -D warnings
cargo clippy -p midi2otomad-ui --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all --check
```
