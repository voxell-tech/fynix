// TODO(nixon): T: 'static shouldn't be forced.
// TODO(nixon): Support custom type constraints.

/// Generates a sealed downcasting trait for a generic
/// wrapper type.
///
/// The produced trait enables safe `downcast_ref` /
/// `downcast_mut` on `dyn Trait` objects without
/// `std::any::Any`. The trait is sealed so only the
/// wrapper type can implement it, making the pointer casts
/// sound.
///
/// # Usage
///
/// ```ignore
/// any_wrapper!(mod my_mod {
///     pub trait MyTrait: MyWrapper<K> {}
/// });
/// ```
///
/// This expands to a sealed trait `MyTrait<K>` implemented
/// for `MyWrapper<K, T>` for all `T: 'static`. The
/// resulting `dyn MyTrait<K>` object exposes:
///
/// - `element_is::<T>()` — type check
/// - `downcast_ref::<T>()` — checked shared borrow
/// - `downcast_mut::<T>()` — checked mutable borrow
/// - `downcast_unchecked_ref::<T>()` — unchecked (unsafe)
/// - `downcast_unchecked_mut::<T>()` — unchecked (unsafe)
#[macro_export]
macro_rules! any_wrapper {
    (mod $seal:ident { $v:vis trait $name:ident: $wrapper:ident {} }) => {
        $crate::any_wrapper!(mod $seal { $v trait $name: $wrapper<> {} });
    };

    ({mod $seal:ident { $v:vis trait $name:ident: $wrapper:ident {} }}) => {
        $crate::any_wrapper!(mod $seal { $v trait $name: $wrapper<> {} });
    };

    ({mod $seal:ident { $v:vis trait $name:ident: $wrapper:ident<$($generic:ident),*> {} }}) => {
        $crate::any_wrapper!(mod $seal { $v trait $name: $wrapper<$($generic),*> {} });
    };


    (mod $seal:ident { $v:vis trait $name:ident: $wrapper:ident<$($generic:ident),*> {} }) => {
        mod $seal {
            use core::any::TypeId;
            use super::$wrapper;

            /// Private trait to prevent other types from implementing
            #[doc = concat!("the [`", stringify!($name), "`] trait.")]
            trait Seal {}

            impl<$($generic,)* T: 'static> Seal for $wrapper<$($generic,)* T> {}

            #[expect(private_bounds)]
            pub trait $name<$($generic,)*>: Seal {
                fn element_type_id(&self) -> TypeId;
            }

            impl<$($generic,)* T: 'static> $name<$($generic,)*> for $wrapper<$($generic,)* T> {
                fn element_type_id(&self) -> TypeId {
                    TypeId::of::<T>()
                }
            }

            impl<$($generic,)*> dyn $name<$($generic,)*> {
                #[inline]
                pub fn element_is<T: 'static>(&self) -> bool {
                    // Compare both `TypeId`s on equality.
                    self.element_type_id() == TypeId::of::<T>()
                }

                #[allow(unused)]
                #[inline]
                pub fn downcast_ref<T: 'static>(
                    &self,
                ) -> Option<&$wrapper<$($generic,)* T>> {
                    if self.element_is::<T>() {
                        // SAFETY: Just checked whether we are
                        // pointing to the correct type, and we
                        // can rely on that check for memory
                        // safety because the trait is sealed and
                        // is only ever implemented for the wrapper.
                        unsafe { Some(self.downcast_unchecked_ref()) }
                    } else {
                        None
                    }
                }

                #[allow(unused)]
                #[inline]
                pub fn downcast_mut<T: 'static>(
                    &mut self,
                ) -> Option<&mut $wrapper<$($generic,)* T>> {
                    if self.element_is::<T>() {
                        // SAFETY: Just checked whether we are
                        // pointing to the correct type, and we
                        // can rely on that check for memory
                        // safety because the trait is sealed and
                        // is only ever implemented for the wrapper.
                        unsafe { Some(self.downcast_unchecked_mut()) }
                    } else {
                        None
                    }
                }

                #[inline]
                /// # Safety
                ///
                /// Calling this method with the incorrect type is
                /// *undefined behavior*.
                pub unsafe fn downcast_unchecked_ref<T: 'static>(
                    &self,
                ) -> &$wrapper<$($generic,)* T> {
                    debug_assert!(self.element_is::<T>());
                    // SAFETY: caller guarantees that T is the
                    // correct type
                    unsafe {
                        &*(self as *const Self as *const $wrapper<$($generic,)* T>)
                    }
                }

                #[inline]
                /// # Safety
                ///
                /// Calling this method with the incorrect type is
                /// *undefined behavior*.
                pub unsafe fn downcast_unchecked_mut<T: 'static>(
                    &mut self,
                ) -> &mut $wrapper<$($generic,)* T> {
                    debug_assert!(self.element_is::<T>());
                    // SAFETY: caller guarantees that T is the
                    // correct type
                    unsafe {
                        &mut *(self as *mut Self as *mut $wrapper<$($generic,)* T>)
                    }
                }
            }
        }

        #[allow(unused)]
        $v use $seal::$name;
    };
}
