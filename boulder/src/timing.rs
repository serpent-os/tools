use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use crate::{architecture::BuildTarget, build};

const PROGRESS_WIDTH: usize = 6;
const ELAPSED_WIDTH: usize = 13;

#[derive(Default)]
pub struct Timing {
    startup: Duration,
    build: BTreeMap<BuildTarget, BTreeMap<Option<build::pgo::Stage>, BTreeMap<build::job::Phase, BuildEntry>>>,
    analysis: Duration,
    packaging: Duration,
}

impl Timing {
    pub fn begin(&mut self, kind: Kind) -> Timer {
        Timer(kind, Instant::now())
    }

    pub fn finish(&mut self, timer: Timer) {
        let elapsed = timer.1.elapsed();

        match timer.0 {
            Kind::Build(
                build @ Build {
                    target,
                    pgo_stage,
                    phase,
                },
            ) => {
                self.build
                    .entry(target)
                    .or_default()
                    .entry(pgo_stage)
                    .or_default()
                    .insert(phase, BuildEntry { build, elapsed });
            }
            Kind::Startup => self.startup = elapsed,
            Kind::Analysis => self.analysis = elapsed,
            Kind::Packaging => self.packaging = elapsed,
        }
    }

    pub fn print_table(&self) {
        let max_prefix_length = self
            .build
            .values()
            .flat_map(|stages| {
                stages
                    .values()
                    .flat_map(|phases| phases.values().map(BuildEntry::max_prefix_length))
            })
            .max()
            .unwrap_or_default()
            // No-op (less than "Packaging")
            // .max("Startup".len())
            // .max("Analysis".len())
            .max("Packaging".len());
        let total_elapsed = self
            .build
            .values()
            .flat_map(|stages| stages.values().flat_map(|phases| phases.values().map(|e| e.elapsed)))
            .sum::<Duration>()
            + self.startup
            + self.analysis
            + self.packaging;

        println!(
            "P{:<max_prefix_length$}  {:>ELAPSED_WIDTH$} {:>PROGRESS_WIDTH$}",
            "hases", "Elapsed", "%",
        );
        println!(
            "│{:<max_prefix_length$}  {} {}",
            "Startup",
            fmt_elapsed(self.startup),
            fmt_progress(self.startup, total_elapsed)
        );

        for (target, stages) in &self.build {
            println!("│{}", build::build_target_prefix(*target, 0),);

            for (stage, phases) in stages {
                if let Some(stage) = stage {
                    println!("│{}", build::pgo_stage_prefix(*stage, 0),);
                }

                for (phase, entry) in phases {
                    let gap = max_prefix_length - (phase.to_string().len() + if stage.is_some() { 2 } else { 1 });

                    println!(
                        "│{}{}  {} {}",
                        build::phase_prefix(*phase, stage.is_some(), 0),
                        " ".repeat(gap),
                        fmt_elapsed(entry.elapsed),
                        fmt_progress(entry.elapsed, total_elapsed),
                    );
                }
            }
        }

        println!(
            "│{:<max_prefix_length$}  {} {}",
            "Analysis",
            fmt_elapsed(self.analysis),
            fmt_progress(self.analysis, total_elapsed)
        );
        println!(
            "│{:<max_prefix_length$}  {} {}",
            "Packaging",
            fmt_elapsed(self.packaging),
            fmt_progress(self.packaging, total_elapsed),
        );
        println!(
            "{}",
            "─".repeat(1 + max_prefix_length + 2 + ELAPSED_WIDTH + 1 + PROGRESS_WIDTH),
        );
        println!(
            "T{:<max_prefix_length$}  {} {}",
            "otal",
            fmt_elapsed(total_elapsed),
            fmt_progress(total_elapsed, total_elapsed)
        );
        println!();
    }
}

pub struct Timer(Kind, Instant);

pub enum Kind {
    Startup,
    Build(Build),
    Analysis,
    Packaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Build {
    pub target: BuildTarget,
    pub pgo_stage: Option<build::pgo::Stage>,
    pub phase: build::job::Phase,
}

struct BuildEntry {
    build: Build,
    elapsed: Duration,
}

impl BuildEntry {
    pub fn max_prefix_length(&self) -> usize {
        self.build
            .target
            .to_string()
            .len()
            .max(
                self.build
                    .pgo_stage
                    .map(|stage| stage.to_string().len() + 1)
                    .unwrap_or_default(),
            )
            .max(self.build.phase.to_string().len() + if self.build.pgo_stage.is_some() { 2 } else { 1 })
    }
}

/// Format a template of `000h00m00.00s`, removing
/// leading zeros for spaces if the duration is
/// too small
fn fmt_elapsed(duration: Duration) -> String {
    let total_seconds = duration.as_secs_f32();
    let total_minutes = total_seconds as u64 / 60;
    let total_hours = total_minutes / 60;

    // Only pad zeros if next unit exists
    let seconds = if total_minutes >= 1 {
        format!("{:0>5.2}s", total_seconds % 60.0)
    } else {
        format!("{:>5.2}s", total_seconds % 60.0)
    };

    let minutes = if total_minutes >= 1 {
        // Only pad zeros if next unit exists
        if total_hours >= 1 {
            format!("{total_minutes:0>2}m")
        } else {
            format!("{total_minutes:>2}m")
        }
    } else {
        " ".repeat(3)
    };

    let hours = if total_hours >= 1 {
        format!("{total_hours:>3}h")
    } else {
        " ".repeat(4)
    };

    format!("{hours}{minutes}{seconds}")
}

fn fmt_progress(elapsed: Duration, total: Duration) -> String {
    let pct = elapsed.as_secs_f32() / total.as_secs_f32() * 100.0;

    format!("{pct:>5.1}%")
}
