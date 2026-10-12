//! Writing goals and session stats: a daily word target, words written today and this session,
//! time spent writing, a streak and the last seven days.
//!
//! "Words written" is the net change in the document's word count while you work (deleting words
//! takes them back off), counted only while the same document stays open, so opening a long file
//! doesn't count as writing it. "Time writing" counts the minutes in which the document changed,
//! allowing a short pause (a minute) between edits.

use serde::{Deserialize, Serialize};

/// A pause longer than this ends a stretch of writing time.
const IDLE_SECS: f64 = 60.0;
/// Days of history kept.
const KEEP_DAYS: usize = 30;

/// One day's totals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Day {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Net words written that day (never shown below zero).
    pub words: i64,
    /// Seconds spent writing.
    pub secs: f64,
}

/// What is saved between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Goals {
    /// Words per day to aim for.
    pub daily: u32,
    /// Recent days, oldest first.
    pub days: Vec<Day>,
}

impl Default for Goals {
    fn default() -> Self {
        Goals { daily: 1000, days: Vec::new() }
    }
}

/// What is tracked while the app runs.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// The document and its word count last seen.
    last: Option<(u64, usize)>,
    /// The document revision last seen, and when it last changed (seconds).
    last_rev: u64,
    last_edit: Option<f64>,
    last_tick: Option<f64>,
    /// Net words written since the app started.
    pub session_words: i64,
}

impl Goals {
    /// Today's entry, created (and old days trimmed) when the date changes.
    pub fn today_mut(&mut self, today: &str) -> &mut Day {
        if self.days.last().is_none_or(|d| d.date != today) {
            self.days.push(Day { date: today.to_string(), words: 0, secs: 0.0 });
            if self.days.len() > KEEP_DAYS {
                let extra = self.days.len() - KEEP_DAYS;
                self.days.drain(..extra);
            }
        }
        // Just pushed or matched above, so the list isn't empty; fall back to a fresh day anyway.
        if self.days.is_empty() {
            self.days.push(Day { date: today.to_string(), words: 0, secs: 0.0 });
        }
        let last = self.days.len() - 1;
        &mut self.days[last]
    }

    /// Today's totals (zero when nothing was written yet today).
    pub fn today(&self, today: &str) -> Day {
        self.days.last().filter(|d| d.date == today).cloned().unwrap_or(Day { date: today.to_string(), words: 0, secs: 0.0 })
    }

    /// Progress towards today's goal, 0.0–1.0.
    pub fn progress(&self, today: &str) -> f32 {
        let words = self.today(today).words.max(0) as f32;
        (words / self.daily.max(1) as f32).clamp(0.0, 1.0)
    }

    /// Days in a row, ending today (or yesterday, if nothing is written yet today), with words
    /// written.
    pub fn streak(&self, today: &str) -> u32 {
        let mut n = 0;
        let mut expect = today.to_string();
        let mut days = self.days.iter().rev().peekable();
        // Nothing yet today doesn't break yesterday's streak.
        if days.peek().is_some_and(|d| d.date == today && d.words <= 0) {
            days.next();
            expect = previous_day(today);
        } else if days.peek().is_some_and(|d| d.date != today) {
            expect = previous_day(today);
        }
        for d in days {
            if d.date != expect || d.words <= 0 {
                break;
            }
            n += 1;
            expect = previous_day(&expect);
        }
        n
    }

    /// The last seven days' words, oldest first, as (weekday letter, words).
    pub fn week(&self, today: &str) -> Vec<(char, i64)> {
        let mut out = Vec::new();
        let mut date = today.to_string();
        for _ in 0..7 {
            let words = self.days.iter().find(|d| d.date == date).map(|d| d.words.max(0)).unwrap_or(0);
            out.push((weekday_letter(&date), words));
            date = previous_day(&date);
        }
        out.reverse();
        out
    }
}

impl Tracker {
    /// Update from the open document. `doc` identifies the document, `words` is its word count,
    /// `rev` its revision and `now` the time in seconds.
    pub fn tick(&mut self, goals: &mut Goals, today: &str, doc: u64, words: usize, rev: u64, now: f64) {
        let day = goals.today_mut(today);
        if let Some((d, w)) = self.last
            && d == doc
        {
            let delta = words as i64 - w as i64;
            day.words += delta;
            self.session_words += delta;
        }
        self.last = Some((doc, words));
        if rev != self.last_rev {
            self.last_rev = rev;
            self.last_edit = Some(now);
        }
        if let (Some(edit), Some(tick)) = (self.last_edit, self.last_tick)
            && now - edit <= IDLE_SECS
        {
            day.secs += (now - tick).clamp(0.0, IDLE_SECS);
        }
        self.last_tick = Some(now);
    }
}

/// `YYYY-MM-DD` for a Unix time in seconds (UTC).
pub fn date_of(unix_secs: i64) -> String {
    let (y, m, d) = wordcraft_engine::cmd::civil_from_days(unix_secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_of(date: &str) -> Option<i64> {
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    // Days from civil (Howard Hinnant), the inverse of `civil_from_days`.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn previous_day(date: &str) -> String {
    days_of(date).map(|n| date_of((n - 1) * 86_400)).unwrap_or_default()
}

fn weekday_letter(date: &str) -> char {
    // 1970-01-01 was a Thursday.
    let letters = ['T', 'F', 'S', 'S', 'M', 'T', 'W'];
    days_of(date).and_then(|n| letters.get(n.rem_euclid(7) as usize).copied()).unwrap_or(' ')
}

/// Minutes, shown as "42 min" or "1 h 05 min".
pub fn minutes_text(secs: f64) -> String {
    let m = (secs / 60.0).floor() as i64;
    if m >= 60 { format!("{} h {:02} min", m / 60, m % 60) } else { format!("{m} min") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(previous_day("2026-03-01"), "2026-02-28");
        assert_eq!(previous_day("2024-03-01"), "2024-02-29");
        assert_eq!(weekday_letter("2026-10-12"), 'M');
    }

    #[test]
    fn typing_counts_and_opening_a_document_does_not() {
        let mut g = Goals::default();
        let mut t = Tracker::default();
        t.tick(&mut g, "2026-10-12", 1, 5000, 1, 0.0);
        assert_eq!(g.today("2026-10-12").words, 0, "opening a long document isn't writing it");
        t.tick(&mut g, "2026-10-12", 1, 5040, 2, 10.0);
        assert_eq!(g.today("2026-10-12").words, 40);
        t.tick(&mut g, "2026-10-12", 2, 100, 3, 20.0);
        assert_eq!(g.today("2026-10-12").words, 40, "switching documents adds nothing");
        assert_eq!(t.session_words, 40);
    }

    #[test]
    fn writing_time_stops_after_a_pause() {
        let mut g = Goals::default();
        let mut t = Tracker::default();
        t.tick(&mut g, "d", 1, 0, 1, 0.0);
        t.tick(&mut g, "d", 1, 1, 2, 30.0);
        t.tick(&mut g, "d", 1, 1, 2, 60.0);
        t.tick(&mut g, "d", 1, 1, 2, 600.0);
        assert!((g.today("d").secs - 60.0).abs() < 1e-6, "{}", g.today("d").secs);
    }

    #[test]
    fn streak_counts_consecutive_days() {
        let g = Goals {
            daily: 500,
            days: vec![
                Day { date: "2026-10-09".into(), words: 300, secs: 0.0 },
                Day { date: "2026-10-10".into(), words: 0, secs: 0.0 },
                Day { date: "2026-10-11".into(), words: 120, secs: 0.0 },
                Day { date: "2026-10-12".into(), words: 80, secs: 0.0 },
            ],
        };
        assert_eq!(g.streak("2026-10-12"), 2);
        assert_eq!(g.streak("2026-10-13"), 2, "today isn't over yet");
        assert_eq!(g.week("2026-10-12").len(), 7);
    }

    #[test]
    fn progress_is_clamped() {
        let mut g = Goals { daily: 0, days: Vec::new() };
        g.today_mut("d").words = -50;
        assert_eq!(g.progress("d"), 0.0);
        g.today_mut("d").words = 10_000;
        assert_eq!(g.progress("d"), 1.0);
    }
}
