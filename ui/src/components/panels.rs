use leptos::prelude::*;

use crate::format::{format_db, format_rate, pct};
use crate::state::Studio;
use crate::widgets::range_row;

const SAMPLE_RATES: [i32; 4] = [44100, 48000, 88200, 96000];

#[component]
pub fn ReverbPanel() -> impl IntoView {
    let s = expect_context::<Studio>();
    let enabled = Signal::derive(move || s.project.with(|p| p.reverb.enabled));

    view! {
        <section class="panel">
            <div class="panel__head">
                <h2 class="panel__heading">"マスターリバーブ"</h2>
                <label class="checkline">
                    <input
                        type="checkbox"
                        prop:checked=move || enabled.get()
                        on:change=move |ev| {
                            let c = event_target_checked(&ev);
                            s.project.update(|p| p.reverb.enabled = c);
                            s.mark_dirty();
                        }
                    />
                    "有効"
                </label>
            </div>
            <p class="panel__muted small">
                "トラックの「リバーブ送り」で各楽器の残響量を調整できます。"
            </p>
            <div class="grid2">
                {range_row("ルームサイズ", Signal::derive(move || s.project.with(|p| p.reverb.room_size)), 0.0, 1.0, 0.01, pct, move |v| { s.project.update(|p| p.reverb.room_size = v); s.mark_dirty(); })}
                {range_row("ダンピング", Signal::derive(move || s.project.with(|p| p.reverb.damping)), 0.0, 1.0, 0.01, pct, move |v| { s.project.update(|p| p.reverb.damping = v); s.mark_dirty(); })}
                {range_row("ステレオ幅", Signal::derive(move || s.project.with(|p| p.reverb.width)), 0.0, 1.0, 0.01, pct, move |v| { s.project.update(|p| p.reverb.width = v); s.mark_dirty(); })}
                {range_row("ウェット量", Signal::derive(move || s.project.with(|p| p.reverb.wet)), 0.0, 1.0, 0.01, pct, move |v| { s.project.update(|p| p.reverb.wet = v); s.mark_dirty(); })}
                {range_row("プリディレイ", Signal::derive(move || s.project.with(|p| p.reverb.pre_delay_ms)), 0.0, 500.0, 1.0, |v| format!("{} ms", v as i64), move |v| { s.project.update(|p| p.reverb.pre_delay_ms = v); s.mark_dirty(); })}
            </div>
        </section>
    }
}

#[component]
pub fn OutputPanel() -> impl IntoView {
    let s = expect_context::<Studio>();
    let limiter_enabled = Signal::derive(move || s.project.with(|p| p.output.limiter.enabled));

    view! {
        <section class="panel">
            <h2 class="panel__heading">"出力設定"</h2>
            <p class="panel__muted small">
                "書き出しの解像度・余韻と、仕上げのマスターリミッターをまとめて調整します。"
            </p>
            <div class="grid2">
                <label class="field">
                    <span class="field__label">"サンプルレート"</span>
                    <select
                        class="select"
                        prop:value=move || s.project.with(|p| p.sample_rate.to_string())
                        on:change=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                s.project.update(|p| p.sample_rate = v);
                                s.mark_dirty();
                            }
                        }
                    >
                        {SAMPLE_RATES.iter().map(|hz| view! { <option value=hz.to_string()>{format_rate(*hz)}</option> }).collect_view()}
                    </select>
                </label>
                {range_row("テール", Signal::derive(move || s.project.with(|p| p.output.tail_sec)), 0.0, 10.0, 0.05, |v| format!("{v:.2} s"), move |v| { s.project.update(|p| p.output.tail_sec = v); s.mark_dirty(); })}
            </div>

            <div class="panel__head">
                <h3 class="subheading">"マスターリミッター"</h3>
                <label class="checkline">
                    <input
                        type="checkbox"
                        prop:checked=move || limiter_enabled.get()
                        on:change=move |ev| {
                            let c = event_target_checked(&ev);
                            s.project.update(|p| p.output.limiter.enabled = c);
                            s.mark_dirty();
                        }
                    />
                    "有効"
                </label>
            </div>
            {range_row("スレッショルド", Signal::derive(move || s.project.with(|p| p.output.limiter.threshold)), 0.1, 1.0, 0.01, format_db, move |v| { s.project.update(|p| p.output.limiter.threshold = v); s.mark_dirty(); })}
        </section>
    }
}

#[component]
pub fn HelpPanel() -> impl IntoView {
    view! {
        <section class="panel help">
            <h2 class="panel__heading">"ワークフロー"</h2>
            <ol class="help__list">
                <li><strong>".mid"</strong>" をドラッグ＆ドロップ → トラック / ノート / テンポを解析"</li>
                <li><strong>"音声素材"</strong>"（WAV / MP3）を追加し、トラックに割り当て"</li>
                <li>"基準ピッチ・DAHDSR エンベロープ・音色フィルター・ダイナミックピッチ・リバーブを調整"</li>
                <li><strong>"Space"</strong>" で再生、タイムラインのクリックでシーク"</li>
                <li>"WAV / MP3 に高音質で書き出し"</li>
            </ol>
            <p class="help__note">
                "🎹 ノートの音高は素材の基準ピッチからの差分で再生速度を変えて発音します。再生は3次エルミート補間で高品質に。ベロシティとエクスプレッション(CC11)は音量に反映、ロングトーンはループ範囲で持続します。フィルターはアンプEG連動スイープと LFO ワブルで時間変化させられます。"
            </p>
        </section>
    }
}
