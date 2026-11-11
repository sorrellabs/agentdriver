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

/// The key's name as rule logs write it: `enter`, `down`, `ctrl+c`, `a`.
impl std::fmt::Display for Key {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Enter => formatter.write_str("enter"),
            Key::Esc => formatter.write_str("esc"),
            Key::Tab => formatter.write_str("tab"),
            Key::ShiftTab => formatter.write_str("shift+tab"),
            Key::Up => formatter.write_str("up"),
            Key::Down => formatter.write_str("down"),
            Key::Left => formatter.write_str("left"),
            Key::Right => formatter.write_str("right"),
            Key::Backspace => formatter.write_str("backspace"),
            Key::Char(' ') => formatter.write_str("space"),
            Key::Char(character) => write!(formatter, "{character}"),
            Key::Ctrl(character) => write!(formatter, "ctrl+{}", character.to_ascii_lowercase()),
            Key::F(number) => write!(formatter, "f{number}"),
        }
    }
}

/// Keys joined by spaces, as one log field: `down enter`.
pub fn key_names(keys: &[Key]) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_logged_by_their_names() {
        assert_eq!(
            key_names(&[Key::Down, Key::Enter, Key::Char('a'), Key::Ctrl('C')]),
            "down enter a ctrl+c"
        );
    }
}
