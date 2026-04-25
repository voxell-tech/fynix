use core::any::TypeId;

use hashbrown::HashMap;

use crate::signal::SignalId;

pub struct SignalMeta {
    pub(super) signal_id: TypeId,
}

pub struct SignalMetas {
    map: HashMap<SignalId, SignalMeta>,
}

impl SignalMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn init<T: 'static>(&mut self, id: SignalId) {
        self.map.insert(
            id,
            SignalMeta {
                signal_id: TypeId::of::<T>(),
            },
        );
    }

    pub fn remove(&mut self, id: &SignalId) -> Option<SignalMeta> {
        self.map.remove(id)
    }

    pub fn get(&self, id: &SignalId) -> Option<&SignalMeta> {
        self.map.get(id)
    }
}

impl Default for SignalMetas {
    fn default() -> Self {
        Self::new()
    }
}
