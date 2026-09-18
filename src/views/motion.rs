use std::time::{Duration, Instant};

use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::InteractiveElementExt;
use gpui_kit::*;

pub fn tab_transition(id: impl Into<ElementId>, content: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .w_full()
        .overflow_hidden()
        .child(content)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint),
            move |element, progress| {
                let slide = 1.0 - progress;
                element.relative().left(relative(slide * 0.04))
            },
        )
        .into_any_element()
}

pub fn modal_transition(id: impl Into<ElementId>, content: impl IntoElement) -> AnyElement {
    div()
        .w_full()
        .flex()
        .justify_center()
        .child(content)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint),
            move |element, progress| {
                let y = (1.0 - progress) * 20.0;
                element.relative().top(px(y)).opacity(progress)
            },
        )
        .into_any_element()
}

pub fn backdrop_transition(id: impl Into<ElementId>, content: impl IntoElement) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .on_scroll_wheel(|_, _, cx| {
            cx.stop_propagation();
        })
        .child(content)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(180)),
            move |element, progress| {
                element.opacity(progress)
            },
        )
        .into_any_element()
}

fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

pub fn selected_highlight(id: impl Into<ElementId>, selected: bool, element: Div) -> AnyElement {
    element
        .with_animation(
            (id.into(), if selected { "selected" } else { "idle" }),
            Animation::new(Duration::from_millis(160)),
            move |element, progress| {
                element.bg(super::red_a(if selected { progress } else { 0.0 }))
            },
        )
        .into_any_element()
}

pub fn smooth_scroll(id: impl Into<ElementId>, content: impl IntoElement) -> SmoothScroll {
    SmoothScroll {
        id: id.into(),
        content: content.into_any_element(),
    }
}

#[derive(IntoElement)]
pub struct SmoothScroll {
    id: ElementId,
    content: AnyElement,
}

#[derive(Default)]
struct ScrollMotion {
    handle: ScrollHandle,
    target: Option<f32>,
    last_offset: f32,
    last_frame: Option<Instant>,
}

fn clamp_offset(offset: f32, maximum: f32) -> f32 {
    offset.clamp(-maximum.max(0.0), 0.0)
}

fn approach(current: f32, target: f32, elapsed: f32) -> f32 {
    let next = current + (target - current) * (1.0 - (-elapsed.max(0.0) / 0.055).exp());
    if (target - next).abs() < 0.25 {
        target
    } else {
        next
    }
}

impl ScrollMotion {
    fn advance(&mut self, reduced: bool) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        let current = f32::from(self.handle.offset().y);
        if (current - self.last_offset).abs() > 0.5 {
            self.target = None;
            self.last_frame = None;
            return false;
        }
        let maximum = f32::from(self.handle.max_offset().y);
        let target = clamp_offset(target, maximum);
        let now = Instant::now();
        let elapsed = self
            .last_frame
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        let next = if reduced {
            target
        } else {
            clamp_offset(approach(current, target, elapsed), maximum)
        };
        self.handle.set_offset(point(px(0.0), px(next)));
        self.last_offset = next;
        self.last_frame = Some(now);
        self.target = (next != target).then_some(target);
        self.target.is_some()
    }
}

impl RenderOnce for SmoothScroll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "motion"), cx, |_, _| {
            ScrollMotion::default()
        });
        let reduced = cx.reduce_motion();
        if state.update(cx, |state, _| state.advance(reduced)) {
            window.request_animation_frame();
        }
        let handle = state.read(cx).handle.clone();
        let cancel = state.clone();
        div()
            .id(self.id.clone())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .w_full()
            .relative()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cancel.update(cx, |state, _| {
                    state.target = None;
                    state.last_frame = None;
                });
            })
            .child(
                div()
                    .id((self.id.clone(), "viewport"))
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .lock_scroll_axis()
                    .track_scroll(&handle)
                    .child(
                        div()
                            .id((self.id.clone(), "content"))
                            .w_full()
                            .min_h_full()
                            .flex_none()
                            .on_scroll_wheel(move |event, window, cx| {
                                let reduced = cx.reduce_motion();
                                let delta =
                                    f32::from(event.delta.pixel_delta(window.line_height()).y);
                                state.update(cx, |state, cx| {
                                    if event.delta.precise() || reduced {
                                        state.target = None;
                                        state.last_frame = None;
                                        return;
                                    }
                                    if delta == 0.0 || !delta.is_finite() {
                                        return;
                                    }
                                    let current = f32::from(state.handle.offset().y);
                                    let maximum = f32::from(state.handle.max_offset().y);
                                    let base = if (current - state.last_offset).abs() <= 0.5 {
                                        state.target.unwrap_or(current)
                                    } else {
                                        current
                                    };
                                    let target = clamp_offset(base + delta, maximum);
                                    state.target = (target != current).then_some(target);
                                    state.last_offset = current;
                                    state.last_frame = Some(Instant::now());
                                    cx.stop_propagation();
                                    cx.notify();
                                });
                            })
                            .child(self.content),
                    ),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&handle)
                        .id((self.id, "scrollbar"))
                        .viewport_from_layout(),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{approach, clamp_offset};

    #[test]
    fn offsets_stay_inside_content() {
        assert_eq!(clamp_offset(20.0, 100.0), 0.0);
        assert_eq!(clamp_offset(-120.0, 100.0), -100.0);
        assert_eq!(clamp_offset(-20.0, 0.0), 0.0);
        assert_eq!(clamp_offset(-20.0, -10.0), 0.0);
        assert_eq!(clamp_offset(-20.0, 100.0), -20.0);
    }

    #[test]
    fn easing_is_frame_rate_independent() {
        let full = approach(0.0, -100.0, 0.032);
        let half = approach(approach(0.0, -100.0, 0.016), -100.0, 0.016);
        assert!((full - half).abs() < 0.001);
    }

    #[test]
    fn easing_reverses_and_settles_without_overshoot() {
        assert!(approach(-80.0, -20.0, 0.016) > -80.0);
        assert!(approach(-80.0, -20.0, 0.016) <= -20.0);
        assert_eq!(approach(-20.1, -20.0, 0.016), -20.0);
        assert_eq!(approach(0.0, -100.0, 10.0), -100.0);
    }
}
