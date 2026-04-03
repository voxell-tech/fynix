use core::any::TypeId;

use hashbrown::HashMap;
use linkme::distributed_slice;

use crate::ctx::FynixCtx;

#[distributed_slice]
pub static ELEMENT_COMPOSERS: [UntypedElementComposer];

/// Typed composer function for element E in world W.
pub type ElementComposerFn<E, W> = fn(&mut E, &mut FynixCtx<W>);

/// Fully type-erased - stores TypeId for both E and W
/// alongside a *const () function pointer.
/// unsafe impl Sync - the pointer is always a fn pointer.
#[derive(Clone)]
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
        debug_assert_eq!(
            TypeId::of::<E>(),
            self.element_id,
            "Element type mismatch"
        );
        debug_assert_eq!(
            TypeId::of::<W>(),
            self.world_id,
            "World type mismatch"
        );

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
    composers: HashMap<ComposerId, UntypedElementComposer>,
}

#[derive(PartialEq, Eq, Hash)]
pub struct ComposerId {
    element_id: TypeId,
    world_id: TypeId,
}

impl Default for ElementComposers {
    fn default() -> Self {
        Self::new()
    }
}

impl From<(TypeId, TypeId)> for ComposerId {
    fn from(value: (TypeId, TypeId)) -> Self {
        Self {
            element_id: value.0,
            world_id: value.1,
        }
    }
}

impl ElementComposers {
    pub fn new() -> Self {
        Self {
            composers: HashMap::new(),
        }
    }

    pub fn construct_from_slice() -> Self {
        let mut composers = HashMap::new();

        for composer in ELEMENT_COMPOSERS {
            composers
                .entry(ComposerId {
                    element_id: composer.element_id,
                    world_id: composer.world_id,
                })
                .or_insert(composer.clone());
        }

        Self { composers }
    }

    pub fn get_composer<E: 'static, W: 'static>(
        &self,
    ) -> Option<&UntypedElementComposer> {
        self.composers.get(&ComposerId::from((
            TypeId::of::<E>(),
            TypeId::of::<W>(),
        )))
    }
}
