//! The showcase's own icons, drawn like the crate's.

use bevy::asset::{Assets, Handle, uuid_handle};
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy_fynix::icons::{Lines, line_icon};

pub const PLAY: Handle<Image> =
    uuid_handle!("454f5ed6-527a-4870-93ac-c0523f5224f6");
pub const PAUSE: Handle<Image> =
    uuid_handle!("ada50446-ac24-4e4f-987b-36087c9622eb");
pub const REWIND: Handle<Image> =
    uuid_handle!("bb9be746-dad3-4acb-b935-49c6eb389cfa");
pub const LAYER: Handle<Image> =
    uuid_handle!("240cfa1d-1798-4463-aa17-93ba5b752eb3");

pub fn draw(world: &mut World) {
    let mut images = world.resource_mut::<Assets<Image>>();
    let icons: [(Handle<Image>, Lines); 4] = [
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
    for (handle, lines) in icons {
        images.insert(&handle, line_icon(lines)).unwrap();
    }
}
