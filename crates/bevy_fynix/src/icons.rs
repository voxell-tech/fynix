//! Line icons drawn in code, so neither the views nor an app need
//! image files for them.

use bevy::app::{App, PreStartup};
use bevy::asset::{Assets, Handle, uuid_handle};
use bevy::ecs::system::ResMut;
use bevy::image::Image;
use bevy::math::Vec2;

/// Points up.
pub const CHEVRON: Handle<Image> =
    uuid_handle!("b28faa72-c1d1-4480-8da3-0355bb9498d6");
/// A diagonal cross.
pub const CLOSE: Handle<Image> =
    uuid_handle!("edf61d1b-6df3-481f-83f9-d822b98227a5");
/// An upright plus.
pub const PLUS: Handle<Image> =
    uuid_handle!("835268fc-7b11-4cae-b6b8-bd5ff48dc14e");

/// Polylines in the 16 unit square an icon is drawn in.
pub type Lines = &'static [&'static [(f32, f32)]];

/// Pixels per unit of the 16 unit square an icon is drawn in.
const SCALE: f32 = 4.0;
/// The pen's width, in units.
const PEN: f32 = 1.5;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PreStartup, draw);
}

fn draw(images: Option<ResMut<Assets<Image>>>) {
    let Some(mut images) = images else {
        return;
    };
    let icons: [(Handle<Image>, Lines); 3] = [
        (CHEVRON, &[&[(3.5, 10.5), (8.0, 6.0), (12.5, 10.5)]]),
        (
            CLOSE,
            &[
                &[(4.5, 4.5), (11.5, 11.5)],
                &[(11.5, 4.5), (4.5, 11.5)],
            ],
        ),
        (
            PLUS,
            &[&[(8.0, 3.0), (8.0, 13.0)], &[(3.0, 8.0), (13.0, 8.0)]],
        ),
    ];
    for (handle, lines) in icons {
        let _ = images.insert(&handle, line_icon(lines));
    }
}

/// White ink along each polyline of `lines`, on a clear square of 16
/// units, with a round pen 1.5 units wide. A tone tints it as an
/// [`icon`](crate::views::icon).
pub fn line_icon(lines: Lines) -> Image {
    let side = (16.0 * SCALE) as u32;
    let segments = lines
        .iter()
        .flat_map(|line| line.windows(2))
        .map(|pair| (Vec2::from(pair[0]), Vec2::from(pair[1])))
        .collect::<Vec<_>>();
    let mut data = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let at = (Vec2::new(x as f32, y as f32) + 0.5) / SCALE;
            let near = segments
                .iter()
                .map(|&(a, b)| distance(at, a, b))
                .fold(f32::INFINITY, f32::min);
            let ink = (PEN / 2.0 - near) * SCALE + 0.5;
            let alpha = (ink.clamp(0.0, 1.0) * 255.0) as u8;
            data.extend([255, 255, 255, alpha]);
        }
    }
    // A clear image is already four bytes a pixel, so it only needs
    // its size and pixels.
    let mut image = Image::transparent();
    image.texture_descriptor.size.width = side;
    image.texture_descriptor.size.height = side;
    image.data = Some(data);
    image
}

/// How far `at` is from the segment from `a` to `b`.
fn distance(at: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let t = ((at - a).dot(along) / along.length_squared())
        .clamp(0.0, 1.0);
    at.distance(a + along * t)
}
