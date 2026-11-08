//! Named keys. Each host adapter encodes these for its own transport.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Key {
    Enter,
    Esc,
    Tab,
    ShiftTab,
    Up,
    Down,
    Left,
    Right,
    Backspace,
    Char(char),
    Ctrl(char),
    F(u8),
}
