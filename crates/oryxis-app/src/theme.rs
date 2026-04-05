use iced::Color;

/// Oryxis dark theme colors — inspired by Termius
pub struct OryxisColors;

#[allow(dead_code)]
impl OryxisColors {
    // Backgrounds
    pub const BG_PRIMARY: Color = Color::from_rgb(0.09, 0.09, 0.12);      // #17171F
    pub const BG_SIDEBAR: Color = Color::from_rgb(0.07, 0.07, 0.10);      // #12121A
    pub const BG_SURFACE: Color = Color::from_rgb(0.12, 0.12, 0.16);      // #1F1F29
    pub const BG_HOVER: Color = Color::from_rgb(0.15, 0.15, 0.20);        // #262633
    pub const BG_SELECTED: Color = Color::from_rgb(0.18, 0.18, 0.25);     // #2E2E40

    // Text
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.90, 0.91, 0.93);     // #E6E8ED
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.55, 0.56, 0.62);   // #8C8F9E
    pub const TEXT_MUTED: Color = Color::from_rgb(0.35, 0.36, 0.42);       // #595B6B

    // Accent
    pub const ACCENT: Color = Color::from_rgb(0.30, 0.56, 1.0);           // #4D8FFF
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.40, 0.63, 1.0);     // #66A1FF
    pub const SUCCESS: Color = Color::from_rgb(0.30, 0.78, 0.55);         // #4DC78C
    pub const WARNING: Color = Color::from_rgb(0.95, 0.73, 0.25);         // #F2BA40
    pub const ERROR: Color = Color::from_rgb(0.92, 0.33, 0.38);           // #EB5461

    // Terminal
    pub const TERMINAL_BG: Color = Color::from_rgb(0.06, 0.06, 0.08);     // #0F0F14
    pub const TERMINAL_FG: Color = Color::from_rgb(0.85, 0.87, 0.90);     // #D9DEE6
    pub const TERMINAL_CURSOR: Color = Color::from_rgb(0.30, 0.56, 1.0);  // #4D8FFF

    // Borders
    pub const BORDER: Color = Color::from_rgb(0.16, 0.16, 0.22);          // #292938
    pub const BORDER_FOCUS: Color = Color::from_rgb(0.30, 0.56, 1.0);     // #4D8FFF
}
