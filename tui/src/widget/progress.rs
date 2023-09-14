// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use ratatui::{
    prelude::{Buffer, Rect},
    style::Style,
    widgets::Widget,
};

const SYMBOLS: &[char] = &[' ', '-', '=', '≡', '■'];
const LEVELS: usize = SYMBOLS.len() - 1;

pub enum Fill {
    UpAcross,
    AcrossUp,
}

pub fn progress(pct: f32, fill: Fill, num_bars: u16) -> impl Widget {
    Progress {
        pct,
        fill,
        num_bars,
    }
}

pub struct Progress {
    pct: f32,
    fill: Fill,
    num_bars: u16,
}

impl Widget for Progress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let pct_str = format!("{:>3}% |", (self.pct * 100.0) as u8);

        buf.set_string(area.x, area.y, &pct_str, Style::default());

        let space = pct_str.len() as u16;

        for i in 0..self.num_bars {
            let x = area.x + space + i;
            let y = area.y;

            let char = match self.fill {
                Fill::UpAcross => up_across(self.pct, self.num_bars, i),
                Fill::AcrossUp => across_up(self.pct, self.num_bars, i),
            };

            buf.get_mut(x, y).set_char(char);
        }

        buf.get_mut(space + self.num_bars, area.y).set_char('|');
    }
}

fn up_across(pct: f32, num_bars: u16, i: u16) -> char {
    let x_pct = pct * num_bars as f32;
    let y_pct = f32::clamp(x_pct - i as f32, 0.0, 1.0);

    let index = (y_pct * LEVELS as f32) as usize;

    SYMBOLS[index]
}

fn across_up(pct: f32, num_bars: u16, i: u16) -> char {
    let y_pct = pct / (1.0 / LEVELS as f32);
    let x_pct = y_pct.fract() * num_bars as f32;
    let partial = f32::clamp(x_pct - i as f32, 0.0, 1.0);

    let index = y_pct as usize + partial as usize;

    SYMBOLS[index]
}
