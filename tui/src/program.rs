// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use crate::Frame;

/// Moss TUI implementations need to conform with this trait to integrate
/// properly with the asynchronous runtime
pub trait Program: Sized {
    type Message;

    const LINES: u16;

    /// Handle updates in response to a Message
    fn update(&mut self, message: Self::Message);

    /// Draw per the current state
    fn draw(&self, frame: &mut Frame);
}
