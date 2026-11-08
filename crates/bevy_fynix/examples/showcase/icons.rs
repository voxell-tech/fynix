//! Line icons drawn in code, so the example loads no files.

use bevy::asset::{Assets, Handle, RenderAssetUsages, uuid_handle};
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat,
};

pub const PLUS: Handle<Image> =
    uuid_handle!("835268fc-7b11-4cae-b6b8-bd5ff48dc14e");
pub const CLOSE: Handle<Image> =
    uuid_handle!("edf61d1b-6df3-481f-83f9-d822b98227a5");
/// Points up.
pub const CHEVRON: Handle<Image> =
    uuid_handle!("b28faa72-c1d1-4480-8da3-0355bb9498d6");
pub const PLAY: Handle<Image> =
    uuid_handle!("454f5ed6-527a-4870-93ac-c0523f5224f6");
pub const PAUSE: Handle<Image> =
    uuid_handle!("ada50446-ac24-4e4f-987b-36087c9622eb");
pub const REWIND: Handle<Image> =
    uuid_handle!("bb9be746-dad3-4acb-b935-49c6eb389cfa");
pub const LAYER: Handle<Image> =
    uuid_handle!("240cfa1d-1798-4463-aa17-93ba5b752eb3");

/// The polylines an icon is drawn along, in units.
type Lines = &'static [&'static [(f32, f32)]];

/// Pixels per unit of the 16 unit square each icon is drawn in.
const SCALE: f32 = 4.0;
/// The pen's width, in units.
const PEN: f32 = 1.5;

/// Draws every icon into the world's images.
pub fn draw(world: &mut World) {
    let mut images = world.resource_mut::<Assets<Image>>();
    let shapes: [(Handle<Image>, Lines); 7] = [
        (
            PLUS,
            &[&[(8.0, 3.0), (8.0, 13.0)], &[(3.0, 8.0), (13.0, 8.0)]],
        ),
        (
            CLOSE,
            &[
                &[(4.5, 4.5), (11.5, 11.5)],
                &[(11.5, 4.5), (4.5, 11.5)],
            ],
        ),
        (CHEVRON, &[&[(3.5, 10.5), (8.0, 6.0), (12.5, 10.5)]]),
        (
            PLAY,
            &[&[(5.0, 3.0), (13.0, 8.0), (5.0, 13.0), (5.0, 3.0)]],
        ),
        (
            PAUSE,
            &[
                &[(5.5, 3.5), (5.5, 12.5)],
                &[(10.5, 3.5), (10.5, 12.5)],
            ],
        ),
        (
            REWIND,
            &[
                &[(3.5, 3.5), (3.5, 12.5)],
                &[(12.5, 3.5), (6.5, 8.0), (12.5, 12.5), (12.5, 3.5)],
            ],
        ),
        (
            LAYER,
            &[&[
                (8.0, 2.5),
                (13.5, 8.0),
                (8.0, 13.5),
                (2.5, 8.0),
                (8.0, 2.5),
            ]],
        ),
    ];
    for (handle, lines) in shapes {
        images.insert(&handle, icon(lines)).unwrap();
    }
}

/// White ink along each polyline of `lines`, on a clear square.
fn icon(lines: &[&[(f32, f32)]]) -> Image {
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
    Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// How far `at` is from the segment from `a` to `b`.
fn distance(at: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let t = ((at - a).dot(along) / along.length_squared())
        .clamp(0.0, 1.0);
    at.distance(a + along * t)
}
