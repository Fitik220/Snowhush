//! Snow resting on an approximation of the standard Windows arrow.
//! The pointer's actual pixels are owned by Windows; only the sloping upper
//! edge is modelled here. Coordinates are DIPs, with Y pointing upwards.
use crate::geometry::V2;

const SITES: usize = 10;
const SPACING: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Low,
    Medium,
    High,
}

impl Level {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];
    pub fn title(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
    pub fn from_index(value: u32) -> Self {
        Self::ALL.get(value as usize).copied().unwrap_or(Self::Medium)
    }
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|&v| v == self).unwrap_or(1) as u32
    }
    fn thickness(self) -> f32 {
        match self {
            Self::Low => 1.7,
            Self::Medium => 3.2,
            Self::High => 5.0,
        }
    }
    fn grip(self) -> f32 {
        match self {
            Self::Low => 350.0,
            Self::Medium => 750.0,
            Self::High => 1400.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CursorSettings {
    pub enabled: bool,
    pub amount: Level,
    pub adhesion: Level,
    pub shake: bool,
    pub melting: bool,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self { enabled: false, amount: Level::Medium, adhesion: Level::Medium, shake: true, melting: true }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CursorSample {
    /// Position of the visual arrow tip in overlay coordinates.
    pub tip: V2,
    pub visible: bool,
    pub arrow: bool,
    /// Native HCURSOR value, used only to detect a shape change.
    pub identity: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Detached {
    pub position: V2,
    pub velocity: V2,
    pub area: f32,
}

pub struct CursorSnow {
    pub settings: CursorSettings,
    heights: [f32; SITES],
    sample: Option<CursorSample>,
    previous: Option<CursorSample>,
    velocity: V2,
    detached: [Option<Detached>; SITES],
    count: usize,
    changed: bool,
    moving: bool,
}

impl CursorSnow {
    pub fn new() -> Self {
        Self {
            settings: CursorSettings::default(),
            heights: [0.0; SITES],
            sample: None,
            previous: None,
            velocity: V2::ZERO,
            detached: [None; SITES],
            count: 0,
            changed: false,
            moving: false,
        }
    }

    pub fn sample(&mut self, sample: Option<CursorSample>) {
        self.sample = sample;
    }
    pub fn count(&self) -> usize {
        self.heights.iter().filter(|&&h| h > 0.05).count()
    }
    pub fn total(&self) -> f32 {
        self.heights.iter().sum()
    }
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
    pub fn moving(&self) -> bool {
        self.moving
    }
    pub fn visible(&self) -> bool {
        self.settings.enabled && self.sample.is_some_and(|s| s.visible && s.arrow)
    }

    fn surface(i: usize) -> V2 {
        let x = (i as f32 + 0.5) * SPACING;
        // The upper/right sloping edge of the familiar arrow. Small irregularity
        // avoids a straight white stripe, while staying within its silhouette.
        V2::new(x, -0.94 * x + 0.16 * (i as f32 * 2.4).sin())
    }

    pub fn collide(&mut self, from: V2, to: V2, area: f32) -> bool {
        if !self.visible() || to.y >= from.y {
            return false;
        }
        let tip = self.sample.unwrap().tip;
        let x = to.x - tip.x;
        let i = (x / SPACING).floor() as isize;
        if !(0..SITES as isize).contains(&i) {
            return false;
        }
        let i = i as usize;
        let y = tip.y + Self::surface(i).y + self.heights[i];
        if from.y < y || to.y > y {
            return false;
        }
        let cap = self.settings.amount.thickness();
        if self.heights[i] >= cap {
            return false;
        }
        self.heights[i] = (self.heights[i] + area / SPACING * 0.5).min(cap);
        self.changed = true;
        true
    }

    /// Advances attachment physics and returns at most one loose grain per site.
    /// A discontinuous cursor jump sheds snow at its old position, never dragging
    /// it across the screen. Velocity is filtered in seconds, not frames.
    pub fn advance(&mut self, dt: f32, wind: f32) -> &[Option<Detached>] {
        self.count = 0;
        self.moving = false;
        self.detached.fill(None);
        let Some(sample) = self.sample else {
            self.previous = None;
            self.clear();
            return &self.detached[..0];
        };
        if !self.settings.enabled || !sample.visible || !sample.arrow {
            self.clear();
            self.previous = Some(sample);
            return &self.detached[..0];
        }
        let previous = self.previous.replace(sample);
        let Some(previous) = previous.filter(|p| p.visible && p.arrow && p.identity == sample.identity) else {
            if self.total() > 0.0 {
                self.changed = true;
            }
            self.heights.fill(0.0);
            self.velocity = V2::ZERO;
            return &self.detached[..0];
        };
        let dt = dt.max(0.001);
        let displacement = sample.tip - previous.tip;
        self.moving = self.total() > 0.0 && displacement.length_squared() > 0.01;
        self.changed |= self.moving;
        let speed = displacement.length() / dt;
        let velocity = displacement / dt;
        let acceleration = (velocity - self.velocity).length() / dt;
        self.velocity += (velocity - self.velocity) * (1.0 - (-dt / 0.055).exp());
        let jump = speed > 2400.0 || displacement.length() > 90.0;
        let grip = self.settings.adhesion.grip();
        let shake = self.settings.shake && speed > 100.0 && acceleration > grip * 2.0;
        let slide = (speed > grip * 0.35 || (speed > 60.0 && acceleration > grip)) && !jump;
        for i in 0..SITES {
            let height = self.heights[i];
            if height <= 0.05 {
                continue;
            }
            if self.settings.melting {
                self.heights[i] = (height - dt * 0.07).max(0.0);
                self.changed |= self.heights[i] != height;
            }
            let fraction = if jump {
                1.0
            } else if shake {
                (dt * 5.0).min(0.8)
            } else if slide {
                (dt * 1.3).min(0.3)
            } else {
                0.0
            };
            if fraction > 0.0 {
                let removed = self.heights[i] * fraction;
                self.heights[i] -= removed;
                self.changed = true;
                let point = Self::surface(i);
                let origin = if jump { previous.tip } else { sample.tip };
                self.detached[self.count] = Some(Detached {
                    position: origin + point + V2::new(0.0, height),
                    velocity: if jump { V2::new(wind * 0.2, -30.0) } else { velocity * 0.35 + V2::new(wind * 0.2, 25.0) },
                    area: removed * SPACING * 2.0,
                });
                self.count += 1;
            }
        }
        &self.detached[..self.count]
    }

    pub fn clear(&mut self) {
        self.changed |= self.total() > 0.0;
        self.heights.fill(0.0);
        self.velocity = V2::ZERO;
    }

    pub fn sprites(&self, mut put: impl FnMut(V2, f32, f32)) {
        if !self.visible() {
            return;
        }
        let tip = self.sample.unwrap().tip;
        for (i, &height) in self.heights.iter().enumerate() {
            if height > 0.05 {
                let p = tip + Self::surface(i) + V2::new(0.0, height * 0.45);
                put(p, (height * 0.48).clamp(0.55, 1.8), (height / 0.7).clamp(0.0, 0.94));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(x: f32, identity: usize) -> CursorSample {
        CursorSample { tip: V2::new(x, 100.0), visible: true, arrow: true, identity }
    }
    #[test]
    fn accumulates_up_to_configured_thickness() {
        let mut snow = CursorSnow::new();
        snow.settings.enabled = true;
        snow.settings.amount = Level::Low;
        snow.sample(Some(sample(0.0, 1)));
        snow.advance(0.016, 0.0);
        for _ in 0..100 {
            assert!(snow.collide(V2::new(4.0, 105.0), V2::new(4.0, 90.0), 1.0) || snow.total() >= 1.7);
        }
        assert!(snow.total() > 0.0 && snow.total() <= 1.7);
    }
    #[test]
    fn slow_motion_carries_snow_and_fast_motion_detaches() {
        let mut snow = CursorSnow::new();
        snow.settings.enabled = true;
        snow.settings.melting = false;
        snow.sample(Some(sample(0.0, 1)));
        snow.advance(0.016, 0.0);
        snow.collide(V2::new(4.0, 105.0), V2::new(4.0, 90.0), 3.0);
        let before = snow.total();
        snow.sample(Some(sample(0.2, 1)));
        snow.advance(0.016, 0.0);
        assert_eq!(snow.total(), before);
        snow.sample(Some(sample(50.0, 1)));
        assert!(!snow.advance(0.016, 0.0).is_empty());
        assert!(snow.total() < before);
    }
    #[test]
    fn shape_change_and_hidden_cursor_clear_snow() {
        let mut snow = CursorSnow::new();
        snow.settings.enabled = true;
        snow.sample(Some(sample(0.0, 1)));
        snow.advance(0.016, 0.0);
        snow.collide(V2::new(4.0, 105.0), V2::new(4.0, 90.0), 3.0);
        snow.sample(Some(sample(0.0, 2)));
        snow.advance(0.016, 0.0);
        assert_eq!(snow.total(), 0.0);
        snow.sample(Some(CursorSample { visible: false, ..sample(0.0, 2) }));
        snow.advance(0.016, 0.0);
        assert_eq!(snow.total(), 0.0);
    }
    #[test]
    fn melting_and_disabled_shake_behave_as_configured() {
        let mut snow = CursorSnow::new();
        snow.settings.enabled = true;
        snow.settings.shake = false;
        snow.sample(Some(sample(0.0, 1)));
        snow.advance(0.016, 0.0);
        snow.collide(V2::new(4.0, 105.0), V2::new(4.0, 90.0), 3.0);
        let before = snow.total();
        snow.sample(Some(sample(0.0, 1)));
        snow.advance(1.0, 0.0);
        assert!(snow.total() < before);
        assert!(snow.advance(1.0, 0.0).is_empty());
    }
    #[test]
    fn level_registry_values_round_trip() {
        for value in Level::ALL {
            assert_eq!(Level::from_index(value.index()), value);
        }
        assert_eq!(Level::from_index(u32::MAX), Level::Medium);
    }
}
