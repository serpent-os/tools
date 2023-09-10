use crate::Frame;

pub trait Program: Sized {
    type Message;

    const LINES: u16;

    fn update(&mut self, message: Self::Message);

    fn draw(&self, frame: &mut Frame);
}
