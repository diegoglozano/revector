//! revector CLI entry point.
//!
//! This binary is a thin shell: it parses flags, resolves config, connects to
//! Qdrant, and hands off to the library. All real logic lives in `revector::*`
//! so it can be unit/integration tested without a process boundary.

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use tracing::error;
use tracing_subscriber::EnvFilter;

use revector::chain::Chain;
use revector::cli::{Cli, Command};
use revector::config::Config;
use revector::migration::discover;
use revector::runner::{Runner, StatusReport};
use revector::spec::CollectionSpec;
use revector::{client, diff, scaffold};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(verbose: u8) {
    let default = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_env("REVECTOR_LOG")
        .unwrap_or_else(|_| EnvFilter::new(format!("revector={default}")));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_target(false)
        .compact()
        .init();
}

/// Resolve config from file/env, then overlay explicit CLI flags.
fn resolve_config(cli: &Cli) -> revector::Result<Config> {
    let mut config = Config::load(cli.config.as_deref())?;
    if let Some(url) = &cli.url {
        config.url = url.clone();
    }
    if let Some(key) = &cli.api_key {
        config.api_key = Some(key.clone());
    }
    if let Some(dir) = &cli.migrations_dir {
        config.migrations_dir = dir.clone();
    }
    Ok(config)
}

async fn run(cli: Cli) -> revector::Result<()> {
    // Commands that don't need a live connection are handled first.
    match &cli.command {
        Command::Init => {
            let config = resolve_config(&cli)?;
            let config_path = cli
                .config
                .clone()
                .unwrap_or_else(|| Path::new("revector.toml").to_path_buf());
            let created = scaffold::init(&config.migrations_dir, &config_path)?;
            if created.is_empty() {
                println!("Already initialized — nothing to create.");
            } else {
                for p in created {
                    println!("created {}", p.display());
                }
            }
            return Ok(());
        }
        Command::New { name } => {
            let config = resolve_config(&cli)?;
            let path = scaffold::new_migration(&config.migrations_dir, name)?;
            println!("created {}", path.display());
            return Ok(());
        }
        _ => {}
    }

    // Remaining commands need config + a chain + a client.
    let config = resolve_config(&cli)?;
    let project_root = std::env::current_dir()?;
    let qdrant = client::connect(&config)?;

    if let Command::Diff { collection, spec } = &cli.command {
        let bytes = std::fs::read(spec).map_err(|source| revector::Error::ReadFile {
            path: spec.clone(),
            source,
        })?;
        let collection_spec: CollectionSpec =
            serde_yaml::from_slice(&bytes).map_err(|source| revector::Error::ParseFile {
                path: spec.clone(),
                source,
            })?;
        let report = diff::diff_collection(&qdrant, collection, &collection_spec).await?;
        print_diff(&report);
        return Ok(());
    }

    // Status / up / down / to all operate over the resolved chain.
    let chain = Chain::resolve(discover(&config.migrations_dir)?)?;

    match cli.command {
        Command::Status => {
            let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, &project_root);
            let report = runner.status().await?;
            print_status(&report);
        }
        Command::Up { to, dry_run } => {
            let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, &project_root)
                .dry_run(dry_run);
            runner.warn_orphans().await?;
            let applied = runner.up(to.as_deref()).await?;
            print_applied("Applied", &applied.revisions, applied.dry_run);
        }
        Command::Down { to, steps, dry_run } => {
            let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, &project_root)
                .dry_run(dry_run);
            let applied = runner.down(to.as_deref(), steps).await?;
            print_applied("Rolled back", &applied.revisions, applied.dry_run);
        }
        Command::To { revision, dry_run } => {
            let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, &project_root)
                .dry_run(dry_run);
            let applied = runner.to(&revision).await?;
            print_applied("Migrated", &applied.revisions, applied.dry_run);
        }
        // Handled above.
        Command::Init | Command::New { .. } | Command::Diff { .. } => unreachable!(),
    }

    Ok(())
}

fn print_status(report: &StatusReport) {
    println!("Migration status");
    println!("  head:    {}", report.head.as_deref().unwrap_or("(none)"));
    println!(
        "  current: {}",
        report.current.as_deref().unwrap_or("(none)")
    );
    println!();
    if report.revisions.is_empty() {
        println!("  no migrations found");
    }
    for r in &report.revisions {
        let mark = if r.applied { "✔" } else { "·" };
        let mut notes = Vec::new();
        if let Some(false) = r.checksum_ok {
            notes.push("CHECKSUM MISMATCH".to_string());
        }
        if !r.reversible {
            notes.push("irreversible".to_string());
        }
        if let Some(at) = &r.applied_at {
            notes.push(format!("applied {at}"));
        }
        let suffix = if notes.is_empty() {
            String::new()
        } else {
            format!("  [{}]", notes.join(", "))
        };
        println!(
            "  {mark} {}  {}{suffix}",
            r.revision,
            r.description.as_deref().unwrap_or("")
        );
    }
    if !report.orphaned.is_empty() {
        println!();
        println!("  orphaned (applied but missing on disk):");
        for o in &report.orphaned {
            println!("    ! {o}");
        }
    }
}

fn print_applied(verb: &str, revisions: &[String], dry_run: bool) {
    let prefix = if dry_run { "[dry-run] " } else { "" };
    if revisions.is_empty() {
        println!("{prefix}Nothing to do.");
        return;
    }
    println!("{prefix}{verb} {} revision(s):", revisions.len());
    for r in revisions {
        println!("  - {r}");
    }
}

fn print_diff(report: &diff::DiffReport) {
    if !report.exists {
        println!(
            "collection `{}` does not exist (declared but not created)",
            report.collection
        );
        return;
    }
    if report.differences.is_empty() {
        println!("collection `{}` is in sync ✔", report.collection);
        return;
    }
    println!(
        "collection `{}` has {} difference(s):",
        report.collection,
        report.differences.len()
    );
    for d in &report.differences {
        println!("  {} : declared {} | live {}", d.path, d.declared, d.live);
    }
}
