//! The viewport, which draws the shape where the clip has it, and
//! the timeline that plays and scrubs the clip.

use bevy::color::Color;
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Local, Res, ResMut};
use bevy::time::Time;
use bevy::ui::{
    AlignItems, Display, JustifyContent, Overflow, PositionType,
    UiRect, Val, percent, px,
};
use bevy_fynix::tokens::{SurfaceTokens, TextTokens, Tone};
use bevy_fynix::views::{
    BehaviorExt, FrameProps, Span, TooltipExt, button, column, frame,
    icon, label, primary, rich, row, slider, tinted_icon,
};
use bevy_fynix::{AnyView, Bevy, ScopedExt, View, resource};

use crate::common::Monokai;
use crate::{
    Build, Clip, DURATION, Easing, LogRows, Shape, commands, icons,
    panel,
};

pub const EASINGS: [&str; 4] =
    ["Linear", "Ease in", "Ease out", "Ease in and out"];

/// Where the shape is drawn this frame.
#[derive(Resource, Default, PartialEq)]
pub struct Pose {
    left: f32,
    top: f32,
    scale: f32,
}

/// Moves the playhead, adds the frames it plays to the log, and poses
/// the shape: out along its travel over the first half of the clip
/// and back over the second.
pub fn play(
    time: Res<Time>,
    mut clip: ResMut<Clip>,
    mut rows: ResMut<LogRows>,
    mut logged: Local<f32>,
    shape: Res<Shape>,
    easing: Res<Easing>,
    mut pose: ResMut<Pose>,
) {
    if clip.playing {
        let step = time.delta_secs();
        clip.time = (clip.time + step) % DURATION;
        *logged += step;
        let frames = (*logged * 60.0) as usize;
        if frames > 0 {
            rows.0 += frames;
            *logged -= frames as f32 / 60.0;
        }
    }
    let there = 1.0 - (2.0 * clip.time / DURATION - 1.0).abs();
    let eased = ease(easing.0, there);
    pose.set_if_neq(Pose {
        left: shape.x + shape.travel * eased,
        top: shape.y,
        scale: 1.0 + 0.15 * eased,
    });
}

/// `t` along the curve of [`EASINGS`] at `at`.
fn ease(at: usize, t: f32) -> f32 {
    match at {
        1 => t * t,
        2 => 1.0 - (1.0 - t) * (1.0 - t),
        3 => t * t * (3.0 - 2.0 * t),
        _ => t,
    }
}

pub fn viewport() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        let accent = cx.theme().accent();
        let faint = cx.theme().tone(Tone::Faint);
        let hairline = cx.theme().hairline();
        let shape = column((label(resource::<Shape, _>(|shape| {
            shape.name.clone()
        }))
        .tone(Tone::OnAccent)
        .bold(true)
        .wrap(false),))
        .position(PositionType::Absolute)
        .inset(resource::<Pose, _>(|pose| at(pose.left, pose.top)))
        .scale(resource::<Pose, _>(|pose| pose.scale))
        .width(resource::<Shape, _>(|shape| px(shape.size)))
        .height(resource::<Shape, _>(|shape| px(shape.size)))
        .radius(resource::<Shape, _>(|shape| shape.radius))
        .opacity(resource::<Shape, _>(|shape| shape.opacity))
        .display(resource::<Shape, _>(|shape| {
            if shape.visible {
                Display::Flex
            } else {
                Display::None
            }
        }))
        .fill(accent)
        .justify(JustifyContent::Center)
        .align(AlignItems::Center);
        let path = frame()
            .position(PositionType::Absolute)
            .inset(resource::<Shape, _>(|shape| {
                let middle = shape.size / 2.0;
                at(shape.x + middle, shape.y + middle)
            }))
            .width(resource::<Shape, _>(|shape| px(shape.travel)))
            .height(px(1.0))
            .fill(faint);
        let caption = row((rich(resource::<Clip, _>(|clip| {
            vec![
                Span::new("t = "),
                Span::new(format!("{:.2}s", clip.time)).bold(),
                Span::new(if clip.playing {
                    "  playing"
                } else {
                    "  paused"
                })
                .italic(),
            ]
        }))
        .tone(Tone::Dim),))
        .position(PositionType::Absolute)
        .inset(UiRect {
            left: px(14.0),
            bottom: px(12.0),
            ..UiRect::all(Val::Auto)
        });
        cx.build(panel(
            column((
                path,
                end(faint, |shape| shape.x, 1.0),
                end(faint, |shape| shape.x + shape.travel, 1.15),
                shape,
                caption,
            ))
            .grow(1.0)
            .width(percent(100.0))
            .fill(Color::srgb_u8(0x19, 0x17, 0x1A))
            .radius(8.0)
            .border(1.0)
            .border_color(hairline)
            .overflow(Overflow::clip()),
        ))
    })
}

/// An absolute frame's inset, `left` and `top` from its parent.
fn at(left: f32, top: f32) -> UiRect {
    UiRect {
        left: px(left),
        top: px(top),
        ..UiRect::all(Val::Auto)
    }
}

/// An outline of the shape where one end of its path has it.
fn end(
    color: Color,
    left: fn(&Shape) -> f32,
    scale: f32,
) -> impl View<Bevy, Monokai> {
    frame()
        .position(PositionType::Absolute)
        .inset(resource::<Shape, _>(move |shape| {
            at(left(shape), shape.y)
        }))
        .width(resource::<Shape, _>(|shape| px(shape.size)))
        .height(resource::<Shape, _>(|shape| px(shape.size)))
        .radius(resource::<Shape, _>(|shape| shape.radius))
        .scale(scale)
        .border(1.0)
        .border_color(color)
}

pub fn timeline() -> AnyView<Bevy, Monokai> {
    let play = button(
        icon(resource::<Clip, _>(|clip| {
            if clip.playing {
                icons::PAUSE
            } else {
                icons::PLAY
            }
        }))
        .size(14.0),
    )
    .rules(primary)
    .on_activate(commands::run(commands::PLAY))
    .tooltip(|| label("Play or pause, Space"));
    let ticks = (0..=8)
        .map(|at| {
            label(format!("{:.1}s", at as f32 * DURATION / 8.0))
                .size(11.0)
                .tone(Tone::Faint)
        })
        .collect::<Vec<_>>();
    let scrub = slider(resource::<Clip, _>(|clip| clip.time))
        .range(0.0, DURATION)
        .on_change(|world, time| {
            let mut clip = world.resource_mut::<Clip>();
            clip.time = time;
            clip.playing = false;
        })
        .width(percent(100.0));
    AnyView::new(move |cx: &mut Build| {
        let line = cx.theme().hairline();
        let head = cx.theme().accent();
        let lane = |keys| lane(keys, line, head);
        cx.build(panel(
            column((
                row((
                    play,
                    tinted_icon(icons::REWIND)
                        .on_activate(commands::run(commands::REWIND))
                        .tooltip(|| label("Back to start")),
                    label(resource::<Clip, _>(|clip| {
                        format!("{:.2}s / {DURATION:.2}s", clip.time)
                    })),
                    column(()).grow(1.0),
                    label(resource::<Easing, _>(|easing| {
                        EASINGS[easing.0].to_string()
                    }))
                    .tone(Tone::Dim),
                ))
                .gap(10.0)
                .align(AlignItems::Center),
                track(
                    "",
                    column((
                        row(ticks)
                            .justify(JustifyContent::SpaceBetween),
                        scrub,
                    ))
                    .gap(4.0),
                ),
                track("x", lane(&[0.0, 0.5, 1.0])),
                track("travel", lane(&[0.0, 0.5, 1.0])),
                track("scale", lane(&[0.0, 0.5, 1.0])),
                track("opacity", lane(&[0.0, 1.0])),
            ))
            .gap(4.0),
        ))
    })
}

/// A row of the timeline: a name, then what it shows over the clip.
fn track(
    name: &'static str,
    over: impl View<Bevy, Monokai>,
) -> impl View<Bevy, Monokai> {
    row((
        column((label(name).tone(Tone::Dim),)).width(px(72.0)),
        column((over,)).grow(1.0),
    ))
    .align(AlignItems::Center)
}

/// Keyframes at `keys`, as shares of the clip, on a `line` under a
/// playhead of `head`.
fn lane(
    keys: &[f32],
    line: Color,
    head: Color,
) -> impl View<Bevy, Monokai> + use<> {
    let size = 10.0;
    let diamonds = keys
        .iter()
        .map(|&key| {
            column(
                (icon(icons::LAYER).size(size).tone(Tone::Accent),),
            )
            .position(PositionType::Absolute)
            .inset(UiRect {
                left: percent(key * 100.0),
                top: px((24.0 - size) / 2.0),
                ..UiRect::all(Val::Auto)
            })
            .margin(UiRect::left(px(-size / 2.0)))
        })
        .collect::<Vec<_>>();
    column((
        frame().height(px(1.0)).width(percent(100.0)).fill(line),
        column(diamonds)
            .position(PositionType::Absolute)
            .width(percent(100.0))
            .height(percent(100.0)),
        frame()
            .position(PositionType::Absolute)
            .inset(resource::<Clip, _>(|clip| UiRect {
                left: percent(clip.time / DURATION * 100.0),
                top: px(0.0),
                ..UiRect::all(Val::Auto)
            }))
            .width(px(2.0))
            .height(percent(100.0))
            .fill(head),
    ))
    .height(px(24.0))
    .justify(JustifyContent::Center)
}
