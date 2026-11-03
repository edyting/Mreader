//! Monotonic pacing timing helpers.

use std::time::{Duration, Instant};

/// Milliseconds per word for a given WPM.
pub fn ms_per_word(wpm: u32) -> f64 {
    let wpm = wpm.max(1) as f64;
    60_000.0 / wpm
}

/// Interval for a phrase containing `word_count` words.
pub fn ms_per_phrase(wpm: u32, word_count: usize) -> f64 {
    ms_per_word(wpm) * word_count.max(1) as f64
}

/// Monotonic schedule that avoids drift from repeated timer callbacks.
#[derive(Debug, Clone)]
pub struct PaceClock {
    started_at: Instant,
    paused_elapsed: Duration,
    running: bool,
    /// Absolute word-index timeline: next advance when elapsed >= deadline.
    next_deadline: Duration,
}

impl Default for PaceClock {
    fn default() -> Self {
        Self::new()
    }
}

impl PaceClock {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            paused_elapsed: Duration::ZERO,
            running: false,
            next_deadline: Duration::ZERO,
        }
    }

    pub fn start(&mut self) {
        self.started_at = Instant::now();
        self.paused_elapsed = Duration::ZERO;
        self.running = true;
        self.next_deadline = Duration::ZERO;
    }

    pub fn pause(&mut self) {
        if self.running {
            self.paused_elapsed = self.elapsed();
            self.running = false;
        }
    }

    pub fn resume(&mut self) {
        if !self.running {
            self.started_at = Instant::now();
            self.running = true;
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.paused_elapsed = Duration::ZERO;
        self.next_deadline = Duration::ZERO;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn elapsed(&self) -> Duration {
        if self.running {
            self.paused_elapsed + self.started_at.elapsed()
        } else {
            self.paused_elapsed
        }
    }

    /// Schedule the next advance after `interval_ms` from the current deadline
    /// (or now if deadline is behind).
    pub fn schedule_next(&mut self, interval_ms: f64) {
        let interval = Duration::from_secs_f64((interval_ms / 1000.0).max(0.001));
        let now = self.elapsed();
        if self.next_deadline < now {
            self.next_deadline = now;
        }
        self.next_deadline += interval;
    }

    /// Returns how many scheduled steps are due based on monotonic elapsed time.
    pub fn due_steps(&mut self, interval_ms: f64) -> usize {
        if !self.running {
            return 0;
        }
        let interval = Duration::from_secs_f64((interval_ms / 1000.0).max(0.001));
        let now = self.elapsed();
        let mut count = 0usize;
        while self.next_deadline + interval <= now {
            self.next_deadline += interval;
            count += 1;
            // Safety valve for catch-up
            if count > 64 {
                self.next_deadline = now;
                break;
            }
        }
        // Also fire once when first crossing deadline after start
        if count == 0 && now >= self.next_deadline && self.next_deadline == Duration::ZERO {
            // bootstrap: set first deadline after one interval from start
            self.next_deadline = interval;
            if now >= interval {
                count = 1;
                while self.next_deadline + interval <= now {
                    self.next_deadline += interval;
                    count += 1;
                    if count > 64 {
                        self.next_deadline = now;
                        break;
                    }
                }
            }
        } else if count == 0 && now >= self.next_deadline && self.next_deadline > Duration::ZERO {
            // overdue single step
            count = 1;
            self.next_deadline += interval;
            while self.next_deadline + interval <= now {
                self.next_deadline += interval;
                count += 1;
                if count > 64 {
                    self.next_deadline = now;
                    break;
                }
            }
        }
        count
    }

    /// Simpler API: check if it is time to advance and consume one step.
    pub fn should_advance(&mut self, interval_ms: f64) -> bool {
        self.due_steps(interval_ms) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn ms_per_word_at_200_wpm() {
        assert!((ms_per_word(200) - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ms_per_phrase_scales() {
        assert!((ms_per_phrase(200, 3) - 900.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clock_pause_resume() {
        let mut clock = PaceClock::new();
        clock.start();
        thread::sleep(Duration::from_millis(50));
        clock.pause();
        let paused = clock.elapsed();
        thread::sleep(Duration::from_millis(50));
        assert!((clock.elapsed() - paused).as_millis() < 10);
        clock.resume();
        thread::sleep(Duration::from_millis(30));
        assert!(clock.elapsed() > paused);
    }
}
