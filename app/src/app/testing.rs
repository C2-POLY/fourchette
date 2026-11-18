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

/// How much bigger than the size a view asks for the loupe is drawn (#253).
pub(super) const SCALE: f32 = 1.6;

// The magnifying glass's materials (#253).
const GOLD_LIGHT: Color32 = Color32::from_rgb(0xff, 0xd8, 0x7a);
const GOLD_DARK: Color32 = Color32::from_rgb(0xc2, 0x78, 0x0c);
const GLASS: Color32 = Color32::from_rgb(0x9f, 0xd4, 0xf2);
const GLASS_LIGHT: Color32 = Color32::from_rgb(0xe4, 0xf5, 0xff);
const WOOD: Color32 = Color32::from_rgb(0x5a, 0x34, 0x22);
const WOOD_LIGHT: Color32 = Color32::from_rgb(0x8e, 0x5a, 0x3a);

/// A thick bar from `a` to `b`, `w` wide, with round ends.
fn bar(p: &Painter, a: Pos2, b: Pos2, w: f32, fill: Color32) {
    let d = (b - a).normalized();
    let n = vec2(-d.y, d.x) * (w / 2.0);
    p.add(Shape::convex_polygon(vec![a + n, b + n, b - n, a - n], fill, Stroke::NONE));
    p.circle_filled(a, w / 2.0, fill);
    p.circle_filled(b, w / 2.0, fill);
}

/// A four-pointed sparkle at `c`, `r` from its centre to a point.
fn sparkle(p: &Painter, c: Pos2, r: f32, fill: Color32) {
    let t = r * 0.22;
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -r), c + vec2(t, 0.0), c + vec2(0.0, r), c + vec2(-t, 0.0)], fill, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![c + vec2(-r, 0.0), c + vec2(0.0, -t), c + vec2(r, 0.0), c + vec2(0.0, t)], fill, Stroke::NONE));
}

/// The loupe, `size` points tall (times `SCALE`), floating just above `at` (the top of the
/// ticket): a magnifying glass with a gold rim, tinted glass catching the light and a wooden
/// handle, outlined in ink like a sticker. It bobs and sways as if it were inspecting, a ping
/// ring spreads from it and a sparkle blinks on its rim.
pub(super) fn loupe(p: &Painter, at: Pos2, size: f32, time: f32, phase: f32) {
    let size = size * SCALE;
    let k = pulse(time, phase);
    let bob = (time * 2.1 + phase).sin();
    let r = size * 0.27;
    let ink = (size * 0.035).max(1.0);
    let c = at - vec2(0.0, size * (0.68 + 0.05 * bob));

    // The glow behind it, then a ping ring spreading out and fading, once a second or so.
    p.circle_filled(c, r * 1.75, AMBER.gamma_multiply(0.12 + 0.12 * k));
    let ping = ((time + phase) * 0.7).fract();
    p.circle_stroke(c, r * (1.25 + 1.1 * ping), Stroke::new(size * 0.045 * (1.0 - ping), AMBER.gamma_multiply(0.8 * (1.0 - ping))));

    // The handle down to the right, swaying a little: ink, wood with its lit side, the gold collar.
    let a = (45.0 + 9.0 * (time * 1.3 + phase).sin()).to_radians();
    let d = vec2(a.cos(), a.sin());
    let (h0, h1, h2) = (c + d * r * 1.05, c + d * r * 1.42, c + d * r * 2.3);
    let w = r * 0.5;
    bar(p, h0, h2, w + ink * 2.0, INK);
    bar(p, h1, h2, w, WOOD);
    let n = vec2(-d.y, d.x) * (w * 0.2);
    bar(p, h1 - n + d * w * 0.3, h2 - n - d * w * 0.15, w * 0.22, WOOD_LIGHT);
    bar(p, h0, h1, w * 1.12, GOLD_DARK);
    bar(p, h0 - n * 0.6, h1 - n * 0.6, w * 0.3, GOLD_LIGHT);

    // The rim: ink, gold, its lit edge.
    p.circle_filled(c, r * 1.22 + ink, INK);
    p.circle_filled(c, r * 1.22, AMBER);
    p.circle_stroke(c, r * 1.13, Stroke::new(r * 0.07, GOLD_LIGHT));
    p.circle_filled(c, r * 1.0, GOLD_DARK);
    // The glass: deeper at the bottom right, clearer at the top left, and its reflections.
    p.circle_filled(c, r * 0.95, GLASS);
    p.circle_filled(c - vec2(r * 0.13, r * 0.13), r * 0.78, GLASS_LIGHT);
    let arc: Vec<Pos2> = (0..=10).map(|i| (200.0 + 7.0 * i as f32).to_radians()).map(|t| c + vec2(t.cos(), t.sin()) * r * 0.62).collect();
    let (first, last) = (arc[0], arc[arc.len() - 1]);
    p.add(Shape::line(arc, Stroke::new(r * 0.17, Color32::WHITE)));
    p.circle_filled(first, r * 0.085, Color32::WHITE);
    p.circle_filled(last, r * 0.085, Color32::WHITE);
    p.circle_filled(c + vec2(r * 0.08, -r * 0.6), r * 0.09, Color32::WHITE);

    // The sparkle on the rim, top right, as the pulse peaks.
    if k > 0.35 {
        let s = c + vec2(1.0, -1.0).normalized() * r * 1.22;
        sparkle(p, s, r * 0.55 * (k - 0.35) / 0.65, Color32::WHITE);
    }
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
