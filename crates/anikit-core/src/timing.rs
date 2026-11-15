//! Timing model (delay, duration, iterations, direction, end delay), closely
//! following the Web Animations specification so behaviour matches CSS/WAAPI.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Direction {
    Normal = 0,
    Reverse = 1,
    Alternate = 2,
    AlternateReverse = 3,
}

impl Direction {
    pub fn from_u8(v: u8) -> Direction {
        match v {
            1 => Direction::Reverse,
            2 => Direction::Alternate,
            3 => Direction::AlternateReverse,
            _ => Direction::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    Before = 0,
    Active = 1,
    After = 2,
}

/// Timing parameters. All times are in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub delay: f64,
    pub duration: f64,
    /// Number of iterations; may be fractional or `f64::INFINITY`.
    pub iterations: f64,
    pub direction: Direction,
    pub end_delay: f64,
    /// Easing applied to each iteration's progress (index into the engine's
    /// easing registry), `u32::MAX` for linear.
    pub easing: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            delay: 0.0,
            duration: 1000.0,
            iterations: 1.0,
            direction: Direction::Normal,
            end_delay: 0.0,
            easing: u32::MAX,
        }
    }
}

/// Result of evaluating timing at a local time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingState {
    pub phase: Phase,
    /// Current iteration index (0-based).
    pub iteration: f64,
    /// Directed (but not eased) progress within the iteration, `[0, 1]`.
    pub progress: f64,
}

impl Timing {
    #[inline]
    pub fn active_duration(&self) -> f64 {
        if self.duration <= 0.0 || self.iterations <= 0.0 {
            0.0
        } else {
            self.duration * self.iterations
        }
    }

    /// Delay + active duration + end delay.
    #[inline]
    pub fn end_time(&self) -> f64 {
        (self.delay + self.active_duration() + self.end_delay).max(0.0)
    }

    #[inline]
    fn directed(&self, iteration: f64, p: f64) -> f64 {
        let odd = (iteration as i64) & 1 == 1;
        let rev = match self.direction {
            Direction::Normal => false,
            Direction::Reverse => true,
            Direction::Alternate => odd,
            Direction::AlternateReverse => !odd,
        };
        if rev {
            1.0 - p
        } else {
            p
        }
    }

    /// Evaluate at local time `t` (ms, delay included).
    #[inline]
    pub fn eval(&self, t: f64) -> TimingState {
        let active = self.active_duration();
        if t < self.delay {
            return TimingState { phase: Phase::Before, iteration: 0.0, progress: self.directed(0.0, 0.0) };
        }
        let at = t - self.delay;
        if at < active {
            if self.iterations == 1.0 {
                // Common case: a single iteration (no floor/modulo needed).
                return TimingState { phase: Phase::Active, iteration: 0.0, progress: self.directed(0.0, at / self.duration) };
            }
            if self.duration == f64::INFINITY {
                return TimingState { phase: Phase::Active, iteration: 0.0, progress: self.directed(0.0, 0.0) };
            }
            let iter = crate::math::floor(at / self.duration);
            let p = (at - iter * self.duration) / self.duration;
            return TimingState { phase: Phase::Active, iteration: iter, progress: self.directed(iter, p) };
        }
        // After phase.
        if active == 0.0 {
            let iter = if self.iterations > 0.0 { crate::math::ceil(self.iterations) - 1.0 } else { 0.0 };
            return TimingState { phase: Phase::After, iteration: iter.max(0.0), progress: self.directed(iter, 1.0) };
        }
        let iters = self.iterations;
        let mut iter = crate::math::floor(iters);
        let mut p = iters - iter;
        if p == 0.0 {
            iter -= 1.0;
            p = 1.0;
        }
        TimingState { phase: Phase::After, iteration: iter.max(0.0), progress: self.directed(iter, p) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_phases() {
        let t = Timing { delay: 100.0, duration: 1000.0, ..Default::default() };
        assert_eq!(t.eval(50.0).phase, Phase::Before);
        let s = t.eval(600.0);
        assert_eq!(s.phase, Phase::Active);
        assert!((s.progress - 0.5).abs() < 1e-12);
        let e = t.eval(5000.0);
        assert_eq!(e.phase, Phase::After);
        assert_eq!(e.progress, 1.0);
        assert_eq!(t.end_time(), 1100.0);
    }

    #[test]
    fn alternate_and_fractional() {
        let t = Timing { duration: 100.0, iterations: 2.5, direction: Direction::Alternate, ..Default::default() };
        assert!((t.eval(25.0).progress - 0.25).abs() < 1e-12);
        let s = t.eval(125.0);
        assert_eq!(s.iteration, 1.0);
        assert!((s.progress - 0.75).abs() < 1e-12);
        let e = t.eval(1000.0);
        assert_eq!(e.iteration, 2.0);
        assert!((e.progress - 0.5).abs() < 1e-12);
        let t2 = Timing { duration: 100.0, iterations: 2.0, direction: Direction::Alternate, ..Default::default() };
        assert_eq!(t2.eval(1000.0).progress, 0.0);
    }

    #[test]
    fn zero_duration() {
        let t = Timing { duration: 0.0, ..Default::default() };
        assert_eq!(t.eval(-1.0).phase, Phase::Before);
        let s = t.eval(0.0);
        assert_eq!(s.phase, Phase::After);
        assert_eq!(s.progress, 1.0);
    }
}
