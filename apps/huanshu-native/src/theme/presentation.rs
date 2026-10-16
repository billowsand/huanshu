//! Presentation surrounds: subdued theme texture behind the page, shared by overview.
use gpui_kit::prelude::*;
use gpui_kit::{
    canvas, div, fill, linear_color_stop, linear_gradient, point, px, size, App, Bounds, BoxShadow,
    Div,
};

use super::StudioExt as _;
use crate::widgets;

pub fn backdrop(cx: &App) -> Div {
    if let Some(paper) = cx.studio().paper {
        return widgets::wood_grain(&paper);
    }
    let c = cx.studio().colors;
    let id = cx.studio().theme.id;
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .overflow_hidden()
        .bg(c.bg)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let dark = matches!(id, "dark-yellow" | "dark-blue" | "dark-terminal");
                    let wash = if dark { c.primary } else { c.border_hover };
                    window.paint_quad(fill(
                        bounds,
                        linear_gradient(
                            135.0,
                            linear_color_stop(c.bg.blend(wash.opacity(0.10)), 0.0),
                            linear_color_stop(c.bg, 1.0),
                        ),
                    ));
                    let width = bounds.size.width.as_f32();
                    let height = bounds.size.height.as_f32();
                    let mut mark = |x: f32, y: f32, w: f32, h: f32, color| {
                        window.paint_quad(fill(
                            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h))),
                            color,
                        ));
                    };
                    match id {
                        // Amber: a quiet, wide architectural grid.
                        "dark-yellow" => {
                            for x in (0..width.ceil() as usize).step_by(96) {
                                mark(x as f32, 0.0, 1.0, height, c.primary.opacity(0.045));
                            }
                            for y in (0..height.ceil() as usize).step_by(96) {
                                mark(0.0, y as f32, width, 1.0, c.primary.opacity(0.045));
                            }
                        }
                        // Star chart: pinpoints with larger crosshair coordinates.
                        "dark-blue" => {
                            for x in (24..width.ceil() as usize).step_by(48) {
                                for y in (24..height.ceil() as usize).step_by(48) {
                                    mark(x as f32, y as f32, 1.0, 1.0, c.primary.opacity(0.22));
                                    if x % 192 == 24 && y % 192 == 24 {
                                        mark(
                                            x as f32 - 3.0,
                                            y as f32,
                                            7.0,
                                            1.0,
                                            c.primary.opacity(0.13),
                                        );
                                        mark(
                                            x as f32,
                                            y as f32 - 3.0,
                                            1.0,
                                            7.0,
                                            c.primary.opacity(0.13),
                                        );
                                    }
                                }
                            }
                        }
                        // Terminal: fine scanlines on ink; dot matrix on paper.
                        "dark-terminal" => {
                            for y in (0..height.ceil() as usize).step_by(4) {
                                mark(0.0, y as f32, width, 1.0, c.text.opacity(0.022));
                            }
                        }
                        "light-terminal" => {
                            for x in (12..width.ceil() as usize).step_by(24) {
                                for y in (12..height.ceil() as usize).step_by(24) {
                                    mark(x as f32, y as f32, 1.0, 1.0, c.muted.opacity(0.16));
                                }
                            }
                        }
                        // Newsprint: column rules and a single cobalt masthead rule.
                        "light-editorial" => {
                            for x in (32..width.ceil() as usize).step_by(160) {
                                mark(x as f32, 0.0, 1.0, height, c.border_hover.opacity(0.12));
                            }
                            mark(0.0, 0.0, width, 2.0, c.primary.opacity(0.4));
                        }
                        _ => {}
                    }
                },
            )
            .size_full(),
        )
}

/// A contact shadow and a soft cast shadow on the page itself, separate from selection.
pub fn page_shadow(cx: &App) -> Vec<BoxShadow> {
    if let Some(paper) = cx.studio().paper {
        return widgets::paper_shadow(&paper);
    }
    let c = cx.studio().colors;
    let dark = cx.studio().theme.tone.is_dark();
    let ink = if dark { c.bg } else { c.text };
    vec![
        BoxShadow {
            color: ink.opacity(if dark { 0.65 } else { 0.12 }),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(3.0),
            spread_radius: px(0.0),
            inset: false,
        },
        BoxShadow {
            color: ink.opacity(if dark { 0.5 } else { 0.18 }),
            offset: point(px(0.0), px(6.0)),
            blur_radius: px(14.0),
            spread_radius: px(-3.0),
            inset: false,
        },
    ]
}
