use fynix::ctx::FynixCtx;
use fynix::element::Element;
use fynix::{Fynix, fynix};

struct Earth {}

struct Mars {}

struct Dummy {}

impl Element for Dummy {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }
}

#[fynix(compose)]
fn compose_dummy_earth(_: &mut Dummy, _: &mut FynixCtx<Earth>) {
    println!("earth composer!");
}

#[fynix(compose)]
fn compose_dummy_mars(_: &mut Dummy, _: &mut FynixCtx<Mars>) {
    println!("mars composer!");
}

pub fn main() {
    let mut f = Fynix::new();
    let mut earth = Earth {};
    let mut earth_ctx = f.create_ctx(&mut earth, None);
    let _ = earth_ctx.add::<Dummy>();

    let mut mars = Mars {};
    let mut mars_ctx = f.create_ctx(&mut mars, None);
    let _ = mars_ctx.add::<Dummy>();
}
