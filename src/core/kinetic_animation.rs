//! Kinetic (momentum) scrolling animation.
//!
//! Based on LWC's `kinetic-animation.ts` - provides smooth deceleration
//! after touch/mouse release.

/// Constants for kinetic scrolling behavior (matching LWC).
pub mod constants {
    /// Minimum speed before animation stops.
    pub const MIN_SCROLL_SPEED: f64 = 0.2;
    /// Maximum initial speed cap.
    pub const MAX_SCROLL_SPEED: f64 = 7.0;
    /// Friction coefficient (velocity multiplier per frame).
    pub const DUMPING_COEFF: f64 = 0.997;
    /// Minimum drag distance to trigger kinetic scrolling.
    pub const SCROLL_MIN_MOVE: f64 = 15.0;
}

/// Kinetic scrolling animation state.
///
/// Tracks velocity and provides position updates with exponential decay.
#[derive(Debug, Clone, Default)]
pub struct KineticAnimation {
    /// Initial velocity when animation started.
    start_speed: f64,
    /// Timestamp when animation started (ms).
    start_time: f64,
    /// Whether animation is currently active.
    active: bool,
}

impl KineticAnimation {
    /// Create a new inactive animation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start the animation with given initial velocity.
    pub fn start(&mut self, speed: f64, now_ms: f64) {
        // Cap speed to prevent crazy scrolling
        let capped = speed.clamp(-constants::MAX_SCROLL_SPEED, constants::MAX_SCROLL_SPEED);

        // Only start if velocity is significant
        if capped.abs() < constants::MIN_SCROLL_SPEED {
            self.active = false;
            return;
        }

        self.start_speed = capped;
        self.start_time = now_ms;
        self.active = true;
    }

    /// Stop the animation.
    pub fn stop(&mut self) {
        self.active = false;
        self.start_speed = 0.0;
    }

    /// Check if animation is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Check if animation has finished (velocity below threshold).
    pub fn finished(&self, now_ms: f64) -> bool {
        if !self.active {
            return true;
        }
        self.speed(now_ms).abs() < constants::MIN_SCROLL_SPEED
    }

    /// Get current velocity at given time.
    pub fn speed(&self, now_ms: f64) -> f64 {
        if !self.active {
            return 0.0;
        }
        let dt = now_ms - self.start_time;
        // Exponential decay: v(t) = v0 * c^t
        self.start_speed * constants::DUMPING_COEFF.powf(dt)
    }

    /// Get position delta since animation start.
    ///
    /// Integral of velocity: p(t) = v0 * (c^t - 1) / ln(c)
    pub fn get_position(&self, now_ms: f64) -> f64 {
        if !self.active {
            return 0.0;
        }
        let dt = now_ms - self.start_time;
        let ln_c = constants::DUMPING_COEFF.ln();
        self.start_speed * (constants::DUMPING_COEFF.powf(dt) - 1.0) / ln_c
    }

    /// Update the animation and return position delta since last update.
    ///
    /// Returns `None` if animation is not active or has finished.
    pub fn update(&mut self, now_ms: f64, last_update_ms: f64) -> Option<f64> {
        if !self.active {
            return None;
        }

        if self.finished(now_ms) {
            self.stop();
            return None;
        }

        // Compute position delta between last update and now
        let pos_now = self.get_position(now_ms);
        let pos_last = self.get_position(last_update_ms);

        Some(pos_now - pos_last)
    }
}

/// Scroll state tracking for a single axis.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Kinetic animation for this axis.
    pub animation: KineticAnimation,
    /// Last update timestamp.
    pub last_update_ms: f64,
    /// Whether currently being dragged.
    pub dragging: bool,
    /// Starting position when drag began.
    pub drag_start_pos: f64,
    /// Starting value when drag began (e.g., viewport start_bar).
    pub drag_start_value: f64,
    /// Velocity samples for kinetic animation (position, time).
    velocity_samples: Vec<(f64, f64)>,
}

impl ScrollState {
    /// Create new scroll state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a drag operation.
    pub fn start_drag(&mut self, pos: f64, value: f64, now_ms: f64) {
        self.animation.stop();
        self.dragging = true;
        self.drag_start_pos = pos;
        self.drag_start_value = value;
        self.last_update_ms = now_ms;
        self.velocity_samples.clear();
        self.velocity_samples.push((pos, now_ms));
    }

    /// Update drag position and compute velocity.
    pub fn update_drag(&mut self, pos: f64, now_ms: f64) {
        if !self.dragging {
            return;
        }

        // Keep only recent samples (last 100ms)
        self.velocity_samples.retain(|(_, t)| now_ms - t < 100.0);
        self.velocity_samples.push((pos, now_ms));
        self.last_update_ms = now_ms;
    }

    /// End drag and potentially start kinetic animation.
    pub fn end_drag(&mut self, now_ms: f64) {
        if !self.dragging {
            return;
        }

        self.dragging = false;

        // Compute velocity from recent samples
        if let (Some(first), Some(last)) =
            (self.velocity_samples.first(), self.velocity_samples.last())
        {
            if self.velocity_samples.len() >= 2 {
                let dt = last.1 - first.1;
                if dt > 0.0 {
                    let velocity = (last.0 - first.0) / dt;
                    self.animation.start(velocity, now_ms);
                }
            }
        }

        self.velocity_samples.clear();
    }

    /// Get drag delta from start position.
    pub fn drag_delta(&self, current_pos: f64) -> f64 {
        current_pos - self.drag_start_pos
    }

    /// Update and return position delta from kinetic animation.
    pub fn update_kinetic(&mut self, now_ms: f64) -> Option<f64> {
        let delta = self.animation.update(now_ms, self.last_update_ms);
        self.last_update_ms = now_ms;
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kinetic_animation_decay() {
        let mut anim = KineticAnimation::new();
        anim.start(5.0, 0.0);

        assert!(anim.is_active());

        // Velocity should decrease over time
        let v0 = anim.speed(0.0);
        let v100 = anim.speed(100.0);
        let v1000 = anim.speed(1000.0);

        assert!(v0 > v100);
        assert!(v100 > v1000);
    }

    #[test]
    fn test_kinetic_below_threshold_does_not_start() {
        let mut anim = KineticAnimation::new();
        anim.start(0.1, 0.0); // Below MIN_SCROLL_SPEED

        assert!(!anim.is_active());
    }

    #[test]
    fn test_scroll_state_drag() {
        let mut state = ScrollState::new();

        state.start_drag(100.0, 0.0, 0.0);
        assert!(state.dragging);
        assert_eq!(state.drag_delta(150.0), 50.0);

        state.update_drag(150.0, 50.0);
        state.update_drag(200.0, 100.0);
        state.end_drag(100.0);

        assert!(!state.dragging);
        // Animation should have started with positive velocity
        assert!(state.animation.is_active());
    }
}
