use anyhow::Result;
use clap::{Parser, Subcommand};
use gp_core::{Config, FeedbackLogger, Generator, OutputMetadata};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gp_inbetween")]
#[command(author, version, about = "AI-assisted inbetweening for Grease Pencil")]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate inbetween frames
    Generate {
        /// First keyframe (PNG)
        #[arg(long)]
        frame_a: PathBuf,

        /// Second keyframe (PNG)
        #[arg(long)]
        frame_b: PathBuf,

        /// Number of frames to generate
        #[arg(long, default_value = "4")]
        num_frames: u32,

        /// Output directory for generated frames
        #[arg(long)]
        output_dir: PathBuf,

        /// Config file path (optional)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Character name (for logging/tracking)
        #[arg(long)]
        character: Option<String>,

        /// Motion type (for logging/tracking, auto-detected if not specified)
        #[arg(long)]
        motion_type: Option<String>,
    },

    /// Accept a generated frame (log feedback)
    Accept {
        /// Frame number
        #[arg(long)]
        frame_number: u32,

        /// Character name
        #[arg(long)]
        character: String,

        /// Motion type
        #[arg(long)]
        motion_type: String,

        /// Was it auto-accepted?
        #[arg(long, default_value = "false")]
        auto: bool,

        /// Confidence score (optional)
        #[arg(long)]
        confidence: Option<f32>,
    },

    /// Reject a generated frame (log feedback)
    Reject {
        /// Frame number
        #[arg(long)]
        frame_number: u32,

        /// Character name
        #[arg(long)]
        character: String,

        /// Motion type
        #[arg(long)]
        motion_type: String,

        /// Issue categories (comma-separated)
        #[arg(long)]
        issues: Option<String>,

        /// Confidence score (optional)
        #[arg(long)]
        confidence: Option<f32>,
    },

    /// Show statistics from feedback log
    Stats {
        /// Filter by character
        #[arg(long)]
        character: Option<String>,

        /// Filter by motion type
        #[arg(long)]
        motion_type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a default configuration file
    InitConfig {
        /// Output path for config file
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match cli.command {
        Commands::Generate {
            frame_a,
            frame_b,
            num_frames,
            output_dir,
            config,
            character,
            motion_type,
        } => {
            run_generate(
                frame_a,
                frame_b,
                num_frames,
                output_dir,
                config,
                character,
                motion_type,
            )?;
        }

        Commands::Accept {
            frame_number,
            character,
            motion_type,
            auto,
            confidence,
        } => {
            let logger = FeedbackLogger::new()?;
            logger.log_acceptance(frame_number, &character, &motion_type, auto, confidence)?;
            println!("Logged acceptance for frame {frame_number}");
        }

        Commands::Reject {
            frame_number,
            character,
            motion_type,
            issues,
            confidence,
        } => {
            let logger = FeedbackLogger::new()?;
            let issue_list: Vec<String> = issues
                .map(|s| s.split(',').map(|i| i.trim().to_string()).collect())
                .unwrap_or_default();

            logger.log_rejection(frame_number, &character, &motion_type, &issue_list, confidence)?;
            println!("Logged rejection for frame {frame_number}");
        }

        Commands::Stats {
            character,
            motion_type,
            json,
        } => {
            let logger = FeedbackLogger::new()?;
            let stats = logger.get_stats(character.as_deref(), motion_type.as_deref())?;

            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("=== GP AI Inbetween Statistics ===");
                println!();
                println!("Total generations: {}", stats.total_generations);
                println!(
                    "Accepted: {} ({:.1}%)",
                    stats.accepted,
                    stats.acceptance_rate * 100.0
                );
                println!("  Auto-accepted: {}", stats.auto_accepted);
                println!("Rejected: {}", stats.rejected);
                println!();

                if !stats.by_motion_type.is_empty() {
                    println!("By motion type:");
                    for (mt, rate) in &stats.by_motion_type {
                        println!("  {}: {:.1}%", mt, rate * 100.0);
                    }
                    println!();
                }

                if !stats.by_character.is_empty() {
                    println!("By character:");
                    for (ch, rate) in &stats.by_character {
                        println!("  {}: {:.1}%", ch, rate * 100.0);
                    }
                    println!();
                }

                if !stats.common_issues.is_empty() {
                    println!("Common issues:");
                    for (issue, count) in stats.common_issues.iter().take(5) {
                        println!("  {}: {} occurrences", issue, count);
                    }
                }
            }
        }

        Commands::InitConfig { output } => {
            let config = Config::default();
            let output_path = output.unwrap_or_else(|| PathBuf::from("gp_ai_config.toml"));

            config.save(&output_path)?;
            println!("Created config file: {}", output_path.display());
            println!();
            println!("Edit this file to configure:");
            println!("  - API backend (replicate, local, serverless)");
            println!("  - API key for Replicate");
            println!("  - Preprocessing settings");
            println!("  - Auto-accept threshold");
        }
    }

    Ok(())
}

fn run_generate(
    frame_a: PathBuf,
    frame_b: PathBuf,
    num_frames: u32,
    output_dir: PathBuf,
    config_path: Option<PathBuf>,
    character: Option<String>,
    motion_type: Option<String>,
) -> Result<()> {
    // Validate inputs
    if !frame_a.exists() {
        anyhow::bail!("Frame A does not exist: {}", frame_a.display());
    }
    if !frame_b.exists() {
        anyhow::bail!("Frame B does not exist: {}", frame_b.display());
    }

    // Load config
    let config = if let Some(path) = config_path {
        log::info!("Loading config from {}", path.display());
        Config::load(&path)?
    } else {
        log::info!("Using default config");
        Config::load_or_default()
    };

    // Create generator
    let generator = Generator::new(config)?;

    // Generate frames
    log::info!("Generating {} inbetween frames...", num_frames);
    let results = generator.generate_inbetweens(
        &frame_a,
        &frame_b,
        num_frames,
        character.as_deref(),
        motion_type.as_deref(),
    )?;

    // Create output directory
    std::fs::create_dir_all(&output_dir)?;

    // Save outputs
    for (i, scored_frame) in results.frames.iter().enumerate() {
        let output_path = output_dir.join(format!("{:04}.png", i));
        scored_frame.frame.save(&output_path)?;

        let status = if scored_frame.auto_accept {
            "auto-accept"
        } else {
            "review"
        };
        log::info!(
            "Saved frame {} (confidence: {:.2}, {})",
            i,
            scored_frame.score,
            status
        );
    }

    // Write metadata
    let metadata: OutputMetadata = (&results).into();
    let metadata_path = output_dir.join("metadata.json");
    std::fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;

    println!("Generated {} frames in {}", results.frames.len(), output_dir.display());

    // Summary
    let auto_accepted: Vec<_> = results.frames.iter().filter(|f| f.auto_accept).collect();
    if !auto_accepted.is_empty() {
        println!(
            "  {} frame(s) auto-accepted (confidence >= {:.0}%)",
            auto_accepted.len(),
            results.metadata.auto_accept_threshold * 100.0
        );
    }

    let needs_review: Vec<_> = results.frames.iter().filter(|f| !f.auto_accept).collect();
    if !needs_review.is_empty() {
        println!("  {} frame(s) need manual review", needs_review.len());
    }

    Ok(())
}
