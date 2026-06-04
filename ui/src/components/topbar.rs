use leptos::prelude::*;

use crate::format::format_time;
use crate::icons::{icon_download, icon_pause, icon_play, icon_skip_back, icon_stop, icon_zap};
use crate::state::{project_duration, Studio};

#[component]
pub fn TopBar() -> impl IntoView {
    let s = expect_context::<Studio>();
    let format = RwSignal::new("wav".to_string());
    let wav_bit_depth = RwSignal::new(24u16);
    let mp3_bitrate = RwSignal::new(320u32);

    let position = move || s.status.get().position;
    let duration = move || {
        let mix = s.status.get().duration;
        if mix > 0.0 {
            mix
        } else {
            project_duration(&s.project.get())
        }
    };
    let level = move || (s.status.get().level as f64 * 100.0).clamp(0.0, 100.0);

    let do_export = move |_| {
        let fmt = format.get();
        if fmt == "mp3" {
            s.export("mp3".into(), None, Some(mp3_bitrate.get()));
        } else {
            s.export("wav".into(), Some(wav_bit_depth.get()), None);
        }
    };

    view! {
        <header class="topbar">
            <div class="topbar__brand">
                <svg class="topbar__logo" width="30" height="30" viewBox="0 0 32 32" fill="none">
                    <rect
                        x="1.5"
                        y="1.5"
                        width="29"
                        height="29"
                        rx="8.5"
                        fill="#1a1612"
                        stroke="#3a342c"
                    ></rect>
                    <path
                        d="M4 16h4l2.4-7.5 4 15 3-9.5 1.8 2h6.8"
                        stroke="#ff8a3d"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    ></path>
                    <circle cx="25.5" cy="8.5" r="2.3" fill="#c8f24e"></circle>
                </svg>
                <div>
                    <h1 class="topbar__title">"midi2otomad"</h1>
                    <p class="topbar__tag">"MIDI 音MAD スタジオ"</p>
                </div>
            </div>

            <div class="topbar__group">
                <button class="btn btn--ghost" on:click=move |_| s.open_midi()>
                    "MIDI を開く"
                </button>
                <select
                    class="select select--mini"
                    title="MIDI 取り込みモード（自動: ch10/バンク127をドラムとして音階固定）"
                    prop:value=move || s.import_mode.get()
                    on:change=move |ev| s.import_mode.set(event_target_value(&ev))
                >
                    <option value="auto">"自動"</option>
                    <option value="normal">"音階"</option>
                    <option value="drum">"ドラム"</option>
                </select>
            </div>

            <div class="topbar__transport">
                <button class="transportbtn" title="先頭へ" on:click=move |_| s.seek(0.0)>
                    {icon_skip_back()}
                </button>
                <button
                    class="transportbtn transportbtn--main"
                    title=move || if s.status.get().playing { "一時停止" } else { "再生" }
                    on:click=move |_| s.toggle_play()
                >
                    {move || if s.status.get().playing { icon_pause().into_any() } else { icon_play().into_any() }}
                </button>
                <button class="transportbtn" title="停止" on:click=move |_| s.stop()>
                    {icon_stop()}
                </button>
                <span class="topbar__time">
                    {move || format_time(position())}
                    <span class="topbar__time-sep">"/"</span>
                    {move || format_time(duration())}
                </span>
                <div class="meter" title="マスターレベル">
                    <div
                        class="meter__fill"
                        style:width=move || format!("{}%", level())
                    ></div>
                </div>
                <button
                    class="perfbtn"
                    class=("perfbtn--on", move || s.performance_mode.get())
                    title="高パフォーマンスモード — プレビューと再生を軽量・高速にレンダリング（線形補間・フィルター/グラニュラー簡略化）。書き出しは常に高音質。"
                    on:click=move |_| {
                        let on = s.performance_mode.get_untracked();
                        s.set_performance_mode(!on);
                    }
                >
                    {icon_zap()}
                    <span class="perfbtn__label">"高速"</span>
                </button>
            </div>

            <div class="topbar__group topbar__master">
                <div class="microfield">
                    <span>"BPM"</span>
                    <span class="microfield__value">
                        {move || (s.project.get().bpm.round() as i64).to_string()}
                    </span>
                </div>
                <label class="microfield microfield--wide">
                    <span>"Master"</span>
                    <input
                        class="range"
                        type="range"
                        min=0
                        max=2
                        step=0.01
                        prop:value=move || s.project.get().master_gain
                        style=("--fill", move || format!("{:.1}%", (s.project.get().master_gain / 2.0 * 100.0).clamp(0.0, 100.0)))
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                s.project.update(|p| p.master_gain = v);
                                s.mark_dirty();
                            }
                        }
                    />
                </label>
            </div>

            <div class="topbar__export">
                <select
                    class="select select--mini"
                    on:change=move |ev| format.set(event_target_value(&ev))
                >
                    <option value="wav">"WAV"</option>
                    <option value="mp3">"MP3"</option>
                </select>
                {move || {
                    if format.get() == "wav" {
                        view! {
                            <select
                                class="select select--mini"
                                on:change=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<u16>() {
                                        wav_bit_depth.set(v);
                                    }
                                }
                            >
                                <option value="16">"16 bit"</option>
                                <option value="24" selected>"24 bit"</option>
                                <option value="32">"32 bit float"</option>
                            </select>
                        }
                            .into_any()
                    } else {
                        view! {
                            <select
                                class="select select--mini"
                                on:change=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                        mp3_bitrate.set(v);
                                    }
                                }
                            >
                                <option value="192">"192 kbps"</option>
                                <option value="256">"256 kbps"</option>
                                <option value="320" selected>"320 kbps"</option>
                            </select>
                        }
                            .into_any()
                    }
                }}
                <button class="btn" prop:disabled=move || s.busy.get().is_some() on:click=do_export>
                    {move || {
                        if s.busy.get().is_some() {
                            view! { "処理中…" }.into_any()
                        } else {
                            view! { {icon_download()} "書き出し" }.into_any()
                        }
                    }}
                </button>
            </div>
        </header>
    }
}
