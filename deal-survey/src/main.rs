//! deal-survey — lesson-collection characterization tool.
//!
//! See deal-survey-spec.md. Read-only over collections; output deterministic and
//! diffable. Slice 1 implements `scan` stage 1 (structural); `profile`/`report`
//! and the DD/probe stages land in later slices.

mod hash;
mod model;
mod scan;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "deal-survey", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Stages 1–4: walk a collection, write one JSON record per deal.
    Scan {
        /// Collection directory to walk (read-only).
        collection_dir: PathBuf,
        /// Output directory for per-deal records.
        #[arg(long)]
        out: PathBuf,
    },
    /// Stage 5: roll per-deal records up into a collection profile. (slice 4)
    Profile {
        records: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Human-readable summary table over profiles. (slice 4)
    Report { profiles: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan { collection_dir, out } => {
            let s = scan::scan_collection(&collection_dir, &out)?;
            eprintln!(
                "deal-survey: {} deal(s) from {} → {}",
                s.total,
                collection_dir.display(),
                out.display()
            );
            eprintln!(
                "  {} written, {} cached | {} baselined, {} with contract | difficulty 0 (cash-out): {} ({:.0}% of contracts)",
                s.written,
                s.cached,
                s.baselined,
                s.with_contract,
                s.difficulty0,
                if s.with_contract > 0 {
                    100.0 * s.difficulty0 as f64 / s.with_contract as f64
                } else {
                    0.0
                },
            );
            Ok(())
        }
        Command::Profile { .. } => {
            anyhow::bail!("`profile` is not implemented yet (roadmap slice 4: survey-profile)")
        }
        Command::Report { .. } => {
            anyhow::bail!("`report` is not implemented yet (roadmap slice 4: survey-profile)")
        }
    }
}
