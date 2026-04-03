use fynix::ctx::FynixCtx;
use fynix::element::Element;
use fynix::{Fynix, fynix};

struct World {
    counter: u32,
}

struct Counter {
    count: u32,
}

impl Element for Counter {
    fn new() -> Self
    where
        Self: Sized,
    {
        Counter { count: 0 }
    }
}

#[fynix(compose)]
fn compose_counter(e: &mut Counter, ctx: &mut FynixCtx<World>) {
    e.count = ctx.world.counter;
    println!("set count to {}", ctx.world.counter);
}

pub fn main() {
    let mut f = Fynix::new();
    let mut world = World { counter: 5 };
    let mut ctx = f.create_ctx(&mut world, None);

    let _ = ctx.add::<Counter>();
}
