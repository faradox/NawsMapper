//! Connected displays, and which one the projector output should use.
//!
//! egui places a viewport in borderless full screen on a monitor by its
//! index in winit's `available_monitors()` order. The index changes when
//! displays are plugged in or out, so the choice is remembered by name (plus
//! which of several same-named displays) and resolved again each time.

use serde::{Deserialize, Serialize};

/// A connected display.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    /// Native size in physical pixels.
    pub size: (u32, u32),
    pub refresh_hz: Option<f32>,
    /// The editor window is on this display.
    pub has_editor: bool,
}

impl Display {
    pub fn describe(&self) -> String {
        let (w, h) = self.size;
        let hz = self.refresh_hz.map(|r| format!(", {r:.0} Hz")).unwrap_or_default();
        let editor = if self.has_editor { " (this screen)" } else { "" };
        format!("{} — {w}×{h}{hz}{editor}", self.name)
    }
}

/// The display chosen for the projector output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayChoice {
    pub name: String,
    /// Which one among displays with this name (two identical projectors), in winit's order.
    pub ordinal: usize,
}

impl DisplayChoice {
    /// The choice that picks `displays[index]`.
    pub fn of(displays: &[Display], index: usize) -> Option<Self> {
        let name = &displays.get(index)?.name;
        let ordinal = displays[..index].iter().filter(|d| &d.name == name).count();
        Some(Self { name: name.clone(), ordinal })
    }

    /// Index of the chosen display among the connected ones, if it is connected.
    pub fn resolve(&self, displays: &[Display]) -> Option<usize> {
        displays.iter().enumerate().filter(|(_, d)| d.name == self.name).nth(self.ordinal).map(|(i, _)| i)
    }

    pub fn label(&self) -> String {
        if self.ordinal == 0 { self.name.clone() } else { format!("{} #{}", self.name, self.ordinal + 1) }
    }
}

/// The displays connected right now, in winit's order.
pub fn connected(frame: &eframe::Frame) -> Vec<Display> {
    let Some(window) = frame.winit_window() else { return Vec::new() };
    let current = window.current_monitor();
    window
        .available_monitors()
        .map(|m| {
            let size = m.size();
            Display {
                name: m.name().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Display".into()),
                size: (size.width, size.height),
                refresh_hz: m.refresh_rate_millihertz().map(|mhz| mhz as f32 / 1000.0),
                has_editor: current.as_ref() == Some(&m),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(name: &str) -> Display {
        Display { name: name.into(), size: (1920, 1080), refresh_hz: Some(60.0), has_editor: false }
    }

    #[test]
    fn choice_follows_the_display_when_others_come_and_go() {
        let before = [d("Built-in"), d("EPSON")];
        let choice = DisplayChoice::of(&before, 1).unwrap();
        assert_eq!(choice, DisplayChoice { name: "EPSON".into(), ordinal: 0 });
        let after = [d("LG"), d("Built-in"), d("EPSON")];
        assert_eq!(choice.resolve(&after), Some(2));
        assert_eq!(choice.resolve(&[d("Built-in")]), None);
    }

    #[test]
    fn identical_projectors_stay_apart() {
        let displays = [d("EPSON"), d("Built-in"), d("EPSON")];
        let second = DisplayChoice::of(&displays, 2).unwrap();
        assert_eq!(second.ordinal, 1);
        assert_eq!(second.resolve(&displays), Some(2));
        assert_eq!(second.label(), "EPSON #2");
        assert_eq!(DisplayChoice::of(&displays, 0).unwrap().label(), "EPSON");
        assert_eq!(DisplayChoice::of(&displays, 3), None);
    }

    #[test]
    fn description_names_size_rate_and_editor_screen() {
        let mut built_in = d("Built-in");
        built_in.has_editor = true;
        assert_eq!(built_in.describe(), "Built-in — 1920×1080, 60 Hz (this screen)");
        let mut pj = d("EPSON");
        pj.refresh_hz = None;
        assert_eq!(pj.describe(), "EPSON — 1920×1080");
    }
}
