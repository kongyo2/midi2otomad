export function HelpPanel(): React.JSX.Element {
  return (
    <section className="panel help">
      <h2 className="panel__heading">ワークフロー</h2>
      <ol className="help__list">
        <li>
          <strong>.mid</strong> をドラッグ＆ドロップ → トラック / ノート / テンポを解析
        </li>
        <li>
          <strong>音声素材</strong>（WAV / MP3）を追加し、トラックに割り当て
        </li>
        <li>基準ピッチ・DAHDSR エンベロープ・音色フィルター・ダイナミックピッチ・リバーブを調整</li>
        <li>
          <strong>Space</strong> で再生、タイムラインのクリックでシーク
        </li>
        <li>WAV / MP3 に高音質で書き出し</li>
      </ol>
      <p className="help__note">
        🎹
        ノートの音高は素材の基準ピッチからの差分で再生速度を変えて発音します。再生は3次エルミート補間で高品質に。各ノートのベロシティは常に音量へ反映し、エクスプレッション(CC11)/ボリューム(CC7)
        の抑揚カーブはトラックごとに反映のオン・オフと深さを調整できます。ダイナミックピッチ（グライド・ビブラート）は素材ごとにオフにできます。ロングトーンはループ範囲で持続し、フィルターはアンプEG連動スイープと
        LFO ワブルで時間変化させられます。
      </p>
    </section>
  );
}
