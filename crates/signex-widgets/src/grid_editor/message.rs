use crate::gerber_viewer::GridUnit;

#[derive(Debug, Clone)]
pub enum GerberGridEditorMessage {
    SelectGrid(usize),
    AddGrid,
    SettingsNameChanged(String),
    SettingsXChanged(String),
    SettingsYChanged(String),
    SetSettingsUnit(GridUnit),
    DeleteGrid,
    MoveGridUp,
    MoveGridDown,
    ResetDefaults,
    Apply,
    Cancel,
}
