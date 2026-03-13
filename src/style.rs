use core::hash::Hash;

use field_path::accessor::UntypedAccessor;

use crate::type_map::TypeMaps;

pub struct Style<K, S>(
    fn(&mut S, &K, &UntypedAccessor, &TypeMaps<K>),
);

impl<K: Hash + Eq + 'static, S: 'static> Style<K, S> {
    pub const fn new<T: 'static + Clone>() -> Self {
        Self(
            #[inline]
            |source: &mut S,
             key: &K,
             accessor: &UntypedAccessor,
             map: &TypeMaps<K>| {
                let Some(accessor) = accessor.typed::<S, T>() else {
                    return;
                };

                let Some(value) = map.get::<T>(key) else {
                    return;
                };

                *accessor.get_mut(source) = value.clone();
            },
        )
    }

    pub fn set(
        &self,
        source: &mut S,
        key: &K,
        accessor: &UntypedAccessor,
        map: &TypeMaps<K>,
    ) {
        (self.0)(source, key, accessor, map);
    }
}

// pub fn test() {
//     let mut values = TypeMaps::<u32>::new();
//     let mut registries = FieldAccessorRegistry::new();

//     let id = 1u32;

//     values.insert(id, 10.0);
// }
