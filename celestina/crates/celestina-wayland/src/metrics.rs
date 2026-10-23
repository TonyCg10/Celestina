//! Frame timing, late frames, rest frames and RSS, enabled by
//! `Config.metrics`: counted per surface; the summary is returned once at
//! exit in the `RunReport`. Ported from the spike's `src/metrics.rs`.
//!
//! `frames` counts every frame a surface drew. Timing (`n`, the `cpu_*`
//! percentiles, `late`, `comp_late`) covers the animated frames only.
//! `rest_frames` counts frames drawn while nothing animated, from
//! [`REST_AFTER`] after the surface's first frame: a surface at rest draws
//! none (design §5.2), so anything there is a frame nobody needed — or the
//! clock's minute.
//!
//! Two clocks per frame callback: the local arrival `Instant` and the
//! compositor's `wl_callback.done` timestamp (ms, wrapping `u32`). An
//! arrival interval beyond 1.5 periods is `late`; a compositor interval
//! beyond it is `comp_late`. Late but not comp-late means the compositor
//! kept its cadence and the client dispatched late; comp-late means the
//! compositor itself skipped a refresh. The `cpu_*` percentiles are the
//! client's own submit time (shell frame + swap), not the frame interval.
#![forbid(unsafe_code)]

use std::{
    fmt::Write as _,
    time::{Duration, Instant},
};

use crate::ids::SurfaceId;

/// Frame-time samples kept per surface (percentiles cover the last ones).
pub const RING: usize = 4096;
/// A callback-to-callback interval beyond this many refresh periods is late.
pub const LATE_FACTOR: f64 = 1.5;
/// RSS is sampled this often while running (and once more at exit).
pub const RSS_EVERY: Duration = Duration::from_secs(10);
/// Frames before this long after a surface's first frame are its start-up
/// (configure, scale, first model), never rest frames.
pub const REST_AFTER: Duration = Duration::from_secs(1);

/// Refresh period for a mode refresh rate in mHz (165_058 -> 6.058 ms).
pub fn period_from_mhz(mhz: i32) -> Option<Duration> {
    (mhz > 0).then(|| Duration::from_secs_f64(1000.0 / f64::from(mhz)))
}

/// A frame is late when its callback came more than 1.5 periods after the
/// previous one.
pub fn is_late(interval: Duration, period: Duration) -> bool {
    interval.as_secs_f64() > period.as_secs_f64() * LATE_FACTOR
}

/// Interval between two `wl_callback.done` timestamps (ms, wrapping at
/// `u32::MAX`).
pub fn comp_interval(prev_ms: u32, now_ms: u32) -> Duration {
    Duration::from_millis(u64::from(now_ms.wrapping_sub(prev_ms)))
}

/// Nearest-rank percentile (`p` in 0..=100) of an ascending slice.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[derive(Debug)]
pub struct SurfaceStats {
    label: String,
    period: Option<Duration>,
    frames: u64,
    /// Frames drawn while nothing animated, after [`REST_AFTER`].
    rest_frames: u64,
    /// When the surface drew its first frame.
    first_frame: Option<Instant>,
    late: u64,
    comp_late: u64,
    max_ms: f64,
    ring: Vec<f64>,
    next: usize,
    last_callback: Option<Instant>,
    last_done: Option<u32>,
    /// Arrival and compositor intervals of the callback that triggers the
    /// next frame.
    pending_interval: Option<Duration>,
    pending_comp: Option<Duration>,
    /// The previous frame was animated: its callback interval is meaningful.
    chain: bool,
}

impl SurfaceStats {
    pub fn new(label: String, period: Option<Duration>) -> Self {
        Self {
            label,
            period,
            frames: 0,
            rest_frames: 0,
            first_frame: None,
            late: 0,
            comp_late: 0,
            max_ms: 0.0,
            ring: Vec::new(),
            next: 0,
            last_callback: None,
            last_done: None,
            pending_interval: None,
            pending_comp: None,
            chain: false,
        }
    }

    /// A `wl_surface.frame` callback arrived at `now` carrying the
    /// compositor's `done` timestamp `done_ms`.
    pub fn on_callback(&mut self, now: Instant, done_ms: u32) {
        self.pending_interval = self.last_callback.map(|last| now.duration_since(last));
        self.pending_comp = self.last_done.map(|last| comp_interval(last, done_ms));
        self.last_callback = Some(now);
        self.last_done = Some(done_ms);
    }

    /// A frame started at `started` was drawn and swapped in `elapsed` (CPU
    /// submit time); `animated` = it asked for another redraw. Every frame
    /// counts in `frames`; only animated frames are timed, and only those
    /// chained to an animated predecessor are classified. A frame that
    /// neither animates nor ends an animation is a rest frame once
    /// [`REST_AFTER`] has passed since the first frame.
    pub fn on_frame(&mut self, started: Instant, elapsed: Duration, animated: bool) {
        self.frames += 1;
        let first = *self.first_frame.get_or_insert(started);
        if !animated && !self.chain && started.saturating_duration_since(first) >= REST_AFTER {
            self.rest_frames += 1;
        }
        let interval = self.pending_interval.take();
        let comp = self.pending_comp.take();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        if animated {
            tracing::debug!(
                surface = %self.label,
                chained = self.chain,
                comp_interval_ms = comp.map(ms),
                arrival_interval_ms = interval.map(ms),
                cpu_ms = ms(elapsed),
                "metrics: animated frame"
            );
        } else {
            tracing::trace!(surface = %self.label, cpu_ms = ms(elapsed), "metrics: idle frame");
        }
        if animated {
            let ms = elapsed.as_secs_f64() * 1000.0;
            self.max_ms = self.max_ms.max(ms);
            if self.ring.len() < RING {
                self.ring.push(ms);
            } else {
                self.ring[self.next] = ms;
            }
            self.next = (self.next + 1) % RING;
            if let (true, Some(p)) = (self.chain, self.period) {
                if interval.is_some_and(|i| is_late(i, p)) {
                    self.late += 1;
                }
                if comp.is_some_and(|c| is_late(c, p)) {
                    self.comp_late += 1;
                }
            }
        }
        self.chain = animated;
    }

    /// `n` is the number of CPU samples behind the percentiles (below 100,
    /// p99 is the maximum); `late`/`comp_late` are `n/a` without a known
    /// refresh period.
    fn summary(&self) -> String {
        let mut sorted = self.ring.clone();
        sorted.sort_by(f64::total_cmp);
        let (late, comp_late) = match self.period {
            Some(_) => (self.late.to_string(), self.comp_late.to_string()),
            None => ("n/a".to_owned(), "n/a".to_owned()),
        };
        format!(
            "{} frames={} rest_frames={} n={} late={late} comp_late={comp_late} cpu_p50_ms={:.3} cpu_p99_ms={:.3} cpu_max_ms={:.3}",
            self.label,
            self.frames,
            self.rest_frames,
            sorted.len(),
            percentile(&sorted, 50.0),
            percentile(&sorted, 99.0),
            self.max_ms
        )
    }
}

#[derive(Debug, Default)]
pub struct Metrics {
    surfaces: Vec<(SurfaceId, SurfaceStats)>,
    /// `None` until `/proc/self/statm` has been read once.
    rss_peak: Option<u64>,
}

impl Metrics {
    pub fn register(&mut self, id: SurfaceId, label: String, period: Option<Duration>) {
        self.surfaces.push((id, SurfaceStats::new(label, period)));
    }

    pub fn period(&self, id: SurfaceId) -> Option<Duration> {
        self.surfaces
            .iter()
            .find(|(i, _)| *i == id)
            .and_then(|(_, s)| s.period)
    }

    pub fn on_callback(&mut self, id: SurfaceId, now: Instant, done_ms: u32) {
        if let Some(s) = self.stats(id) {
            s.on_callback(now, done_ms);
        }
    }

    pub fn on_frame(&mut self, id: SurfaceId, started: Instant, elapsed: Duration, animated: bool) {
        if let Some(s) = self.stats(id) {
            s.on_frame(started, elapsed, animated);
        }
    }

    /// Reads the current RSS and keeps the peak.
    pub fn sample_rss(&mut self) {
        if let Some(bytes) = rss_bytes() {
            self.rss_peak = Some(self.rss_peak.map_or(bytes, |peak| peak.max(bytes)));
        }
    }

    /// The exit summary: one line per surface, then the RSS peak in MiB
    /// (`n/a` when statm was never readable).
    pub fn summary(&mut self) -> String {
        self.sample_rss();
        let mut out = String::new();
        for (_, s) in &self.surfaces {
            let _ = writeln!(out, "{}", s.summary());
        }
        out.push_str(&rss_field(self.rss_peak));
        out
    }

    fn stats(&mut self, id: SurfaceId) -> Option<&mut SurfaceStats> {
        self.surfaces
            .iter_mut()
            .find(|(i, _)| *i == id)
            .map(|(_, s)| s)
    }
}

/// `rss_mib=<peak in MiB>`, or `rss_mib=n/a` when statm was unreadable.
fn rss_field(peak: Option<u64>) -> String {
    match peak {
        Some(bytes) => format!("rss_mib={:.1}", bytes as f64 / (1024.0 * 1024.0)),
        None => "rss_mib=n/a".to_owned(),
    }
}

/// Resident set size from `/proc/self/statm` (resident pages x page size).
pub fn rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * page_size())
}

/// Kernel page size from `smaps` (no libc dependency); 4 KiB if unreadable.
fn page_size() -> u64 {
    static SIZE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *SIZE.get_or_init(|| {
        std::fs::read_to_string("/proc/self/smaps")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("KernelPageSize:"))
                    .and_then(|v| v.trim().strip_suffix("kB")?.trim().parse::<u64>().ok())
            })
            .map_or(4096, |kb| kb * 1024)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: f64) -> Duration {
        Duration::from_secs_f64(v / 1000.0)
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let sorted: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&sorted, 50.0), 50.0);
        assert_eq!(percentile(&sorted, 99.0), 99.0);
        assert_eq!(percentile(&sorted, 100.0), 100.0);
        assert_eq!(percentile(&[7.0], 99.0), 7.0);
        assert_eq!(percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn late_is_beyond_one_and_a_half_periods() {
        let period = period_from_mhz(165_000).unwrap(); // ~6.06 ms
        assert!(!is_late(ms(8.0), period));
        assert!(is_late(ms(10.0), period));
    }

    #[test]
    fn period_from_mhz_rejects_unknown_rates() {
        assert_eq!(period_from_mhz(0), None);
        assert!((period_from_mhz(165_058).unwrap().as_secs_f64() * 1000.0 - 6.058).abs() < 0.01);
    }

    #[test]
    fn every_frame_counts_but_only_animated_ones_are_timed_and_late_needs_a_chain() {
        let period = period_from_mhz(165_000);
        let mut s = SurfaceStats::new("DP-1".into(), period);
        let t0 = Instant::now();
        // Idle frame: counted, not timed. Its long gap must not be classified.
        s.on_callback(t0, 1000);
        s.on_frame(t0, ms(1.0), false);
        // First animated frame after idle: timed, no chain, never late.
        s.on_callback(t0 + ms(500.0), 1500);
        s.on_frame(t0 + ms(500.0), ms(2.0), true);
        // 6 ms later on both clocks: on time.
        s.on_callback(t0 + ms(506.0), 1506);
        s.on_frame(t0 + ms(506.0), ms(3.0), true);
        // 10 ms later on both clocks: late and comp-late.
        s.on_callback(t0 + ms(516.0), 1516);
        s.on_frame(t0 + ms(516.0), ms(1.0), true);
        assert_eq!((s.frames, s.ring.len(), s.late, s.comp_late), (4, 3, 1, 1));
        assert_eq!(s.max_ms, 3.0);
        assert_eq!(s.rest_frames, 0, "all of it within the start-up second");
    }

    #[test]
    fn rest_frames_are_idle_frames_after_the_first_second() {
        let mut s = SurfaceStats::new("winit".into(), period_from_mhz(60_000));
        let t0 = Instant::now();
        // Start-up: configure, scale, the first model.
        s.on_frame(t0, ms(1.0), false);
        s.on_frame(t0 + ms(400.0), ms(1.0), false);
        // An animation and the frame that ends it, after the first second:
        // not rest.
        s.on_frame(t0 + ms(1500.0), ms(1.0), true);
        s.on_frame(t0 + ms(1517.0), ms(1.0), false);
        assert_eq!(s.rest_frames, 0);
        // A frame nothing animated asked for: rest.
        s.on_frame(t0 + ms(2500.0), ms(1.0), false);
        assert_eq!((s.frames, s.rest_frames), (5, 1));
    }

    #[test]
    fn compositor_interval_wraps_at_u32_max() {
        assert_eq!(comp_interval(100, 106), ms(6.0));
        assert_eq!(comp_interval(u32::MAX - 2, 4), ms(7.0));
        assert_eq!(comp_interval(u32::MAX, 0), ms(1.0));
    }

    #[test]
    fn comp_late_and_late_classify_independently_across_the_wrap() {
        let period = period_from_mhz(165_000); // ~6.06 ms, late beyond ~9.09
        let mut s = SurfaceStats::new("DP-1".into(), period);
        let t0 = Instant::now();
        let start = u32::MAX - 5;
        s.on_callback(t0, start);
        s.on_frame(t0, ms(1.0), true);
        // Compositor on cadence across the wrap (7 ms), client arrival late
        // (12 ms): late only.
        s.on_callback(t0 + ms(12.0), start.wrapping_add(7));
        s.on_frame(t0 + ms(12.0), ms(1.0), true);
        assert_eq!((s.late, s.comp_late), (1, 0));
        // Compositor skipped a refresh (13 ms, timestamps wrapped), arrival
        // on time-ish (8 ms): comp-late only.
        s.on_callback(t0 + ms(20.0), start.wrapping_add(20));
        s.on_frame(t0 + ms(20.0), ms(1.0), true);
        assert_eq!((s.late, s.comp_late), (1, 1));
        // Both on time.
        s.on_callback(t0 + ms(26.0), start.wrapping_add(26));
        s.on_frame(t0 + ms(26.0), ms(1.0), true);
        assert_eq!((s.frames, s.late, s.comp_late), (4, 1, 1));
    }

    #[test]
    fn ring_keeps_the_last_samples() {
        let mut s = SurfaceStats::new("DP-1".into(), None);
        let t0 = Instant::now();
        for i in 0..RING + 10 {
            s.on_frame(t0, ms(i as f64), true);
        }
        assert_eq!(s.ring.len(), RING);
        assert_eq!(s.frames as usize, RING + 10);
        assert!(!s.ring.contains(&0.0));
    }

    #[test]
    fn summary_lists_surfaces_then_rss() {
        let mut m = Metrics::default();
        m.register(SurfaceId(1), "DP-1".into(), period_from_mhz(165_000));
        m.register(SurfaceId(2), "DP-2".into(), None);
        m.on_frame(SurfaceId(1), Instant::now(), ms(2.0), true);
        let text = m.summary();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "DP-1 frames=1 rest_frames=0 n=1 late=0 comp_late=0 cpu_p50_ms=2.000 cpu_p99_ms=2.000 cpu_max_ms=2.000"
        );
        assert!(lines[1].starts_with("DP-2 frames=0 rest_frames=0 n=0 late=n/a comp_late=n/a "));
        assert!(lines[2].starts_with("rss_mib="));
    }

    #[test]
    fn rss_field_is_mib_or_n_a() {
        assert_eq!(rss_field(Some(3 * 1024 * 1024 / 2)), "rss_mib=1.5");
        assert_eq!(rss_field(None), "rss_mib=n/a");
    }
}
