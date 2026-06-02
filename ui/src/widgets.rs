//! 再利用する小さな UI 部品。スライダー・チェックボックス行と波形キャンバス。

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// ラベル + レンジスライダー + 現在値表示の 1 行。値はシグナルでリアクティブに同期する。
pub fn range_row(
    label: &'static str,
    value: Signal<f64>,
    min: f64,
    max: f64,
    step: f64,
    fmt: impl Fn(f64) -> String + Send + Sync + 'static,
    on_input: impl Fn(f64) + 'static,
) -> impl IntoView {
    view! {
        <label class="field">
            <span class="field__label">{label} " " <em>{move || fmt(value.get())}</em></span>
            <input
                class="range"
                type="range"
                min=min
                max=max
                step=step
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                        on_input(v);
                    }
                }
            />
        </label>
    }
}

/// キャンバス要素から 2D コンテキストを取り出す。
pub fn context_2d(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|o| o.dyn_into::<CanvasRenderingContext2d>().ok())
}

/// 波形サムネイル。`peaks` は 0..1 の振幅エンベロープ。`loop_region` は (start, end, enabled) の割合。
#[component]
pub fn Waveform(
    #[prop(into)] peaks: Signal<Vec<f32>>,
    #[prop(into)] loop_region: Signal<Option<(f64, f64, bool)>>,
    #[prop(into)] color: String,
    height: f64,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    Effect::new(move |_| {
        let peaks = peaks.get();
        let loop_region = loop_region.get();
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        let width = peaks.len().max(1) as u32;
        canvas.set_width(width);
        canvas.set_height(height as u32);
        let Some(ctx) = context_2d(&canvas) else {
            return;
        };
        let w = width as f64;
        ctx.clear_rect(0.0, 0.0, w, height);
        ctx.set_fill_style_str("rgba(255,255,255,0.03)");
        ctx.fill_rect(0.0, 0.0, w, height);

        if let Some((start, end, true)) = loop_region {
            let x0 = start * w;
            let x1 = end * w;
            ctx.set_fill_style_str("rgba(124,92,255,0.18)");
            ctx.fill_rect(x0, 0.0, (x1 - x0).max(1.0), height);
            ctx.set_stroke_style_str("rgba(124,92,255,0.9)");
            ctx.set_line_width(2.0);
            for x in [x0, x1] {
                ctx.begin_path();
                ctx.move_to(x, 0.0);
                ctx.line_to(x, height);
                ctx.stroke();
            }
        }

        let mid = height / 2.0;
        ctx.set_stroke_style_str("rgba(255,255,255,0.12)");
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(0.0, mid);
        ctx.line_to(w, mid);
        ctx.stroke();

        ctx.set_fill_style_str(&color);
        for (i, &value) in peaks.iter().enumerate() {
            let h = (value as f64 * (height - 6.0)).max(1.0);
            ctx.fill_rect(i as f64, mid - h / 2.0, 1.0, h);
        }
    });

    view! { <div class="waveform"><canvas node_ref=canvas_ref></canvas></div> }
}
