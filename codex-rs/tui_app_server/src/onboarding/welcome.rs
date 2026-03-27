use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::tui::FrameRequester;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use super::onboarding_screen::StepState;

pub(crate) struct WelcomeWidget {
    pub is_logged_in: bool,
}

impl KeyboardHandler for WelcomeWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let _ = key_event;
    }
}

impl WelcomeWidget {
    pub(crate) fn new(
        is_logged_in: bool,
        _request_frame: FrameRequester,
        _animations_enabled: bool,
    ) -> Self {
        Self { is_logged_in }
    }

    pub(crate) fn update_layout_area(&self, area: Rect) {
        let _ = area;
    }
}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            "  ".into(),
            "Welcome to ".into(),
            "Codex".bold(),
            ", OpenAI's command-line coding agent".into(),
        ]));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl StepStateProvider for WelcomeWidget {
    fn get_step_state(&self) -> StepState {
        match self.is_logged_in {
            true => StepState::Hidden,
            false => StepState::Complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn row_containing(buf: &Buffer, needle: &str) -> Option<u16> {
        (0..buf.area.height).find(|&y| {
            let mut row = String::new();
            for x in 0..buf.area.width {
                row.push_str(buf[(x, y)].symbol());
            }
            row.contains(needle)
        })
    }

    #[test]
    fn welcome_renders_animation_on_first_draw() {
        let widget = WelcomeWidget::new(false, FrameRequester::test_dummy(), true);
        let area = Rect::new(0, 0, 60, 37);
        let mut buf = Buffer::empty(area);
        (&widget).render(area, &mut buf);

        let welcome_row = row_containing(&buf, "Welcome");
        assert_eq!(welcome_row, Some(0));
    }

    #[test]
    fn welcome_skips_animation_below_height_breakpoint() {
        let widget = WelcomeWidget::new(false, FrameRequester::test_dummy(), true);
        let area = Rect::new(0, 0, 60, 36);
        let mut buf = Buffer::empty(area);
        (&widget).render(area, &mut buf);

        let welcome_row = row_containing(&buf, "Welcome");
        assert_eq!(welcome_row, Some(0));
    }

    #[test]
    fn ctrl_dot_changes_animation_variant() {
        let mut widget = WelcomeWidget::new(false, FrameRequester::test_dummy(), true);

        widget.handle_key_event(KeyEvent::new(
            crossterm::event::KeyCode::Char('.'),
            crossterm::event::KeyModifiers::CONTROL,
        ));

        assert!(!widget.is_logged_in);
    }
}
