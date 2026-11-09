//! A box of editable text with [`Atom`]s between the words.
//!
//! The text is a [`Rich`](super::Rich) laid out by Bevy, with an
//! inline box for each atom, and the caret and selection are
//! parley's, over that layout. Bevy's `EditableText` is not used: its
//! layout has no inline boxes. It needs Bevy's `UiPlugin`, and
//! `DefaultPlugins` adds it.

mod content;
mod edit;
mod input;
mod paint;
mod scroll;

use core::time::Duration;

use bevy::app::{App, PostUpdate, PreUpdate, Update};
use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::color::Color;
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::Changed;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ecs::world::World;
use bevy::input::InputSystems;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::math::Rect;
use bevy::text::{ComputedTextBlock, TextLayoutInfo};
use bevy::time::{Real, Time};
use bevy::ui::widget::text_system;
use bevy::ui::{Overflow, PositionType, UiRect, Val, px};
use bevy::window::SystemCursorIcon;
use content::Content;
pub use content::{Caret, Piece, Plain, PlainAtom};
use edit::Edit;
use parley::editing::Selection;
use parley::{Brush, Layout};
use scroll::{MaxLines, Seen};

use super::atom::{Atom, AtomBox};
use super::field::defaults;
use super::frame::{Frame, FrameProps, frame};
use super::label::label;
use super::rich::{Span, rich};
use super::text_input::call;
use crate::cursor::EntityCursor;
use crate::prop::{Prop, component};
use crate::scroll::ScrollGoal;
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens, Tone,
};
use crate::{Bevy, Cx, Styled, View};

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PreUpdate, input::ime.after(InputSystems))
        .add_systems(
            Update,
            notify.in_set(crate::FynixSystems::Settle),
        )
        .add_systems(
            PostUpdate,
            (
                apply,
                scroll::cap,
                scroll::reveal,
                paint::paint,
                paint::ime_window,
            )
                .chain()
                .after(text_system),
        );
}

/// What Enter does in a [`RichFieldView`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Enter {
    /// Starts a new line.
    #[default]
    Newline,
    /// Runs the submit handler, and Shift and Enter start a new line.
    Submit,
}

/// How high a line of text is, in its font size, as Bevy lays it out
/// unless told otherwise.
const LINE_HEIGHT: f32 = 1.2;

type Handler = Box<dyn Fn(&mut World, Vec<Piece>) + Send + Sync>;

/// Run with the pieces of a field when its content changes.
#[derive(Component)]
struct OnChange(Handler);

/// Run with the pieces of a field when Enter submits it.
#[derive(Component)]
struct OnSubmit(Handler);

/// A field's content, caret and selection, on its root. A system that
/// holds a field's entity edits it through this.
#[derive(Component)]
pub struct RichField {
    content: Content,
    selection: Selection,
    /// A caret asked for, which the layout has yet to show: the
    /// selection until the layout catches up.
    at: Option<usize>,
    pending: Vec<Edit>,
    /// A paste the clipboard has yet to give, which the pending
    /// edits wait for.
    paste: Option<ClipboardRead>,
    /// What an input method is composing at the caret.
    preedit: Option<String>,
    /// The text node, whose layout the selection is over.
    text: Entity,
    enter: Enter,
    /// How many changes the content has had, and how many of them
    /// the change handler has been told of.
    version: u64,
    notified: u64,
    caret: Entity,
    selected: Vec<Entity>,
    caret_color: Color,
    selection_color: Color,
    /// When the caret last moved, for it to blink from.
    active: Duration,
    /// Where the caret was when the view last scrolled to show it.
    seen: Option<Seen>,
    /// The first line of the view, once it holds more lines than it
    /// shows.
    first: usize,
}

impl RichField {
    fn new(
        text: Entity,
        pieces: impl IntoIterator<Item = Piece>,
    ) -> Self {
        Self {
            content: Content::new(pieces),
            selection: Selection::default(),
            at: None,
            pending: Vec::new(),
            paste: None,
            preedit: None,
            text,
            enter: Enter::default(),
            version: 0,
            notified: 0,
            caret: Entity::PLACEHOLDER,
            selected: Vec::new(),
            caret_color: Color::WHITE,
            selection_color: Color::NONE,
            active: Duration::ZERO,
            seen: None,
            first: 0,
        }
    }

    /// What the field holds, in order, as text and atoms.
    pub fn pieces(&self) -> Vec<Piece> {
        self.content.pieces()
    }

    /// The text of the field with no atom in it, and where each atom
    /// stood in it: what an app sends.
    pub fn plain(&self) -> Plain {
        self.content.plain(0..self.content.len())
    }

    /// The text of the field with each atom as the text it copies as:
    /// what copying all of it puts on the clipboard.
    pub fn copy_text(&self) -> String {
        self.content.copy_text(0..self.content.len())
    }

    /// Whether the field holds no text and no atom.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Replaces the content, with the caret at its end. Edits still
    /// waiting are dropped.
    pub fn set_content(
        &mut self,
        pieces: impl IntoIterator<Item = Piece>,
    ) {
        self.content = Content::new(pieces);
        self.version += 1;
        self.at = Some(self.content.len());
        self.pending.clear();
        self.paste = None;
        self.preedit = None;
    }

    /// Empties the field, as [`set_content`](Self::set_content) with
    /// nothing does.
    pub fn clear(&mut self) {
        self.set_content([]);
    }

    /// Puts `atom` at the caret, in place of what is selected.
    pub fn insert_atom(&mut self, atom: Atom) {
        self.pending.push(Edit::InsertAtom(atom));
    }

    /// Types `text` at the caret, in place of what is selected.
    pub fn insert_text(&mut self, text: impl Into<String>) {
        self.pending.push(Edit::Insert(text.into()));
    }

    /// Takes out the atoms with `key`. A caret after one of them
    /// stays by what it was by, and a selection is dropped.
    pub fn remove_atom(&mut self, key: impl Into<String>) {
        self.pending.push(Edit::RemoveAtom(key.into()));
    }

    /// Puts the caret at `caret`, dropping the selection. Nothing
    /// happens for the side of an atom the field has none of.
    pub fn set_caret(&mut self, caret: Caret) {
        self.pending.push(Edit::SetCaret(caret));
    }

    /// Whether there is nothing to show but a placeholder.
    fn is_blank(&self) -> bool {
        self.content.is_empty() && self.preedit.is_none()
    }

    fn caret_index(&self) -> usize {
        self.at
            .unwrap_or_else(|| self.selection.focus().index())
            .min(self.content.len())
    }

    fn spans(&self) -> Vec<Span> {
        let preedit = self
            .preedit
            .as_deref()
            .map(|typing| (self.caret_index(), typing));
        self.content.spans(preedit)
    }

    /// Whether `layout` is not yet the layout of the spans: it is not
    /// safe to ask it about the content until it is.
    fn is_stale<B: Brush>(&self, layout: &Layout<B>) -> bool {
        let expected = self.content.len()
            + self.preedit.as_ref().map_or(0, String::len);
        layout
            .len()
            .checked_sub(1)
            .and_then(|last| layout.get(last))
            .is_none_or(|line| {
                let end = line.text_range().end;
                // An empty text is given a character to hold.
                end != expected && !(expected == 0 && end == 1)
            })
    }
}

/// An editable box of text and atoms, in a [`Frame`] drawn as a
/// [`TextField`](super::TextField) is.
///
/// It is not bound to a value: what it holds is read and changed
/// through its [`RichField`]. It wraps its lines, and grows with
/// them, up to [`max_lines`](Self::max_lines).
pub struct RichFieldView {
    pub frame: Frame,
    placeholder: Prop<String>,
    enter: Enter,
    max_lines: Option<usize>,
    start: Vec<Piece>,
    on_change: Option<Handler>,
    on_submit: Option<Handler>,
}

/// An empty field of text and atoms.
pub fn rich_field() -> RichFieldView {
    RichFieldView {
        frame: Frame::unset(),
        placeholder: Prop::Unset,
        enter: Enter::default(),
        max_lines: None,
        start: Vec::new(),
        on_change: None,
        on_submit: None,
    }
}

impl RichFieldView {
    /// What shows, dim, while the field is empty.
    pub fn placeholder(
        mut self,
        placeholder: impl Into<Prop<String>>,
    ) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// What Enter does in the field.
    pub fn enter(mut self, enter: Enter) -> Self {
        self.enter = enter;
        self
    }

    /// How many lines it shows before it scrolls, keeping the caret
    /// in view. It grows without end when unset. A line is as high as
    /// it is laid out, so the field is as tall as the lines in view
    /// are, and a tall atom makes it taller while it is in view.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines.max(1));
        self
    }

    /// What the field holds at first.
    pub fn content(
        mut self,
        pieces: impl IntoIterator<Item = Piece>,
    ) -> Self {
        self.start = pieces.into_iter().collect();
        self
    }

    /// Runs `on_change` with what the field holds each time it
    /// changes, whether typed, pasted or set.
    pub fn on_change(
        mut self,
        on_change: impl Fn(&mut World, Vec<Piece>) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    /// Runs `on_submit` with what the field holds when Enter
    /// submits it, as [`Enter::Submit`] says.
    pub fn on_submit(
        mut self,
        on_submit: impl Fn(&mut World, Vec<Piece>) + Send + Sync + 'static,
    ) -> Self {
        self.on_submit = Some(Box::new(on_submit));
        self
    }
}

impl FrameProps for RichFieldView {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T> View<Bevy, T> for RichFieldView
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        cx.scope(|cx| {
            cx.defaults(|cx| {
                cx.root(defaults);
                // The room a one-line text field has above and below
                // its text inside its border, kept as the field
                // grows.
                cx.root(|cx| {
                    cx.set::<Frame>(|frame, theme: &T| {
                        let line = theme.body_size() * LINE_HEIGHT;
                        let above = ((theme.row() - line - 2.0)
                            / 2.0)
                            .max(0.0);
                        frame.padding(UiRect::axes(
                            px(theme.gap()),
                            px(above),
                        ))
                    });
                });
            });
            let root = cx.build(self.frame);
            let mut field =
                RichField::new(Entity::PLACEHOLDER, self.start);
            field.enter = self.enter;
            let theme = cx.theme();
            field.caret_color = theme.tone(Tone::Body);
            field.selection_color = theme.selection();
            cx.world.entity_mut(root).insert(field);

            let spans = component::<RichField, _>(root, |field| {
                field.map(RichField::spans).unwrap_or_default()
            });
            let body = cx.under(root, |cx| {
                cx.build(
                    rich(spans)
                        .atoms()
                        .grow(1.0)
                        .min_width(px(0.0))
                        .overflow(Overflow::scroll_y()),
                )
            });
            cx.world.entity_mut(body).insert(ScrollGoal::default());
            let text = cx
                .world
                .get::<Children>(body)
                .and_then(|kids| kids.first().copied())
                .expect("the text of the field");
            let blank = component::<RichField, _>(root, |field| {
                field.is_some_and(RichField::is_blank)
            })
            .map(|blank| if blank { 1.0 } else { 0.0 });
            cx.under(body, |cx| {
                let holder = cx.build(
                    frame().position(PositionType::Absolute).inset(
                        UiRect {
                            left: px(0.0),
                            top: px(0.0),
                            ..UiRect::all(Val::Auto)
                        },
                    ),
                );
                cx.under(holder, |cx| {
                    cx.build(
                        label(self.placeholder)
                            .tone(Tone::Faint)
                            .opacity(blank),
                    )
                });
            });
            let caret = paint::spawn_caret(cx.world, body);

            let mut entity = cx.world.entity_mut(root);
            if let Some(mut field) = entity.get_mut::<RichField>() {
                field.text = text;
                field.caret = caret;
                field.notified = field.version;
            }
            entity.insert((
                TabIndex(0),
                EntityCursor(SystemCursorIcon::Text),
            ));
            entity
                .observe(input::key)
                .observe(input::press)
                .observe(input::drag)
                .observe(input::gained)
                .observe(input::lost);
            if let Some(on_change) = self.on_change {
                entity.insert(OnChange(on_change));
            }
            if let Some(on_submit) = self.on_submit {
                entity.insert(OnSubmit(on_submit));
            }
            if let Some(lines) = self.max_lines {
                entity.insert(MaxLines(lines));
            }
            root
        })
    }
}

/// Runs the handler of each field whose content changed.
fn notify(
    fields: Query<(Entity, &RichField), Changed<RichField>>,
    mut commands: Commands,
) {
    for (node, field) in &fields {
        if field.version == field.notified {
            continue;
        }
        commands.queue(move |world: &mut World| {
            let Some(mut field) = world.get_mut::<RichField>(node)
            else {
                return;
            };
            if field.version == field.notified {
                return;
            }
            field.notified = field.version;
            let pieces = field.pieces();
            call::<OnChange>(world, node, |change, world| {
                (change.0)(world, pieces);
            });
        });
    }
}

/// Makes the edits the fields have waited for, on the layout of
/// their text. A change to the content leaves the rest for the next
/// frame, when the layout shows it.
fn apply(
    mut fields: Query<&mut RichField>,
    texts: Query<(&ComputedTextBlock, &TextLayoutInfo)>,
    chips: Query<&AtomBox>,
    mut clipboard: Option<ResMut<Clipboard>>,
    time: Option<Res<Time<Real>>>,
) {
    for mut field in &mut fields {
        let waiting = field.bypass_change_detection();
        if let Some(read) = waiting.paste.as_mut() {
            let result = read.poll_result();
            if matches!(read, ClipboardRead::Pending(_)) {
                continue;
            }
            waiting.paste = None;
            if let Some(Ok(text)) = result
                && !text.is_empty()
            {
                waiting
                    .pending
                    .insert(0, Edit::Insert(edit::pasted(&text)));
            }
        }
        if field.bypass_change_detection().pending.is_empty() {
            continue;
        }
        let Ok((block, info)) = texts.get(field.text) else {
            continue;
        };
        let layout = block.buffer();
        if field.is_stale(layout) {
            continue;
        }
        let boxes = info
            .inline_boxes
            .iter()
            .map(|(entity, _, rect)| {
                chips.get(*entity).map_or(*rect, |held| held.chip)
            })
            .collect::<Vec<Rect>>();
        field.active = time
            .as_ref()
            .map_or(Duration::ZERO, |time| time.elapsed());
        while !field.pending.is_empty() {
            let edit = field.pending.remove(0);
            if field.apply(
                edit,
                layout,
                &boxes,
                clipboard.as_deref_mut(),
            ) {
                break;
            }
        }
    }
}
