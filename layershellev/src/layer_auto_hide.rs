//! Client-side implementation of the layer auto-hide protocol (layer_auto_hide_manager_v1)
//!
//! This protocol allows layer shell surfaces to register for compositor-driven
//! auto-hide behavior. The compositor handles all animation and hover detection
//! internally, providing smooth 60fps transitions.

use wayland_client::protocol::wl_surface::WlSurface;

// Re-export generated types
pub use generated::{layer_auto_hide_manager_v1, layer_auto_hide_v1};

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    unused_imports
)]
mod generated {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/layer-auto-hide.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/layer-auto-hide.xml");
}

/// User data for auto-hide objects - stores the surface reference
#[derive(Debug, Clone)]
pub struct LayerAutoHideData {
    pub surface: WlSurface,
}

#[cfg(test)]
mod tests {
    use super::layer_auto_hide_v1::Edge;

    #[test]
    fn every_edge_reaches_the_compositor_as_itself() {
        // An edge missing from the protocol fell back to the bottom one, so a bar
        // on the top or a side hid toward, and was revealed from, the bottom.
        for (value, edge) in [
            (0, Edge::Bottom),
            (1, Edge::Top),
            (2, Edge::Left),
            (3, Edge::Right),
        ] {
            assert_eq!(Edge::try_from(value), Ok(edge));
        }
    }
}
