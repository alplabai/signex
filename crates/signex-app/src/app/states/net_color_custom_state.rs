/// Custom net-colour picker state (Active Bar -> Net Color -> Custom).
#[derive(Debug, Clone)]
pub struct NetColorCustomState {
    pub show: bool,
    pub draft: iced::Color,
}

impl Default for NetColorCustomState {
    fn default() -> Self {
        Self {
            show: false,
            draft: iced::Color::from_rgb(0.40, 0.40, 0.93),
        }
    }
}
