//! What marks a ticket To test in every view of the board (#249): the same amber loupe pulsing
//! over it, whatever the skin, besides each view's own staging (the city's inspection, the
//! stadium's photo-finish, the mountain's checkpoint, the food court's tasting, the room's test
//! bench). Read only, drawn with the painter.

use egui::{vec2, Color32, Painter, Pos2, Shape, Stroke};

/// The amber of what waits to be tested.
pub(super) const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa6, 0x23);
const INK: Color32 = Color32::from_rgb(0x2b, 0x22, 0x10);

/// How far the pulse is at `time`, 0 to 1, about once a second; `phase` keeps neighbours apart.
pub(super) fn pulse(time: f32, phase: f32) -> f32 {
    0.5 + 0.5 * (time * 3.4 + phase).sin()
}

/// Whether a blinking light is on at `time`: on for a third of each `period` seconds.
pub(super) fn blink(time: f32, period: f32, phase: f32) -> bool {
    ((time + phase) / period).fract() < 0.34
}

/// The loupe's pin, `size` points tall, its point on `at` (just above the ticket): an amber disc
/// with a white loupe, its halo breathing with `time`.
pub(super) fn loupe(p: &Painter, at: Pos2, size: f32, time: f32, phase: f32) {
    let k = pulse(time, phase);
    let r = size * 0.36 * (1.0 + 0.07 * k);
    let c = at - vec2(0.0, size * 0.62 + k * size * 0.06);
    p.circle_filled(c, r * (1.35 + 0.55 * k), AMBER.gamma_multiply(0.30 * (1.0 - k) + 0.06));
    // The pin's point, then its disc.
    let tip = at;
    p.add(Shape::convex_polygon(vec![c + vec2(-r * 0.55, r * 0.6), c + vec2(r * 0.55, r * 0.6), tip], AMBER, Stroke::NONE));
    p.circle_filled(c, r, AMBER);
    p.circle_stroke(c, r, Stroke::new((size * 0.05).max(1.0), INK.gamma_multiply(0.55)));
    // The loupe: a ring and its handle.
    let lens = c + vec2(-r * 0.14, -r * 0.14);
    let w = (size * 0.085).max(1.2);
    p.circle_stroke(lens, r * 0.42, Stroke::new(w, Color32::WHITE));
    p.line_segment([lens + vec2(r * 0.3, r * 0.3), lens + vec2(r * 0.66, r * 0.66)], Stroke::new(w * 1.25, Color32::WHITE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pulse_stays_between_0_and_1_and_the_blink_is_on_a_third_of_the_time() {
        let samples: Vec<f32> = (0..400).map(|i| pulse(i as f32 * 0.01, 0.3)).collect();
        assert!(samples.iter().all(|k| (0.0..=1.0).contains(k)));
        let on = (0..900).filter(|i| blink(*i as f32 / 300.0, 1.5, 0.0)).count();
        assert!((290..=320).contains(&on), "{on}");
    }
}
