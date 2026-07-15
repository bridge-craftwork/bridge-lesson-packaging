//! deal-survey — lesson-collection characterization tool.
//!
//! See deal-survey-spec.md. Read-only over collections; output deterministic and
//! diffable. `scan` runs stages 1–4 (structural, baseline DD, cash-out, probes);
//! `profile` rolls records up (stage 5); `report` prints a summary.

mod hash;
mod model;
mod profile;
mod report;
mod scan;
mod topics;

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
    /// Stage 5: roll per-deal records up into a collection profile.
    Profile {
        /// Directory of per-deal record JSON files (from `scan`).
        records: PathBuf,
        /// Output directory for the collection profile.
        #[arg(long)]
        out: PathBuf,
        /// Optional editorial metadata TOML sidecar to fold in.
        #[arg(long)]
        editorial: Option<PathBuf>,
        /// Optional topic-baseline TOML for a per-topic difficulty breakdown.
        #[arg(long)]
        topics: Option<PathBuf>,
    },
    /// Human-readable summary table over a profile file or directory.
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
            let makeable = s.difficulty.iter().sum::<usize>() + s.unclassified;
            eprintln!(
                "  {} written, {} cached | {} baselined, {} with contract",
                s.written, s.cached, s.baselined, s.with_contract,
            );
            eprintln!(
                "  cardplay difficulty (of {makeable} makeable): 0 cash-out={}  1 establish={}  2 technique={}  unclassified={}",
                s.difficulty[0], s.difficulty[1], s.difficulty[2], s.unclassified,
            );
            Ok(())
        }
        Command::Profile { records, out, editorial, topics } => {
            let p = profile::build_from_dir(&records, editorial.as_deref(), topics.as_deref())?;
            let path = profile::write(&out, &p)?;
            eprintln!(
                "deal-survey: profiled {} deal(s) of '{}' → {}",
                p.deal_count,
                p.collection,
                path.display()
            );
            Ok(())
        }
        Command::Report { profiles } => report::report(&profiles),
    }
}
