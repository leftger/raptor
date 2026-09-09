use bevy::prelude::Message;

/// High-level commands emitted by keyboard/mouse systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub enum Command {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    GoToFirst,
    GoToLast,
    OpenSelected,
    GoBack,
    GoToParent,
    GoToRoot,
    GoHome,
    ReloadDirectory,
    ToggleLabels,
    ToggleHidden,
    RevealInFileManager,
}
