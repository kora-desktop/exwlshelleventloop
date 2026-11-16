use std::{borrow::Cow, fs::File};

use iced_core::{Font, Pixels};

use crate::reexport::{Anchor, KeyboardInteractivity, Layer};

pub use layershellev::LayerTransition;
pub use layershellev::StartMode;

use layershellev::reexport::wayland_client::{Connection, wl_keyboard::KeymapFormat};

#[derive(Debug)]
pub struct VirtualKeyboardSettings {
    pub file: File,
    pub keymap_size: u32,
    pub keymap_format: KeymapFormat,
}
/// MainSettings for iced_layershell
/// different from [`crate::Settings`], it does not contain the field of flags
#[derive(Debug)]
pub struct Settings {
    /// The identifier of the application.
    ///
    /// If provided, this identifier may be used to identify the application or
    /// communicate with it through the windowing system.
    pub id: Option<String>,

    /// settings for layer shell
    pub layer_settings: LayerShellSettings,
    /// The data needed to initialize an Application
    ///
    /// The fonts to load on boot.
    pub fonts: Vec<Cow<'static, [u8]>>,

    /// The default [`Font`] to be used.
    ///
    /// By default, it uses [`Family::SansSerif`](iced::font::Family::SansSerif).
    pub default_font: Font,

    /// The text size that will be used by default.
    ///
    /// The default value is `16.0`.
    pub default_text_size: Pixels,

    /// If set to true, the renderer will try to perform antialiasing for some
    /// primitives.
    ///
    /// Enabling it can produce a smoother result in some widgets, like the
    /// `Canvas`, at a performance cost.
    ///
    /// By default, it is disabled.
    ///
    pub antialiasing: bool,

    pub virtual_keyboard_support: Option<VirtualKeyboardSettings>,

    /// set the used wayland connection, all wayland object will share it, and they can be used by
    /// each other.
    pub with_connection: Option<Connection>,
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            id: None,
            fonts: Vec::new(),
            layer_settings: LayerShellSettings::default(),
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: false,
            virtual_keyboard_support: None,
            with_connection: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerShellSettings {
    pub anchor: Anchor,
    pub layer: Layer,
    pub exclusive_zone: i32,
    pub size: Option<(u32, u32)>,
    pub margin: (i32, i32, i32, i32),
    pub keyboard_interactivity: KeyboardInteractivity,
    pub start_mode: StartMode,
    pub events_transparent: bool,
    /// Request blur effect for this surface (requires compositor support for org_kde_kwin_blur)
    pub blur: bool,
    /// Custom blur radius in pixels. When `blur` is true and this is `Some(r)`,
    /// the compositor uses radius `r` instead of its default.
    /// Requires compositor support for org_kde_kwin_blur version 2.
    pub blur_radius: Option<f32>,
    /// Backdrop saturation multiplier for the blur, matching CSS
    /// `backdrop-filter: saturate(x)`. When `blur` is true and this is `Some(s)`,
    /// the compositor saturates the blurred backdrop by `s` (1.0 = unchanged).
    /// Requires compositor support for org_kde_kwin_blur version 3.
    pub blur_saturation: Option<f32>,
    /// Frosted-glass white-tint strength for the blur. `Some(0.0)` disables the
    /// compositor's white overlay so the blur is a faithful `backdrop-filter`
    /// and this surface's own background provides the glass colour; `None` keeps
    /// the compositor default tint. Requires org_kde_kwin_blur version 3.
    pub blur_tint: Option<f32>,
    /// Frosted-glass border strength (alpha) for the blur. `Some(0.0)` disables
    /// the compositor's 1px white border (e.g. when the surface draws its own);
    /// `None` keeps the compositor default. Requires org_kde_kwin_blur version 3.
    pub blur_border: Option<f32>,
    /// Request shadow effect for this surface (requires compositor support for layer_shadow_manager_v1)
    pub shadow: bool,
    /// Corner radius for this surface [top_left, top_right, bottom_right, bottom_left]
    /// (requires compositor support for layer_corner_radius_manager_v1)
    pub corner_radius: Option<[u32; 4]>,
    /// Show/hide transition animation requested when this surface is toggled via
    /// the `layer_surface_visibility` protocol. `None` takes the compositor
    /// default (a left/right edge-anchored surface slides; everything else fades
    /// in/out with a subtle upward slide). `Some(Fade)` keeps that fade+rise on
    /// an edge-anchored surface; `Some(Slide)` forces the full edge slide (the
    /// dock — the only surface that should). Requires
    /// `zcosmic_layer_surface_visibility` version 2.
    pub transition: Option<LayerTransition>,
    /// Map the surface hidden until `ShowWindow`, so its first show plays the
    /// compositor's open animation as every later one does. A surface mapped
    /// visible has no hidden state to open from. Requires
    /// `zcosmic_layer_surface_visibility`.
    pub start_hidden: bool,
    /// Skip the hide animation of that initial hide. For a surface that maps
    /// hidden and stays hidden until summoned; see
    /// [`layershellev::NewLayerShellSettings::start_hidden_without_animating`].
    /// Honoured from `zcosmic_layer_surface_visibility` v4.
    pub start_hidden_without_animating: bool,
    /// Enable foreign toplevel tracking - receive events for all opened windows on the system
    /// (requires compositor support for zwlr_foreign_toplevel_manager_v1)
    /// This field is only available when the `foreign-toplevel` feature is enabled.
    #[cfg(feature = "foreign-toplevel")]
    pub foreign_toplevel: bool,
    /// Auto-size the surface to fit content after first layout.
    /// When true, the initial size is used as a maximum, and the surface will be resized
    /// to match the actual content size after the first render.
    pub auto_size: bool,

    /// Receive the device's special key on this surface
    /// (requires compositor support for zcosmic_special_action_v1)
    pub special_action: bool,

    /// Also act as the fallback receiver, used when no registered surface is focused
    pub special_action_default: bool,
}

impl Default for LayerShellSettings {
    fn default() -> Self {
        LayerShellSettings {
            anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Top,
            exclusive_zone: -1,
            size: None,
            margin: (0, 0, 0, 0),
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            events_transparent: false,
            start_mode: StartMode::default(),
            blur: false,
            blur_radius: None,
            blur_saturation: None,
            blur_tint: None,
            blur_border: None,
            shadow: false,
            corner_radius: None,
            transition: None,
            start_hidden: false,
            start_hidden_without_animating: false,
            #[cfg(feature = "foreign-toplevel")]
            foreign_toplevel: false,
            auto_size: false,
            special_action: false,
            special_action_default: false,
        }
    }
}

impl LayerShellSettings {
    /// Enable foreign toplevel tracking - receive events for all opened windows on the system.
    /// Requires compositor support for `zwlr_foreign_toplevel_manager_v1`.
    /// This method is only available when the `foreign-toplevel` feature is enabled.
    #[cfg(feature = "foreign-toplevel")]
    #[must_use]
    pub fn with_foreign_toplevel(mut self) -> Self {
        self.foreign_toplevel = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default() {
        let settings: Settings = Settings::default();

        assert!(settings.id.is_none());
        assert!(settings.fonts.is_empty());
        assert_eq!(settings.default_font, Font::default());
        assert_eq!(settings.default_text_size, Pixels(16.0));
        assert!(!settings.antialiasing);
        assert!(settings.virtual_keyboard_support.is_none());

        // Test default layershellv settings
        assert_eq!(
            settings.layer_settings.anchor,
            Anchor::Bottom | Anchor::Left | Anchor::Right
        );
        assert_eq!(settings.layer_settings.layer, Layer::Top);
        assert_eq!(settings.layer_settings.exclusive_zone, -1);
        assert_eq!(settings.layer_settings.size, None);
        assert_eq!(settings.layer_settings.margin, (0, 0, 0, 0));
        assert_eq!(
            settings.layer_settings.keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
        assert!(matches!(
            settings.layer_settings.start_mode,
            StartMode::Active
        ));
    }

    #[test]
    fn test_virtual_keyboard_settings() {
        let file = File::open("/dev/null").expect("Failed to open file");
        let keymap_size = 1024;
        let keymap_format = KeymapFormat::XkbV1;

        let virtual_keyboard_settings = VirtualKeyboardSettings {
            file,
            keymap_size,
            keymap_format,
        };

        assert_eq!(virtual_keyboard_settings.keymap_size, 1024);
        assert_eq!(virtual_keyboard_settings.keymap_format, KeymapFormat::XkbV1);
    }

    #[test]
    fn test_layer_shell_settings_custom() {
        let layer_settings = LayerShellSettings {
            anchor: Anchor::Top | Anchor::Left,
            layer: Layer::Background,
            exclusive_zone: 0,
            size: Some((1920, 1080)),
            margin: (10, 10, 10, 10),
            keyboard_interactivity: KeyboardInteractivity::None,
            start_mode: StartMode::TargetScreen("HDMI-1".to_string()),
            events_transparent: false,
            blur: false,
            blur_radius: None,
            blur_saturation: None,
            blur_tint: None,
            blur_border: None,
            shadow: false,
            corner_radius: None,
            transition: None,
            start_hidden: false,
            start_hidden_without_animating: false,
            auto_size: false,
            special_action: false,
            special_action_default: false,
        };

        assert_eq!(layer_settings.anchor, Anchor::Top | Anchor::Left);
        assert_eq!(layer_settings.layer, Layer::Background);
        assert_eq!(layer_settings.exclusive_zone, 0);
        assert_eq!(layer_settings.size, Some((1920, 1080)));
        assert_eq!(layer_settings.margin, (10, 10, 10, 10));
        assert_eq!(
            layer_settings.keyboard_interactivity,
            KeyboardInteractivity::None
        );
        assert_eq!(
            layer_settings.start_mode,
            StartMode::TargetScreen("HDMI-1".to_string())
        );
    }
}
