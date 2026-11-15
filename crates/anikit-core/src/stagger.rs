//! Stagger: distribute delays (or any value) across many targets, in 1D or
//! across a 2D grid, from any origin, with easing over the distribution.

use alloc::vec::Vec;

use crate::easing::Easing;
use crate::math::{abs, sqrt, Rng};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StaggerFrom {
    First,
    Last,
    Center,
    /// Outer items first, centre last.
    Edges,
    /// Seeded random order.
    Random(u32),
    /// Fractional index (or grid cell index) of the origin.
    Index(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stagger {
    /// Value step between neighbouring items (ignored when `total` is set).
    pub each: f64,
    /// Spread the whole distribution over exactly this amount instead.
    pub total: Option<f64>,
    /// Added to every value.
    pub start: f64,
    pub from: StaggerFrom,
    /// `(columns, rows)` for grid distance.
    pub grid: Option<(usize, usize)>,
    pub axis: Option<Axis>,
    pub ease: Easing,
}

impl Default for Stagger {
    fn default() -> Self {
        Stagger {
            each: 100.0,
            total: None,
            start: 0.0,
            from: StaggerFrom::First,
            grid: None,
            axis: None,
            ease: Easing::Linear,
        }
    }
}

/// Compute one value per item.
pub fn stagger(count: usize, s: &Stagger) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let mut d: Vec<f64> = Vec::with_capacity(count);
    let last = (count - 1) as f64;
    match s.grid {
        Some((cols, rows)) if cols > 0 => {
            let rows = rows.max(1);
            let (cw, rh) = ((cols - 1) as f64, (rows - 1) as f64);
            let (ox, oy) = match s.from {
                StaggerFrom::First | StaggerFrom::Random(_) => (0.0, 0.0),
                StaggerFrom::Last => (cw, rh),
                StaggerFrom::Center | StaggerFrom::Edges => (cw / 2.0, rh / 2.0),
                StaggerFrom::Index(i) => {
                    let i = i.max(0.0);
                    (i % cols as f64, crate::math::floor(i / cols as f64))
                }
            };
            for i in 0..count {
                let x = (i % cols) as f64;
                let y = (i / cols) as f64;
                let (dx, dy) = (x - ox, y - oy);
                d.push(match s.axis {
                    Some(Axis::X) => abs(dx),
                    Some(Axis::Y) => abs(dy),
                    None => sqrt(dx * dx + dy * dy),
                });
            }
        }
        _ => {
            let o = match s.from {
                StaggerFrom::First | StaggerFrom::Random(_) => 0.0,
                StaggerFrom::Last => last,
                StaggerFrom::Center | StaggerFrom::Edges => last / 2.0,
                StaggerFrom::Index(i) => i,
            };
            for i in 0..count {
                d.push(abs(i as f64 - o));
            }
        }
    }
    let maxd = d.iter().cloned().fold(0.0, f64::max);
    if let StaggerFrom::Edges = s.from {
        for v in d.iter_mut() {
            *v = maxd - *v;
        }
    }
    if let StaggerFrom::Random(seed) = s.from {
        // Random permutation of evenly spaced slots (no clumping).
        let mut rng = Rng(seed);
        let mut slots: Vec<f64> = (0..count).map(|i| i as f64).collect();
        for i in (1..count).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            slots.swap(i, j);
        }
        d = slots;
    }
    let maxd = d.iter().cloned().fold(0.0, f64::max);
    let span = match s.total {
        Some(t) => t,
        None => s.each * maxd,
    };
    d.into_iter()
        .map(|v| {
            let n = if maxd > 0.0 { v / maxd } else { 0.0 };
            s.start + s.ease.ease(n) * span
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_from_variants() {
        let s = Stagger { each: 10.0, ..Default::default() };
        assert_eq!(stagger(4, &s), [0.0, 10.0, 20.0, 30.0]);
        let s = Stagger { each: 10.0, from: StaggerFrom::Center, ..Default::default() };
        assert_eq!(stagger(5, &s), [20.0, 10.0, 0.0, 10.0, 20.0]);
        let s = Stagger { each: 10.0, from: StaggerFrom::Edges, ..Default::default() };
        assert_eq!(stagger(5, &s), [0.0, 10.0, 20.0, 10.0, 0.0]);
        let s = Stagger { total: Some(100.0), from: StaggerFrom::Last, ..Default::default() };
        assert_eq!(stagger(3, &s), [100.0, 50.0, 0.0]);
    }
    #[test]
    fn grid_and_random() {
        let s = Stagger { each: 1.0, grid: Some((3, 3)), from: StaggerFrom::Center, ..Default::default() };
        let v = stagger(9, &s);
        assert_eq!(v[4], 0.0);
        assert!((v[0] - 2f64.sqrt()).abs() < 1e-12 && v[1] == 1.0);
        let s = Stagger { each: 1.0, from: StaggerFrom::Random(7), ..Default::default() };
        let mut v = stagger(10, &s);
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(v, (0..10).map(|i| i as f64).collect::<Vec<_>>());
    }
}
