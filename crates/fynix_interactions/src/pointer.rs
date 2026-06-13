use spatree::kurbo::Point;

pub struct Click;
pub struct Hover;

pub trait PointerEvent {
    fn location(&self) -> Point;
    fn is_pressed(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerId(u8);

impl PointerId {
    pub const fn new(id: u8) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerButton(u8);

impl PointerButton {
    const PRIMARY_ID: u8 = 0;
    const SECONDARY_ID: u8 = 1;
    const MIDDLE_ID: u8 = 2;

    pub const PRIMARY: Self = Self::new(Self::PRIMARY_ID);
    pub const SECONDARY: Self = Self::new(Self::SECONDARY_ID);
    pub const MIDDLE: Self = Self::new(Self::MIDDLE_ID);

    pub const fn new(id: u8) -> Self {
        Self(id)
    }

    pub const fn is_primary(&self) -> bool {
        matches!(self.0, Self::PRIMARY_ID)
    }

    pub const fn is_secondary(&self) -> bool {
        matches!(self.0, Self::SECONDARY_ID)
    }

    pub const fn is_middle(&self) -> bool {
        matches!(self.0, Self::MIDDLE_ID)
    }
}
