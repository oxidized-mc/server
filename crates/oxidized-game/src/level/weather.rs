//! Weather types for the Minecraft world.

/// The weather state the server can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherType {
    /// Clear skies — sets `clear_weather_time` to the given duration.
    Clear,
    /// Rain without thunder.
    Rain,
    /// Thunderstorm (implies rain).
    Thunder,
}

impl WeatherType {
    /// Parses a weather type from a command string.
    ///
    /// Accepts `"clear"`, `"rain"`, and `"thunder"` (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "clear" => Some(Self::Clear),
            "rain" => Some(Self::Rain),
            "thunder" => Some(Self::Thunder),
            _ => None,
        }
    }

    /// Returns the canonical name for display.
    pub fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Rain => "rain",
            Self::Thunder => "thunder",
        }
    }
}
