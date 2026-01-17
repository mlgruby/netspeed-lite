//! # Job Scheduler
//!
//! This module manages the scheduling of speed tests.
//! It supports three modes:
//! 1. `HourlyAligned`: Runs at the start of every hour (e.g., 1:00, 2:00).
//! 2. `Interval`: Runs at a fixed interval (e.g., every 30 minutes) from startup.
//! 3. `Cron`: Runs according to a standard Cron expression.
//!
//! It provides `calculate_next_run` to determine the next execution time based on the selected mode.
use crate::config::{Config, ScheduleMode};
use crate::metrics::Metrics;
use crate::notifier::Notifier;
use crate::runner::{run_speedtest, RunOutcome};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration as TokioDuration};

pub struct Scheduler {
    config: Config,
    metrics: Metrics,
    notifier: Option<Notifier>,
    run_in_progress: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new(config: Config, metrics: Metrics, notifier: Option<Notifier>) -> Self {
        Self {
            config,
            metrics,
            notifier,
            run_in_progress: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&self) {
        tracing::info!("Starting scheduler in {:?} mode", self.config.schedule.mode);

        loop {
            let next_run = self.calculate_next_run();
            let now = Utc::now();

            if next_run > now {
                let sleep_duration = (next_run - now)
                    .to_std()
                    .unwrap_or(TokioDuration::from_secs(1));
                tracing::info!(
                    "Next run scheduled at {} (sleeping for {:?})",
                    next_run,
                    sleep_duration
                );
                sleep(sleep_duration).await;
            }

            // Check for overlap
            if self.run_in_progress.load(Ordering::SeqCst) && !self.config.schedule.allow_overlap {
                tracing::warn!("Previous run still in progress, skipping this run");
                self.metrics
                    .runs_total
                    .with_label_values(&["skipped"])
                    .inc();

                // Optionally notify about skipped run
                if let Some(_notifier) = &self.notifier {
                    if self.config.notify_on.failure {
                        // We could add a special notification for skipped runs
                        tracing::debug!("Skipped run notification not implemented");
                    }
                }
                continue;
            }

            // Execute the run
            self.execute_run().await;
        }
    }

    fn calculate_next_run(&self) -> DateTime<Utc> {
        match self.config.schedule.mode {
            ScheduleMode::HourlyAligned => self.calculate_next_aligned_run(),
            ScheduleMode::Interval => self.calculate_next_interval_run(),
            ScheduleMode::Cron => self.calculate_next_cron_run(),
        }
    }

    fn calculate_next_cron_run(&self) -> DateTime<Utc> {
        let expression = self
            .config
            .schedule
            .cron_expression
            .as_ref()
            .expect("Cron expression required for Cron mode");

        let schedule = Schedule::from_str(expression).expect("Invalid cron expression");
        let tz: Tz = self
            .config
            .schedule
            .timezone
            .parse()
            .expect("Invalid timezone");

        schedule
            .upcoming(tz)
            .next()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() + Duration::minutes(1))
    }

    fn calculate_next_aligned_run(&self) -> DateTime<Utc> {
        let tz: Tz = self
            .config
            .schedule
            .timezone
            .parse()
            .expect("Invalid timezone");
        let now_tz = Utc::now().with_timezone(&tz);

        // Calculate next top of hour
        let next_hour = if now_tz.minute() == 0 && now_tz.second() == 0 && now_tz.nanosecond() == 0
        {
            // If we're exactly at the top of the hour, schedule for next hour
            now_tz + Duration::hours(1)
        } else {
            // Otherwise, go to the next top of hour
            tz.with_ymd_and_hms(
                now_tz.year(),
                now_tz.month(),
                now_tz.day(),
                now_tz.hour() + 1,
                0,
                0,
            )
            .single()
            .unwrap_or_else(|| now_tz + Duration::hours(1))
        };

        next_hour.with_timezone(&Utc)
    }

    fn calculate_next_interval_run(&self) -> DateTime<Utc> {
        Utc::now() + Duration::seconds(self.config.schedule.interval_seconds as i64)
    }

    async fn execute_run(&self) {
        self.run_in_progress.store(true, Ordering::SeqCst);

        let run_id = Utc::now().timestamp();
        tracing::info!(run_id = run_id, "Starting speed test run");

        let result = run_speedtest(
            &self.config.speedtest.command,
            &self.config.speedtest.args,
            self.config.speedtest.timeout_seconds,
        )
        .await;

        let duration = result.duration;
        let outcome = result.outcome;

        // Update metrics
        let timestamp = Utc::now().timestamp() as f64;
        self.metrics.run_timestamp_seconds.set(timestamp);
        self.metrics
            .run_duration_seconds
            .set(duration.as_secs_f64());

        match &outcome {
            RunOutcome::Success(speedtest_result) => {
                tracing::info!(
                    run_id = run_id,
                    duration_secs = duration.as_secs(),
                    download_mbps = speedtest_result.download_bps / 1_000_000.0,
                    upload_mbps = speedtest_result.upload_bps / 1_000_000.0,
                    latency_ms = speedtest_result.latency_seconds * 1000.0,
                    "Speed test completed successfully"
                );

                self.metrics.last_success.set(1.0);
                self.metrics
                    .runs_total
                    .with_label_values(&["success"])
                    .inc();

                // Update measurement metrics
                self.metrics.download_bps.set(speedtest_result.download_bps);
                self.metrics.upload_bps.set(speedtest_result.upload_bps);
                self.metrics
                    .latency_seconds
                    .set(speedtest_result.latency_seconds);

                if let Some(jitter) = speedtest_result.jitter_seconds {
                    self.metrics.jitter_seconds.set(jitter);
                }

                if let Some(loss) = speedtest_result.packet_loss_ratio {
                    self.metrics.packet_loss_ratio.set(loss);
                }

                // Send notification if configured
                if let Some(notifier) = &self.notifier {
                    if self.config.notify_on.success {
                        notifier.notify(&outcome, duration).await;
                    }
                }
            }
            RunOutcome::Failure(error) => {
                tracing::error!(
                    run_id = run_id,
                    duration_secs = duration.as_secs(),
                    error = %error,
                    "Speed test failed"
                );

                self.metrics.last_success.set(0.0);
                self.metrics
                    .runs_total
                    .with_label_values(&["failure"])
                    .inc();

                // Send notification if configured
                if let Some(notifier) = &self.notifier {
                    if self.config.notify_on.failure {
                        notifier.notify(&outcome, duration).await;
                    }
                }
            }
        }

        self.run_in_progress.store(false, Ordering::SeqCst);
    }
}
