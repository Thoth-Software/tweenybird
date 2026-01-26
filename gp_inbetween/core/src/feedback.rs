use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedbackEntry {
    pub timestamp: u64,
    pub event: FeedbackEvent,
    pub character: String,
    pub motion_type: String,
    pub frame_number: Option<u32>,
    pub auto_accepted: Option<bool>,
    pub issues: Option<Vec<String>>,
    pub confidence_score: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackEvent {
    Generation,
    Accept,
    Reject,
}

#[derive(Debug, Serialize, Clone)]
pub struct Statistics {
    pub total_generations: u32,
    pub accepted: u32,
    pub rejected: u32,
    pub acceptance_rate: f32,
    pub auto_accepted: u32,
    pub by_motion_type: Vec<(String, f32)>,
    pub by_character: Vec<(String, f32)>,
    pub common_issues: Vec<(String, u32)>,
}

pub struct FeedbackLogger {
    log_path: PathBuf,
}

impl FeedbackLogger {
    pub fn new() -> Result<Self> {
        let log_path = Self::default_log_path()?;

        // Ensure directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create feedback log directory")?;
        }

        Ok(Self { log_path })
    }

    pub fn with_path(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { log_path: path })
    }

    fn default_log_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not determine home directory")?;

        let log_dir = home.join(".blender").join("gp_ai_feedback");
        Ok(log_dir.join("feedback.jsonl"))
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn append_entry(&self, entry: &FeedbackEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open feedback log")?;

        let json = serde_json::to_string(entry)?;
        writeln!(file, "{json}")?;

        Ok(())
    }

    /// Log a generation event
    pub fn log_generation(
        &self,
        character: &str,
        motion_type: &str,
        num_frames: u32,
    ) -> Result<()> {
        log::info!(
            "Logging generation: character={}, motion={}, frames={}",
            character,
            motion_type,
            num_frames
        );

        let entry = FeedbackEntry {
            timestamp: Self::current_timestamp(),
            event: FeedbackEvent::Generation,
            character: character.to_string(),
            motion_type: motion_type.to_string(),
            frame_number: Some(num_frames),
            auto_accepted: None,
            issues: None,
            confidence_score: None,
        };

        self.append_entry(&entry)
    }

    /// Log frame acceptance
    pub fn log_acceptance(
        &self,
        frame_number: u32,
        character: &str,
        motion_type: &str,
        auto_accepted: bool,
        confidence_score: Option<f32>,
    ) -> Result<()> {
        log::info!(
            "Logging acceptance: frame={}, character={}, motion={}, auto={}",
            frame_number,
            character,
            motion_type,
            auto_accepted
        );

        let entry = FeedbackEntry {
            timestamp: Self::current_timestamp(),
            event: FeedbackEvent::Accept,
            character: character.to_string(),
            motion_type: motion_type.to_string(),
            frame_number: Some(frame_number),
            auto_accepted: Some(auto_accepted),
            issues: None,
            confidence_score,
        };

        self.append_entry(&entry)
    }

    /// Log frame rejection
    pub fn log_rejection(
        &self,
        frame_number: u32,
        character: &str,
        motion_type: &str,
        issues: &[String],
        confidence_score: Option<f32>,
    ) -> Result<()> {
        log::info!(
            "Logging rejection: frame={}, character={}, motion={}, issues={:?}",
            frame_number,
            character,
            motion_type,
            issues
        );

        let entry = FeedbackEntry {
            timestamp: Self::current_timestamp(),
            event: FeedbackEvent::Reject,
            character: character.to_string(),
            motion_type: motion_type.to_string(),
            frame_number: Some(frame_number),
            auto_accepted: None,
            issues: Some(issues.to_vec()),
            confidence_score,
        };

        self.append_entry(&entry)
    }

    /// Read all entries from the log
    fn read_entries(&self) -> Result<Vec<FeedbackEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = std::fs::File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<FeedbackEntry>(&line) {
                entries.push(entry);
            } else {
                log::warn!("Failed to parse feedback entry: {}", line);
            }
        }

        Ok(entries)
    }

    /// Get acceptance rate filtered by character and/or motion type
    pub fn get_acceptance_rate(
        &self,
        character: Option<&str>,
        motion_type: Option<&str>,
    ) -> Result<f32> {
        let entries = self.read_entries()?;

        let mut accepts = 0u32;
        let mut rejects = 0u32;

        for entry in entries {
            // Filter by character if specified
            if let Some(ch) = character {
                if entry.character != ch {
                    continue;
                }
            }

            // Filter by motion type if specified
            if let Some(mt) = motion_type {
                if entry.motion_type != mt {
                    continue;
                }
            }

            match entry.event {
                FeedbackEvent::Accept => accepts += 1,
                FeedbackEvent::Reject => rejects += 1,
                FeedbackEvent::Generation => {}
            }
        }

        let total = accepts + rejects;
        if total == 0 {
            return Ok(0.5); // No data, assume 50%
        }

        Ok(accepts as f32 / total as f32)
    }

    /// Get comprehensive statistics
    pub fn get_stats(
        &self,
        character: Option<&str>,
        motion_type: Option<&str>,
    ) -> Result<Statistics> {
        let entries = self.read_entries()?;

        let mut total_generations = 0u32;
        let mut accepted = 0u32;
        let mut rejected = 0u32;
        let mut auto_accepted = 0u32;
        let mut by_motion_type: HashMap<String, (u32, u32)> = HashMap::new();
        let mut by_character: HashMap<String, (u32, u32)> = HashMap::new();
        let mut issue_counts: HashMap<String, u32> = HashMap::new();

        for entry in entries {
            // Filter by character if specified
            if let Some(ch) = character {
                if entry.character != ch {
                    continue;
                }
            }

            // Filter by motion type if specified
            if let Some(mt) = motion_type {
                if entry.motion_type != mt {
                    continue;
                }
            }

            match entry.event {
                FeedbackEvent::Generation => {
                    total_generations += 1;
                }
                FeedbackEvent::Accept => {
                    accepted += 1;

                    if entry.auto_accepted == Some(true) {
                        auto_accepted += 1;
                    }

                    by_motion_type
                        .entry(entry.motion_type.clone())
                        .or_insert((0, 0))
                        .0 += 1;

                    by_character
                        .entry(entry.character.clone())
                        .or_insert((0, 0))
                        .0 += 1;
                }
                FeedbackEvent::Reject => {
                    rejected += 1;

                    by_motion_type
                        .entry(entry.motion_type.clone())
                        .or_insert((0, 0))
                        .1 += 1;

                    by_character
                        .entry(entry.character.clone())
                        .or_insert((0, 0))
                        .1 += 1;

                    // Count issues
                    if let Some(issues) = &entry.issues {
                        for issue in issues {
                            *issue_counts.entry(issue.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let acceptance_rate = if accepted + rejected > 0 {
            accepted as f32 / (accepted + rejected) as f32
        } else {
            0.0
        };

        // Convert motion type stats to rates
        let by_motion_type: Vec<(String, f32)> = by_motion_type
            .into_iter()
            .map(|(mt, (acc, rej))| {
                let rate = if acc + rej > 0 {
                    acc as f32 / (acc + rej) as f32
                } else {
                    0.0
                };
                (mt, rate)
            })
            .collect();

        // Convert character stats to rates
        let by_character: Vec<(String, f32)> = by_character
            .into_iter()
            .map(|(ch, (acc, rej))| {
                let rate = if acc + rej > 0 {
                    acc as f32 / (acc + rej) as f32
                } else {
                    0.0
                };
                (ch, rate)
            })
            .collect();

        // Sort issues by count
        let mut common_issues: Vec<(String, u32)> = issue_counts.into_iter().collect();
        common_issues.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(Statistics {
            total_generations,
            accepted,
            rejected,
            acceptance_rate,
            auto_accepted,
            by_motion_type,
            by_character,
            common_issues,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_log_and_read() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test_feedback.jsonl");
        let logger = FeedbackLogger::with_path(log_path).unwrap();

        logger.log_generation("hero", "walk", 4).unwrap();
        logger
            .log_acceptance(1, "hero", "walk", false, Some(0.9))
            .unwrap();
        logger
            .log_rejection(2, "hero", "walk", &["artifacts".to_string()], Some(0.6))
            .unwrap();

        let stats = logger.get_stats(None, None).unwrap();
        assert_eq!(stats.total_generations, 1);
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.rejected, 1);
        assert!((stats.acceptance_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_filter_by_character() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test_feedback.jsonl");
        let logger = FeedbackLogger::with_path(log_path).unwrap();

        logger
            .log_acceptance(1, "hero", "walk", false, None)
            .unwrap();
        logger
            .log_acceptance(2, "hero", "walk", false, None)
            .unwrap();
        logger
            .log_rejection(3, "villain", "walk", &[], None)
            .unwrap();

        let hero_rate = logger.get_acceptance_rate(Some("hero"), None).unwrap();
        assert!((hero_rate - 1.0).abs() < 0.01);

        let villain_rate = logger.get_acceptance_rate(Some("villain"), None).unwrap();
        assert!((villain_rate - 0.0).abs() < 0.01);
    }
}
