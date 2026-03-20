use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    pub sidebar: Rect,
    pub messages: Rect,
    pub input: Rect,
}

impl AppLayout {
    pub fn new(area: Rect, input_lines: u16) -> Self {
        // Sidebar width: wider for tool call tree display
        let sidebar_width = if area.width > 100 {
            24
        } else if area.width > 60 {
            20
        } else {
            16
        };

        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sidebar_width), // sidebar
                Constraint::Min(30),               // chat area
            ])
            .split(area);

        // Input height: 1 border + text lines, capped at 1/3 of screen
        let max_input = (h_chunks[1].height / 3).max(2);
        let input_height = (input_lines + 1).clamp(2, max_input); // +1 for top border

        // Chat area: messages + input
        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),                    // messages
                Constraint::Length(input_height),       // input
            ])
            .split(h_chunks[1]);

        Self {
            sidebar: h_chunks[0],
            messages: v_chunks[0],
            input: v_chunks[1],
        }
    }
}
