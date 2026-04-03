use fynix::Fynix;
use fynix_elements::Label;

fn main() {
    let mut fynix = Fynix::new();
    fynix_elements::init_resources(&mut fynix);

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
