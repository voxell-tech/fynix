//! Dragging a tab to the bar of an area, the middle of an area, or
//! one of its edges, with a hint showing where it lands.

use bevy::camera::visibility::Visibility;
use bevy::color::Alpha;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::math::{Rect, Vec2};
use bevy::picking::Pickable;
use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy::text::{
    FontSize, FontWeight, LineBreak, TextColor, TextFont, TextLayout,
};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, ComputedNode,
    GlobalZIndex, Node, PositionType, UiGlobalTransform, UiRect,
    UiScale, px,
};
use bevy::window::SystemCursorIcon;

use super::layout::DockArea;
use super::registry::DockRegistry;
use super::tabs::{DockTab, TabRow};
use super::tree::{DockTree, Edge, NodeId};
use super::{DockTokens, logical, logical_rect};
use crate::Theme;
use crate::cursor::OverrideCursor;
use crate::tokens::Tone;

/// How far the pointer moves before a press on a tab is a drag, in
/// logical pixels.
const THRESHOLD: f32 = 5.0;

/// The outer share of an area on each side that drops on an edge.
const EDGE: f32 = 0.25;

/// The width of the hint over an empty bar.
const EMPTY_HINT: f32 = 40.0;

/// Where the ghost sits against the pointer.
const GRAB: Vec2 = Vec2::new(40.0, 12.0);

const GHOST_Z: i32 = 200;
const HINT_Z: i32 = 190;

/// What a tab dropped now would do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropTarget {
    /// Go to `index` in the bar of `leaf`.
    Tabs { leaf: NodeId, index: usize },
    /// Go to the end of the tabs of `leaf`.
    Area(NodeId),
    /// Go into a new leaf beside `leaf`.
    Edge { leaf: NodeId, edge: Edge },
}

/// The tab dragged, if any.
#[derive(Resource, Default, Debug)]
pub enum DockDrag {
    #[default]
    Idle,
    /// Pressed, and not moved far enough yet.
    Pending {
        node: Entity,
        tab: DockTab,
        start: Vec2,
    },
    Dragging {
        node: Entity,
        tab: DockTab,
        ghost: Entity,
        hint: Option<Entity>,
        target: Option<DropTarget>,
    },
}

/// The bar of a leaf, and where its tabs are.
struct Bar {
    leaf: NodeId,
    rect: Rect,
    tabs: Vec<Rect>,
}

/// What dropping at `cursor` does, and the rect to hint it with.
fn target_at(
    cursor: Vec2,
    bars: &[Bar],
    areas: &[(NodeId, Rect)],
) -> Option<(DropTarget, Rect)> {
    if let Some(bar) =
        bars.iter().find(|bar| bar.rect.contains(cursor))
    {
        return Some(over_bar(cursor, bar));
    }
    let &(leaf, rect) =
        areas.iter().find(|(_, rect)| rect.contains(cursor))?;
    Some(match edge_of(rect, cursor) {
        Some(edge) => {
            (DropTarget::Edge { leaf, edge }, edge_half(rect, edge))
        }
        None => (DropTarget::Area(leaf), rect),
    })
}

/// A slot beside the tab nearest `cursor`, hinted with that side of
/// the tab.
fn over_bar(cursor: Vec2, bar: &Bar) -> (DropTarget, Rect) {
    let nearest =
        bar.tabs.iter().enumerate().min_by(|(_, a), (_, b)| {
            let distance =
                |rect: &Rect| rect.center().distance_squared(cursor);
            distance(a).total_cmp(&distance(b))
        });
    let Some((index, tab)) = nearest else {
        let hint = Rect::new(
            bar.rect.min.x,
            bar.rect.min.y,
            bar.rect.min.x + EMPTY_HINT,
            bar.rect.max.y,
        );
        let target = DropTarget::Tabs {
            leaf: bar.leaf,
            index: 0,
        };
        return (target, hint);
    };
    let centre = tab.center().x;
    let after = cursor.x > centre;
    let hint = if after {
        Rect::new(centre, tab.min.y, tab.max.x, tab.max.y)
    } else {
        Rect::new(tab.min.x, tab.min.y, centre, tab.max.y)
    };
    let target = DropTarget::Tabs {
        leaf: bar.leaf,
        index: index + usize::from(after),
    };
    (target, hint)
}

/// The edge of `rect` the cursor is in the outer share of, if any.
fn edge_of(rect: Rect, cursor: Vec2) -> Option<Edge> {
    let from_centre = (cursor - rect.center()) / rect.size();
    if from_centre.x < -EDGE {
        Some(Edge::Left)
    } else if from_centre.x > EDGE {
        Some(Edge::Right)
    } else if from_centre.y > EDGE {
        Some(Edge::Bottom)
    } else if from_centre.y < -EDGE {
        Some(Edge::Top)
    } else {
        None
    }
}

/// The half of `rect` on the `edge` side.
fn edge_half(rect: Rect, edge: Edge) -> Rect {
    let centre = rect.center();
    match edge {
        Edge::Top => {
            Rect::new(rect.min.x, rect.min.y, rect.max.x, centre.y)
        }
        Edge::Bottom => {
            Rect::new(rect.min.x, centre.y, rect.max.x, rect.max.y)
        }
        Edge::Left => {
            Rect::new(rect.min.x, rect.min.y, centre.x, rect.max.y)
        }
        Edge::Right => {
            Rect::new(centre.x, rect.min.y, rect.max.x, rect.max.y)
        }
    }
}

/// A press that starts on a tab waits for the pointer to move.
pub(super) fn start(
    drag: On<Pointer<DragStart>>,
    tabs: Query<&DockTab>,
    scale: Option<Res<UiScale>>,
    mut state: ResMut<DockDrag>,
) {
    let node = drag.event_target();
    let Ok(&tab) = tabs.get(node) else {
        return;
    };
    *state = DockDrag::Pending {
        node,
        tab,
        start: logical(drag.pointer_location.position, scale),
    };
}

/// Starts the drag once the pointer has moved far enough, then keeps
/// the ghost under the pointer and the hint on the drop target.
#[allow(clippy::too_many_arguments)]
pub(super) fn moved<T: DockTokens>(
    mut drag: On<Pointer<Drag>>,
    mut state: ResMut<DockDrag>,
    tree: Res<DockTree>,
    registry: Res<DockRegistry<T>>,
    theme: Res<Theme<T>>,
    scale: Option<Res<UiScale>>,
    bars: Query<(&TabRow, &ChildOf, &Children)>,
    areas: Query<(&DockArea, &ComputedNode, &UiGlobalTransform)>,
    rects: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut nodes: Query<&mut Node>,
    mut forced: ResMut<OverrideCursor>,
    mut commands: Commands,
) {
    let cursor = logical(drag.pointer_location.position, scale);
    match &mut *state {
        DockDrag::Idle => {}
        DockDrag::Pending { node, tab, start } => {
            if drag.event_target() != *node {
                return;
            }
            drag.propagate(false);
            if cursor.distance(*start) < THRESHOLD {
                return;
            }
            let (node, tab) = (*node, *tab);
            let window = tree
                .leaf(tab.leaf)
                .and_then(|leaf| {
                    leaf.windows
                        .iter()
                        .find(|entry| entry.id == tab.tab)
                })
                .map(|entry| entry.window_id.clone())
                .unwrap_or_default();
            let name = registry
                .get(&window)
                .map_or(window, |kind| kind.name.clone());
            let ghost =
                spawn_ghost(&mut commands, name, cursor, &theme.0);
            commands.entity(node).insert(Visibility::Hidden);
            forced.0 = Some(SystemCursorIcon::Grabbing);
            *state = DockDrag::Dragging {
                node,
                tab,
                ghost,
                hint: None,
                target: None,
            };
        }
        DockDrag::Dragging {
            node,
            tab,
            ghost,
            hint,
            target,
        } => {
            if drag.event_target() != *node {
                return;
            }
            drag.propagate(false);
            if let Ok(mut ghost) = nodes.get_mut(*ghost) {
                ghost.left = px(cursor.x - GRAB.x);
                ghost.top = px(cursor.y - GRAB.y);
            }

            let bars = bars
                .iter()
                .filter_map(|(row, parent, children)| {
                    let (computed, transform) =
                        rects.get(parent.parent()).ok()?;
                    Some(Bar {
                        leaf: row.leaf,
                        rect: logical_rect(computed, transform),
                        tabs: children
                            .iter()
                            .filter_map(|&kid| rects.get(kid).ok())
                            .map(|(computed, transform)| {
                                logical_rect(computed, transform)
                            })
                            .collect(),
                    })
                })
                .collect::<Vec<_>>();
            let areas = areas
                .iter()
                .map(|(area, computed, transform)| {
                    (area.leaf, logical_rect(computed, transform))
                })
                .collect::<Vec<_>>();
            let lone = tree
                .leaf(tab.leaf)
                .is_some_and(|leaf| leaf.windows.len() == 1);
            let next = target_at(cursor, &bars, &areas).filter(
                |(next, _)| match *next {
                    DropTarget::Area(leaf) => leaf != tab.leaf,
                    DropTarget::Edge { leaf, .. } => {
                        !(leaf == tab.leaf && lone)
                    }
                    DropTarget::Tabs { .. } => true,
                },
            );

            *target = next.map(|(next, _)| next);
            let tint = theme.0.tone(Tone::Accent).with_alpha(0.18);
            match (*hint, next) {
                (None, Some((_, rect))) => {
                    *hint =
                        Some(spawn_hint(&mut commands, rect, tint));
                }
                (Some(shown), Some((_, rect))) => {
                    if let Ok(mut shown) = nodes.get_mut(shown) {
                        place(&mut shown, rect);
                    }
                }
                (Some(shown), None) => {
                    commands.entity(shown).try_despawn();
                    *hint = None;
                }
                (None, None) => {}
            }
        }
    }
}

/// Drops the tab on the target the drag ended over.
pub(super) fn end(
    _: On<Pointer<DragEnd>>,
    mut state: ResMut<DockDrag>,
    mut tree: ResMut<DockTree>,
    mut forced: ResMut<OverrideCursor>,
    mut commands: Commands,
) {
    let DockDrag::Dragging {
        node,
        tab,
        ghost,
        hint,
        target,
    } = core::mem::take(&mut *state)
    else {
        *state = DockDrag::Idle;
        return;
    };
    forced.0 = None;
    clean_up(&mut commands, node, ghost, hint);
    match target {
        Some(DropTarget::Tabs { leaf, index }) => {
            tree.insert_tab(tab.tab, leaf, true, Some(index));
        }
        Some(DropTarget::Area(leaf)) => tree.move_tab(tab.tab, leaf),
        Some(DropTarget::Edge { leaf, edge }) => {
            tree.split_tab(leaf, edge, tab.tab);
        }
        None => {}
    }
}

/// Ends the drag without dropping on Escape.
pub(super) fn cancel(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<DockDrag>,
    mut forced: ResMut<OverrideCursor>,
    mut commands: Commands,
) {
    if !keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)) {
        return;
    }
    if let DockDrag::Dragging {
        node, ghost, hint, ..
    } = core::mem::take(&mut *state)
    {
        forced.0 = None;
        clean_up(&mut commands, node, ghost, hint);
    }
}

/// Removes the ghost and the hint, and shows the tab again.
fn clean_up(
    commands: &mut Commands,
    node: Entity,
    ghost: Entity,
    hint: Option<Entity>,
) {
    commands.entity(ghost).try_despawn();
    if let Some(hint) = hint {
        commands.entity(hint).try_despawn();
    }
    commands.entity(node).try_insert(Visibility::Inherited);
}

/// The tab being dragged, following the pointer.
fn spawn_ghost<T: DockTokens>(
    commands: &mut Commands,
    name: String,
    cursor: Vec2,
    theme: &T,
) -> Entity {
    let mut ghost = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(cursor.x - GRAB.x),
            top: px(cursor.y - GRAB.y),
            height: px(theme.row()),
            padding: UiRect::axes(px(8.0), px(3.0)),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(theme.radius())),
            ..Node::default()
        },
        BackgroundColor(theme.fill()),
        GlobalZIndex(GHOST_Z),
        Pickable::IGNORE,
    ));
    ghost.with_children(|ghost| {
        ghost.spawn((
            Text::new(name),
            TextFont {
                font_size: FontSize::Px(theme.body_size()),
                weight: FontWeight::BOLD,
                ..TextFont::default()
            },
            TextColor(theme.tone(Tone::Body)),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..TextLayout::default()
            },
            Pickable::IGNORE,
        ));
    });
    ghost.id()
}

fn spawn_hint(
    commands: &mut Commands,
    rect: Rect,
    tint: bevy::color::Color,
) -> Entity {
    let mut node = Node {
        position_type: PositionType::Absolute,
        border_radius: BorderRadius::all(px(4.0)),
        ..Node::default()
    };
    place(&mut node, rect);
    commands
        .spawn((
            node,
            BackgroundColor(tint),
            GlobalZIndex(HINT_Z),
            Pickable::IGNORE,
        ))
        .id()
}

/// Puts `node` over `rect`.
fn place(node: &mut Node, rect: Rect) {
    node.left = px(rect.min.x);
    node.top = px(rect.min.y);
    node.width = px(rect.width());
    node.height = px(rect.height());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    const LEFT: NodeId = NodeId(1);
    const RIGHT: NodeId = NodeId(2);

    fn bars() -> Vec<Bar> {
        vec![Bar {
            leaf: LEFT,
            rect: rect(0.0, 0.0, 200.0, 20.0),
            tabs: vec![
                rect(0.0, 0.0, 60.0, 20.0),
                rect(62.0, 0.0, 122.0, 20.0),
            ],
        }]
    }

    fn areas() -> Vec<(NodeId, Rect)> {
        vec![
            (LEFT, rect(0.0, 0.0, 200.0, 200.0)),
            (RIGHT, rect(200.0, 0.0, 400.0, 200.0)),
        ]
    }

    #[test]
    fn over_a_bar_the_slot_is_beside_the_nearest_tab() {
        let (target, hint) =
            target_at(Vec2::new(10.0, 10.0), &bars(), &areas())
                .unwrap();
        assert_eq!(
            target,
            DropTarget::Tabs {
                leaf: LEFT,
                index: 0
            }
        );
        assert_eq!(hint, rect(0.0, 0.0, 30.0, 20.0));

        let (target, hint) =
            target_at(Vec2::new(110.0, 10.0), &bars(), &areas())
                .unwrap();
        assert_eq!(
            target,
            DropTarget::Tabs {
                leaf: LEFT,
                index: 2
            }
        );
        assert_eq!(hint, rect(92.0, 0.0, 122.0, 20.0));
    }

    #[test]
    fn past_the_last_tab_is_the_end_of_the_bar() {
        let (target, _) =
            target_at(Vec2::new(190.0, 10.0), &bars(), &areas())
                .unwrap();
        assert_eq!(
            target,
            DropTarget::Tabs {
                leaf: LEFT,
                index: 2
            }
        );
    }

    #[test]
    fn an_empty_bar_takes_the_first_slot() {
        let empty = vec![Bar {
            leaf: RIGHT,
            rect: rect(200.0, 0.0, 400.0, 20.0),
            tabs: Vec::new(),
        }];
        let (target, hint) =
            target_at(Vec2::new(300.0, 10.0), &empty, &areas())
                .unwrap();
        assert_eq!(
            target,
            DropTarget::Tabs {
                leaf: RIGHT,
                index: 0
            }
        );
        assert_eq!(hint, rect(200.0, 0.0, 240.0, 20.0));
    }

    #[test]
    fn the_outer_quarter_of_an_area_is_an_edge() {
        let cases = [
            (Vec2::new(210.0, 100.0), Edge::Left),
            (Vec2::new(390.0, 100.0), Edge::Right),
            (Vec2::new(300.0, 30.0), Edge::Top),
            (Vec2::new(300.0, 190.0), Edge::Bottom),
        ];
        for (cursor, edge) in cases {
            let (target, _) =
                target_at(cursor, &bars(), &areas()).unwrap();
            assert_eq!(
                target,
                DropTarget::Edge { leaf: RIGHT, edge }
            );
        }
    }

    #[test]
    fn an_edge_hints_the_half_it_would_take() {
        let (_, hint) =
            target_at(Vec2::new(210.0, 100.0), &bars(), &areas())
                .unwrap();
        assert_eq!(hint, rect(200.0, 0.0, 300.0, 200.0));
        let (_, hint) =
            target_at(Vec2::new(300.0, 190.0), &bars(), &areas())
                .unwrap();
        assert_eq!(hint, rect(200.0, 100.0, 400.0, 200.0));
    }

    #[test]
    fn the_middle_of_an_area_hints_all_of_it() {
        let (target, hint) =
            target_at(Vec2::new(300.0, 100.0), &bars(), &areas())
                .unwrap();
        assert_eq!(target, DropTarget::Area(RIGHT));
        assert_eq!(hint, rect(200.0, 0.0, 400.0, 200.0));
    }

    #[test]
    fn outside_every_area_there_is_no_target() {
        assert!(
            target_at(Vec2::new(500.0, 100.0), &bars(), &areas())
                .is_none()
        );
    }
}
