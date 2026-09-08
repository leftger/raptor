#[derive(Debug, Clone, Copy, PartialEq)]
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
