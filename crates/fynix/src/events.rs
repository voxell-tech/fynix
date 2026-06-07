use crate::typing::type_pool::TypePool;

/// Fynix-owned queue of typed messages flowing from the UI to the
/// host world.
///
/// Interaction handlers push messages with [`Self::push`]; the
/// backend drains them once per frame with [`Self::drain`]. Each
/// message type lives in its own [`TypePool`] column, so messages of
/// different types never collide.
pub struct Events {
    pool: TypePool,
}

impl Events {
    /// Creates an empty queue.
    pub fn new() -> Self {
        Self {
            pool: TypePool::new(),
        }
    }

    /// Pushes `event` onto the queue for its type.
    pub fn push<E: 'static>(&mut self, event: E) {
        let _ = self.pool.insert(event);
    }

    /// Returns the number of queued events of type `E`.
    pub fn len<E: 'static>(&self) -> usize {
        self.pool.len::<E>()
    }

    /// Returns `true` if no events of type `E` are queued.
    pub fn is_empty<E: 'static>(&self) -> bool {
        self.len::<E>() == 0
    }

    /// Iterates the queued events of type `E` without removing them.
    pub fn iter<E: 'static>(&self) -> impl Iterator<Item = &E> {
        self.pool.iter::<E>()
    }

    /// Removes and yields every queued event of type `E`, leaving the
    /// queue for that type empty.
    pub fn drain<E: 'static>(&mut self) -> impl Iterator<Item = E> {
        self.pool.drain::<E>()
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::Events;

    #[derive(Debug, PartialEq)]
    struct Increment;

    #[derive(Debug, PartialEq)]
    struct SpawnEnemy(u32);

    #[test]
    fn push_then_iter() {
        let mut events = Events::new();
        events.push(Increment);
        events.push(Increment);
        assert_eq!(events.iter::<Increment>().count(), 2);
    }

    #[test]
    fn types_do_not_collide() {
        let mut events = Events::new();
        events.push(Increment);
        events.push(SpawnEnemy(7));
        assert_eq!(events.len::<Increment>(), 1);
        assert_eq!(events.len::<SpawnEnemy>(), 1);
        assert_eq!(
            events.iter::<SpawnEnemy>().next(),
            Some(&SpawnEnemy(7))
        );
    }

    #[test]
    fn drain_empties_the_queue() {
        let mut events = Events::new();
        events.push(SpawnEnemy(1));
        events.push(SpawnEnemy(2));

        let drained =
            events.drain::<SpawnEnemy>().collect::<Vec<_>>();
        assert_eq!(drained, [SpawnEnemy(1), SpawnEnemy(2)]);
        assert!(events.is_empty::<SpawnEnemy>());
    }

    #[test]
    fn drain_absent_type_is_empty() {
        let mut events = Events::new();
        assert_eq!(events.drain::<Increment>().count(), 0);
    }

    #[test]
    fn push_after_drain_reuses_type() {
        let mut events = Events::new();
        events.push(Increment);
        let _ = events.drain::<Increment>().count();
        events.push(Increment);
        assert_eq!(events.len::<Increment>(), 1);
    }
}
