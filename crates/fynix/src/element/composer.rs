use core::any::TypeId;

use hashbrown::HashMap;
use linkme::distributed_slice;

use crate::ctx::FynixCtx;

#[distributed_slice]
pub static ELEMENT_COMPOSERS: [UntypedElementComposer] = [..];

/// Typed composer function for element E in world W.
pub type ElementComposerFn<E, W> = fn(&mut E, &mut FynixCtx<W>);

/// Fully type-erased - stores TypeId for both E and W
/// alongside a *const () function pointer.
/// unsafe impl Sync - the pointer is always a fn pointer.
#[allow(dead_code)]
pub struct UntypedElementComposer {
    element_id: TypeId,
    world_id: TypeId,
    compose_fn: *const (),
}

unsafe impl Sync for UntypedElementComposer {}

impl UntypedElementComposer {
    pub const fn new<E: 'static, W: 'static>(
        f: ElementComposerFn<E, W>,
    ) -> Self {
        Self {
            element_id: TypeId::of::<E>(),
            world_id: TypeId::of::<W>(),
            compose_fn: f as *const (),
        }
    }

    pub fn execute<E: 'static, W: 'static>(
        &self,
        element: &mut E,
        ctx: &mut FynixCtx<W>,
    ) {
        let f = unsafe {
            core::mem::transmute::<*const (), ElementComposerFn<E, W>>(
                self.compose_fn,
            )
        };
        f(element, ctx);
    }
}

/// Non-generic registry keyed on element TypeId.
pub struct ElementComposers {
    composers: HashMap<TypeId, UntypedElementComposer>,
}

impl ElementComposers {
    pub fn new() -> Self {
        Self {
            composers: HashMap::new(),
        }
    }

    pub fn construct_from_slice() -> Self {
        let composers = HashMap::new();

        // TODO!

        Self { composers }
    }

    pub fn get_composer<E: 'static>(
        &self,
    ) -> Option<&UntypedElementComposer> {
        self.composers.get(&TypeId::of::<E>())
    }
}
