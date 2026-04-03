use std::sync::Arc;

use fynix::Fynix;
use fynix_elements::{Label, TextContext};
use parley::fontique::Blob;

const FONT: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

fn main() {
    let mut fynix = Fynix::new();
    fynix_elements::init_resources(&mut fynix);

    if let Some(text_cx) =
        fynix.resources_mut().get_mut::<TextContext>()
    {
        let blob = Blob::new(Arc::new(FONT));
        let ids =
            text_cx.font_cx.collection.register_fonts(blob, None);
        text_cx.font_cx.collection.set_generic_families(
            parley::fontique::GenericFamily::SansSerif,
            ids.into_iter().map(|(f, _)| f),
        );
    }

    let mut world = ();
    let label_id = {
        let mut ctx = fynix.root_ctx(&mut world);
        ctx.add_with::<Label>(|label, _ctx| {
            label.text = "Hello, Fynix!".into();
            label.font_size = 24.0;
        })
    };

    fynix.layout(&label_id);

    let size = fynix
        .elements()
        .metas
        .get(&label_id)
        .map(|m| m.node.size)
        .expect("Label node not found.");

    println!("Label size: {}x{}", size.width, size.height);
    assert!(size.width > 0.0, "Expected non-zero width.");
    assert!(size.height > 0.0, "Expected non-zero height.");
}
