// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    thread,
    time::{Duration, Instant},
};

use tui::{ProgressBar, ProgressStyle};

fn main() {
    let now = Instant::now();

    let progress_bar = ProgressBar::new(100).with_style(
        ProgressStyle::with_template("\n[{bar:20.cyan/blue}] {percent}%")
            .unwrap()
            .progress_chars("■≡=- "),
    );

    let mut progress = 0;

    loop {
        progress_bar.println(format!("{:?}", now.elapsed()));

        thread::sleep(Duration::from_millis(50));

        progress = (progress + 1) % 100;

        progress_bar.set_position(progress);
    }
}
