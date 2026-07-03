use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub fn range_row(
    label: impl IntoView + 'static,
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
                style=("--fill", move || format!("{:.1}%", ((value.get() - min) / (max - min) * 100.0).clamp(0.0, 100.0)))
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                        on_input(v);
                    }
                }
            />
        </label>
    }
}

pub fn context_2d(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|o| o.dyn_into::<CanvasRenderingContext2d>().ok())
}

/// ポインタ位置を要素幅に対する 0.0〜1.0 の割合へ変換する。
fn pointer_frac(ev: &web_sys::PointerEvent) -> Option<f64> {
    let el = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?;
    let rect = el.get_bounding_client_rect();
    if rect.width() <= 0.0 {
        return None;
    }
    Some(((ev.client_x() as f64 - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

const MIN_DRAG_FRAC: f64 = 0.002;

#[component]
pub fn Waveform(
    #[prop(into)] peaks: Signal<Vec<f32>>,
    #[prop(into)] loop_region: Signal<Option<(f64, f64, bool)>>,
    #[prop(into)] trim: Signal<Option<(f64, f64, bool)>>,
    #[prop(into)] color: String,
    height: f64,
    /// ドラッグで範囲選択したときに (開始, 終了) の割合 (0..1) を通知する。
    #[prop(optional)]
    on_select: Option<Callback<(f64, f64)>>,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let drag_anchor = StoredValue::new(None::<f64>);
    Effect::new(move |_| {
        let peaks = peaks.get();
        let loop_region = loop_region.get();
        let trim = trim.get();
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
        ctx.set_fill_style_str("rgba(255,238,210,0.025)");
        ctx.fill_rect(0.0, 0.0, w, height);

        if let Some((start, end, true)) = loop_region {
            let x0 = start * w;
            let x1 = end * w;
            ctx.set_fill_style_str("rgba(200,242,78,0.15)");
            ctx.fill_rect(x0, 0.0, (x1 - x0).max(1.0), height);
            ctx.set_stroke_style_str("rgba(200,242,78,0.9)");
            ctx.set_line_width(2.0);
            for x in [x0, x1] {
                ctx.begin_path();
                ctx.move_to(x, 0.0);
                ctx.line_to(x, height);
                ctx.stroke();
            }
        }

        let mid = height / 2.0;
        ctx.set_stroke_style_str("rgba(255,240,220,0.1)");
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

        if let Some((start, end, true)) = trim {
            let x0 = start * w;
            let x1 = end * w;
            ctx.set_fill_style_str("rgba(8,7,6,0.62)");
            if x0 > 0.0 {
                ctx.fill_rect(0.0, 0.0, x0, height);
            }
            if x1 < w {
                ctx.fill_rect(x1, 0.0, w - x1, height);
            }
            ctx.set_stroke_style_str("rgba(255,138,61,0.95)");
            ctx.set_line_width(2.0);
            for x in [x0, x1] {
                ctx.begin_path();
                ctx.move_to(x, 0.0);
                ctx.line_to(x, height);
                ctx.stroke();
            }
        }
    });

    let emit = move |anchor: f64, frac: f64| {
        if let Some(cb) = on_select {
            let (a, b) = if frac < anchor {
                (frac, anchor)
            } else {
                (anchor, frac)
            };
            if b - a >= MIN_DRAG_FRAC {
                cb.run((a, b));
            }
        }
    };

    view! {
        <div
            class="waveform"
            class:waveform--editable=on_select.is_some()
            on:pointerdown=move |ev| {
                if on_select.is_none() {
                    return;
                }
                if let Some(frac) = pointer_frac(&ev) {
                    drag_anchor.set_value(Some(frac));
                    if let Some(el) = ev
                        .current_target()
                        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    {
                        let _ = el.set_pointer_capture(ev.pointer_id());
                    }
                }
            }
            on:pointermove=move |ev| {
                let Some(anchor) = drag_anchor.get_value() else { return };
                if let Some(frac) = pointer_frac(&ev) {
                    emit(anchor, frac);
                }
            }
            on:pointerup=move |ev| {
                if let (Some(anchor), Some(frac)) = (drag_anchor.get_value(), pointer_frac(&ev)) {
                    emit(anchor, frac);
                }
                drag_anchor.set_value(None);
            }
            on:pointercancel=move |_| drag_anchor.set_value(None)
        >
            <canvas node_ref=canvas_ref></canvas>
        </div>
    }
}
