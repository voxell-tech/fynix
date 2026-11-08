//! The log: a row per frame played, 100,000 to start, built only
//! near the view.

use bevy::color::Color;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::system::{Local, Query, Res};
use bevy::ui::{AlignItems, ComputedNode, UiRect, px};
use bevy_fynix::tokens::{TextTokens, Tone};
use bevy_fynix::views::{
    BehaviorExt, FrameProps, RowSlot, Span, VirtualList, button,
    checkbox, column, label, rich, row, virtual_list,
};
use bevy_fynix::{AnyView, Bevy, View, resource};

use crate::common::Monokai;
use crate::{Build, FollowLog, LogRows, commands, panel};

pub fn output() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        cx.build(panel(
            column((
                row((
                    action("Top", |list| list.scroll_to(0)),
                    action("Row 50,000", |list| {
                        list.scroll_to(50_000)
                    }),
                    action("Bottom", |list| {
                        list.scroll_to(usize::MAX)
                    }),
                    label(resource::<LogRows, _>(|rows| {
                        format!("{} rows", rows.0)
                    }))
                    .tone(Tone::Dim),
                    column(()).grow(1.0),
                    label("Follow"),
                    checkbox(resource::<FollowLog, _>(follows))
                        .on_change(|world, on| {
                            world.resource_mut::<FollowLog>().0 = on;
                        }),
                ))
                .gap(8.0)
                .align(AlignItems::Center),
                column((virtual_list(
                    resource::<LogRows, _>(|rows| rows.0),
                    20.0,
                    line,
                )
                .follow_bottom(resource::<
                    FollowLog,
                    _,
                >(
                    follows
                )),))
                .grow(1.0)
                .min_height(px(0.0)),
            ))
            .gap(8.0)
            .grow(1.0)
            .min_height(px(0.0)),
        ))
    })
}

fn follows(follow: &FollowLog) -> bool {
    follow.0
}

/// Takes the log to its bottom when it starts to follow or comes
/// into view, so that it follows from there. A hidden tab is not laid
/// out, so the list cannot follow while it is behind another.
pub fn follow_from_bottom(
    follow: Res<FollowLog>,
    mut lists: Query<(&mut VirtualList, &ComputedNode)>,
    mut shown: Local<bool>,
) {
    for (mut list, node) in &mut lists {
        let showing = node.size().y > 0.0;
        if follow.0 && (follow.is_changed() || showing && !*shown) {
            list.scroll_to(usize::MAX);
        }
        *shown = showing;
    }
}

/// A button that runs `run` on the log's list.
fn action(
    text: &'static str,
    run: fn(&mut VirtualList),
) -> impl View<Bevy, Monokai> {
    button(label(text))
        .padding(UiRect::axes(px(10.0), px(4.0)))
        .on_activate(move |world| commands::on_log(world, run))
}

/// The row a slot shows: when the frame played, which it was and how
/// long it took, slow ones in the critical tone.
fn line(slot: RowSlot) -> AnyView<Bevy, Monokai> {
    AnyView::new(move |cx: &mut Build| {
        let theme = cx.theme();
        let (dim, body, slow) = (
            theme.tone(Tone::Dim),
            theme.tone(Tone::Body),
            theme.tone(Tone::Critical),
        );
        cx.build(
            row((rich(
                slot.index()
                    .map(move |index| spans(index, dim, body, slow)),
            ),))
            .padding(UiRect::axes(px(8.0), px(2.0))),
        )
    })
}

fn spans(
    index: usize,
    dim: Color,
    body: Color,
    slow: Color,
) -> Vec<Span> {
    let millis = index * 1000 / 60;
    let (minutes, seconds) = (millis / 60_000, millis % 60_000);
    // A spread of frame times that looks measured.
    let mut mixed =
        (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    mixed ^= mixed >> 31;
    let took = 4.0 + (mixed % 1000) as f32 / 100.0;
    vec![
        Span::new(format!(
            "{minutes:02}:{:02}.{:03}  ",
            seconds / 1000,
            seconds % 1000
        ))
        .color(dim),
        Span::new(format!("frame {index}")).color(body),
        Span::new(format!("  {took:.2} ms")).color(if took > 13.0 {
            slow
        } else {
            dim
        }),
    ]
}
