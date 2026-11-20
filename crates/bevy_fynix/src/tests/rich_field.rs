//! A rich field laid out headless, frame by frame.

use bevy::app::App;
use bevy::asset::{AssetId, Assets};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::math::Vec2;
use bevy::picking::pointer::PointerButton;
use bevy::text::Font;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform, px};

use super::Plain;
use super::layout::laid_out;
use crate::views::{
    Atom, FrameProps, Piece, RichField, frame, rich_field,
};
use crate::{ViewExt, mount};

const INTER: &[u8] =
    include_bytes!("../../../../assets/fonts/Inter.ttf");

fn chip(key: &str) -> Atom {
    Atom::new::<Plain>(key, key, || {
        frame().width(px(70.0)).height(px(20.0)).boxed()
    })
}

/// How high the box of a field's text is.
fn height(app: &App, root: Entity) -> f32 {
    let body = app.world().get::<Children>(root).unwrap()[0];
    let node = app.world().get::<ComputedNode>(body).unwrap();
    node.size().y * node.inverse_scale_factor()
}

#[test]
fn a_field_typed_into_every_frame_stops_at_its_lines() {
    let mut app = laid_out();
    app.add_plugins(bevy::text::TextPlugin);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), Font::from_bytes(INTER.to_vec()))
        .unwrap();
    let root = mount::<Plain>(
        app.world_mut(),
        rich_field()
            .max_lines(4)
            .content([
                Piece::Text("Look at ".into()),
                Piece::Atom(chip("a")),
                Piece::Text(" example!".into()),
            ])
            .width(px(420.0)),
    );
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(root, FocusCause::Pressed);
    app.update();
    let mut heights = Vec::new();
    for _ in 0..16 {
        app.world_mut()
            .get_mut::<RichField>(root)
            .unwrap()
            .insert_text("\n");
        app.update();
        heights.push(height(&app, root));
    }
    // Lines of 17, the one with the chip 20 high, and a frame for the
    // field to see it has a line more than it shows.
    assert!(
        heights.iter().all(|high| *high <= 3.0 + 5.0 * 17.0),
        "{heights:?}"
    );
    // Once the chip's line has scrolled away, four lines of text.
    assert!(
        heights[8..].iter().all(|high| *high == 4.0 * 17.0),
        "{heights:?}"
    );
}

#[test]
fn a_press_in_a_scrolled_field_puts_the_caret_on_the_line_under_it() {
    let mut app = laid_out();
    app.add_plugins(bevy::text::TextPlugin);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), Font::from_bytes(INTER.to_vec()))
        .unwrap();
    // Ten lines of 7 bytes, `line 0` to `line 9`, 17 high.
    let text = (0..10)
        .map(|at| format!("line {at}"))
        .collect::<Vec<_>>()
        .join("\n");
    let root = mount::<Plain>(
        app.world_mut(),
        rich_field()
            .max_lines(4)
            .content([Piece::Text(text)])
            .width(px(420.0)),
    );
    for _ in 0..5 {
        app.update();
    }
    let body = app.world().get::<Children>(root).unwrap()[0];
    super::wheel(&mut app, body, -3.0);
    for _ in 0..40 {
        app.update();
    }
    let scrolled = app.world().get::<ScrollPosition>(body).unwrap().y;
    let text = app.world().get::<Children>(body).unwrap()[0];
    let top = {
        let node = app.world().get::<ComputedNode>(text).unwrap();
        let center =
            app.world().get::<UiGlobalTransform>(text).unwrap();
        center.affine().translation - node.size() / 2.0
    };

    let line = 7.0;
    let at = top + Vec2::new(20.0, line * 17.0 + 8.0);
    super::pointer_press(&mut app, root, PointerButton::Primary, at);
    app.update();

    let caret = app.world().get::<RichField>(root).unwrap().caret();
    assert!(
        caret.is_some_and(|at| (49..=55).contains(&at)),
        "{caret:?}"
    );
    assert_eq!(
        app.world().get::<ScrollPosition>(body).unwrap().y,
        scrolled
    );
}

#[test]
fn blank_lines_before_hidden_text_keep_their_height() {
    use crate::views::{Mark, MarkKind};
    let mut app = laid_out();
    app.add_plugins(bevy::text::TextPlugin);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), Font::from_bytes(INTER.to_vec()))
        .unwrap();
    // Three blank lines, then `**bold**` with its stars hidden.
    let text = "Some text\n\n\n\n**bold**";
    let root = mount::<Plain>(
        app.world_mut(),
        rich_field()
            .content([Piece::Text(text.into())])
            .marks(|_: &str| {
                [13..15, 19..21]
                    .into_iter()
                    .map(|range| Mark {
                        range,
                        kind: MarkKind::Hidden { shown_in: 13..21 },
                    })
                    .collect()
            })
            .width(px(420.0)),
    );
    for _ in 0..3 {
        app.update();
    }

    let body = app.world().get::<Children>(root).unwrap()[0];
    let text = app.world().get::<Children>(body).unwrap()[0];
    let block = app
        .world()
        .get::<bevy::text::ComputedTextBlock>(text)
        .unwrap();
    let heights = block
        .buffer()
        .lines()
        .map(|line| line.metrics().line_height)
        .collect::<Vec<_>>();
    assert_eq!(heights, [17.0; 5]);
}
