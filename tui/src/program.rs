// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use crate::Frame;

pub trait Program: Sized {
    type Message;

    const LINES: u16;

    fn update(&mut self, message: Self::Message);

    fn draw(&self, frame: &mut Frame);
}
