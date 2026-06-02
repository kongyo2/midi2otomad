use leptos::prelude::*;
use midi2otomad_core::music::midi_to_note_name;

use crate::state::Studio;

fn mini_bars(peaks: &[f32]) -> Vec<f64> {
    if peaks.is_empty() {
        return vec![8.0; 22];
    }
    let step = (peaks.len() / 22).max(1);
    (0..22)
        .map(|i| {
            let v = peaks.get(i * step).copied().unwrap_or(0.0) as f64;
            (v * 100.0).max(8.0)
        })
        .collect()
}

#[component]
pub fn SampleLibrary() -> impl IntoView {
    let s = expect_context::<Studio>();
    view! {
        <section class="panel">
            <div class="panel__head">
                <h2 class="panel__heading">"音声素材ライブラリ"</h2>
                <button class="btn btn--sm" on:click=move |_| s.open_audio()>
                    "+ 追加"
                </button>
            </div>
            <div
                class="droparea"
                class:droparea--over=move || s.drag_active.get()
            >
                {move || {
                    let samples = s.project.get().samples;
                    if samples.is_empty() {
                        view! {
                            <p class="droparea__hint">
                                "WAV / MP3 などをここにドロップ、または「追加」"
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <ul class="samplelist">
                                <For
                                    each=move || s.project.get().samples
                                    key=|sample| sample.id.clone()
                                    let:sample
                                >
                                    {
                                        let id = sample.id.clone();
                                        let id_sel = id.clone();
                                        let id_rm = id.clone();
                                        let bars = mini_bars(
                                            s.peaks.get_untracked().get(&id).map(Vec::as_slice).unwrap_or(&[]),
                                        );
                                        let sub = format!(
                                            "基準 {} · {:.2}s{}",
                                            midi_to_note_name(sample.base_pitch as f64),
                                            sample.duration_sec,
                                            if sample.loop_region.enabled { " · ⟳loop" } else { "" },
                                        );
                                        view! {
                                            <li>
                                                <button
                                                    class="samplelist__item"
                                                    class:samplelist__item--active=move || {
                                                        s.selected_sample.get().as_deref() == Some(id_sel.as_str())
                                                    }
                                                    on:click=move |_| s.selected_sample.set(Some(id.clone()))
                                                >
                                                    <span class="miniwave">
                                                        {bars
                                                            .into_iter()
                                                            .map(|h| {
                                                                view! {
                                                                    <span
                                                                        class="miniwave__bar"
                                                                        style:height=format!("{h}%")
                                                                    ></span>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </span>
                                                    <span class="samplelist__meta">
                                                        <span class="samplelist__name">{sample.name.clone()}</span>
                                                        <span class="samplelist__sub">{sub}</span>
                                                    </span>
                                                </button>
                                                <button
                                                    class="iconbtn"
                                                    title="削除"
                                                    on:click=move |_| s.remove_sample(id_rm.clone())
                                                >
                                                    "✕"
                                                </button>
                                            </li>
                                        }
                                    }
                                </For>
                            </ul>
                        }
                            .into_any()
                    }
                }}
            </div>
        </section>
    }
}
