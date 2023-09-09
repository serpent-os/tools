use std::time::{Duration, Instant};

use ratatui::{
    prelude::{Constraint, Layout},
    style::{Color, Style},
    widgets::LineGauge,
};
use smol::Timer;
use tui::Handle;

fn main() {
    tui::run(Program::default(), run).unwrap();
}

async fn run(mut handle: Handle<Message>) {
    let now = Instant::now();

    let mut progress = 0;

    loop {
        handle.print(format!("{:?}", now.elapsed())).await;

        Timer::after(Duration::from_millis(50)).await;

        progress = (progress + 1) % 100;

        handle.update(Message::Progress(progress)).await;
    }
}

#[derive(Default)]
struct Program {
    progress: f64,
}

enum Message {
    Progress(u8),
}

impl tui::Program for Program {
    const LINES: u16 = 3;

    type Message = Message;

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Progress(p) => self.progress = p as f64 / 100.0,
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<tui::Backend>) {
        let layout = Layout::new()
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(frame.size());

        frame.render_widget(
            LineGauge::default()
                .ratio(self.progress)
                .style(Style::default().fg(Color::Gray))
                .gauge_style(Style::default().fg(Color::Green)),
            layout[1],
        );
    }
}
