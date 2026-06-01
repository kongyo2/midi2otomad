# midi2otomad

単体で完結する GUI を備えた、MIDI 音MAD（YTPMV）作成デスクトップアプリ。
`.mid` を読み込み、トラックごとに WAV/MP3 などの音声素材を割り当て、ノートの音高に合わせて
ピッチ（再生速度）を計算して発音し、1 本の高音質 WAV / MP3 として一瞬で書き出します。

## 主な機能

- **MIDI 読み込み**: `.mid` をドラッグ＆ドロップ。トラック / ノート（音高・タイミング・長さ）/
  テンポ / エクスプレッション(CC11)・ボリューム(CC7) を解析。
- **音声素材の割り当て**: トラック単位、さらに **ノート番号単位** で別々の素材を割り当て可能
  （ドラムキットや音域別の貼り替えに対応）。
- **基準ピッチ / 微調整**: 素材の元音高を設定し、MIDI ノートとの差分で再生速度を自動計算。
- **エンベロープ**: アタック / リリースをミリ秒単位で調整し、出だしと余韻を滑らかに。
- **ループ**: ロングトーン対応。素材の「ここからここまで」をループ範囲として指定。
- **ベロシティ + エクスプレッション**を音量にリアルタイム反映。
- **トランスポート**: 再生 / 一時停止 / 停止、タイムラインのクリックでシーク、Space で再生切替。
- **書き出し**: 等倍再生ではなく内部処理で一括ミックスし、WAV（16/24-bit, 32-bit float）または
  MP3（最大 320kbps / libmp3lame）で出力。

## アーキテクチャ

プレビューと書き出しは **同一の純 TypeScript ミキサー**（`src/shared/audio/mixer.ts`）で
レンダリングするため、聴いた音とそのまま同じものが書き出されます（WYSIWYG）。

- `src/shared` — プロジェクトのデータモデル（zod）、ピッチ計算、オフラインミキサー（純粋関数・テスト可能）。
- `src/main` — Electron メインプロセス。ファイルダイアログと、WAV（自前実装）/ MP3（node-av = FFmpeg）エンコード。
- `src/preload` — `contextBridge` 経由の型付き IPC。
- `src/renderer` — React 19 の UI。Web Audio による再生エンジン、ピアノロール、素材エディタ。

## 開発

```bash
npm install
npm run dev          # アプリを起動
npm run typecheck    # 型チェック（node + web）
npm run lint:strict  # 静的解析
npm run format:check # 整形チェック
npm run selftest     # ミキサー / MIDI 解析 / WAV・MP3 書き出しのヘッドレス検証
npm run build        # 本番ビルド
npm run package      # 配布パッケージ生成（electron-builder）
```
