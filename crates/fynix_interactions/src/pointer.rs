#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerButton(u16);

impl PointerButton {
    pub const PRIMARY: Self = Self::new(0);
    pub const SECONDARY: Self = Self::new(1);
    pub const MIDDLE: Self = Self::new(2);

    pub const fn new(id: u16) -> Self {
        Self(id)
    }
}
