//! # Handle the layer_shell in a winit way
//!
//! Min example is under
//!
//! ```rust, no_run
//! use std::fs::File;
//! use std::os::fd::AsFd;
//!
//! use layershellev::keyboard::{KeyCode, PhysicalKey};
//! use layershellev::reexport::*;
//! use layershellev::*;
//!
//! fn main() {
//!     let mut ev: WindowState<()> = WindowState::new("Hello")
//!         .with_allscreens()
//!         .with_size((0, 400))
//!         .with_layer(Layer::Top)
//!         .with_margin((20, 20, 100, 20))
//!         .with_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right)
//!         .with_keyboard_interacivity(KeyboardInteractivity::Exclusive)
//!         .with_exclusive_zone(-1)
//!         .build()
//!         .unwrap();
//!
//!     ev.running(|event, ev, index| {
//!         match event {
//!             // NOTE: this will send when init, you can request bind extra object from here
//!             LayerShellEvent::InitRequest => ReturnData::RequestBind,
//!             LayerShellEvent::BindProvide(globals, qh) => {
//!                 // NOTE: you can get implied wayland object from here
//!                 let virtual_keyboard_manager = globals
//!                         .bind::<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardManagerV1, _, _>(
//!                             qh,
//!                             1..=1,
//!                             (),
//!                         )
//!                         .unwrap();
//!                 println!("{:?}", virtual_keyboard_manager);
//!                 ReturnData::None
//!             }
//!             LayerShellEvent::XdgInfoChanged(_) => {
//!                 let index = index.unwrap();
//!                 let unit = ev.get_unit_with_id(index).unwrap();
//!                 println!("{:?}", unit.get_xdgoutput_info());
//!                 ReturnData::None
//!             }
//!             LayerShellEvent::RequestBuffer(file, shm, qh, init_w, init_h) => {
//!                 draw(file, (init_w, init_h));
//!                 let pool = shm.create_pool(file.as_fd(), (init_w * init_h * 4) as i32, qh, ());
//!                 ReturnData::WlBuffer(pool.create_buffer(
//!                     0,
//!                     init_w as i32,
//!                     init_h as i32,
//!                     (init_w * 4) as i32,
//!                     wl_shm::Format::Argb8888,
//!                     qh,
//!                     (),
//!                 ))
//!             }
//!             LayerShellEvent::RequestMessages(DispatchMessage::RequestRefresh { width, height, .. }) => {
//!                 println!("{width}, {height}");
//!                 ReturnData::None
//!             }
//!             LayerShellEvent::RequestMessages(DispatchMessage::MouseButton { .. }) => ReturnData::None,
//!             LayerShellEvent::RequestMessages(DispatchMessage::MouseEnter {
//!                 pointer, ..
//!             }) => ReturnData::RequestSetCursorShape((
//!                 "crosshair".to_owned(),
//!                 pointer.clone(),
//!             )),
//!             LayerShellEvent::RequestMessages(DispatchMessage::MouseMotion {
//!                 time,
//!                 surface_x,
//!                 surface_y,
//!             }) => {
//!                 println!("{time}, {surface_x}, {surface_y}");
//!                 ReturnData::None
//!             }
//!             LayerShellEvent::RequestMessages(DispatchMessage::KeyboardInput { event, .. }) => {
//!                if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
//!                    ReturnData::RequestExit
//!                } else {
//!                    ReturnData::None
//!                }
//!            }
//!             _ => ReturnData::None,
//!         }
//!     })
//!     .unwrap();
//! }
//!
//! fn draw(tmp: &mut File, (buf_x, buf_y): (u32, u32)) {
//!     use std::{cmp::min, io::Write};
//!     let mut buf = std::io::BufWriter::new(tmp);
//!     for y in 0..buf_y {
//!         for x in 0..buf_x {
//!             let a = 0xFF;
//!             let r = min(((buf_x - x) * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
//!             let g = min((x * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
//!             let b = min(((buf_x - x) * 0xFF) / buf_x, (y * 0xFF) / buf_y);
//!
//!             let color = (a << 24) + (r << 16) + (g << 8) + b;
//!             buf.write_all(&color.to_ne_bytes()).unwrap();
//!         }
//!     }
//!     buf.flush().unwrap();
//! }
//! ```
//!
use calloop::channel::Channel;
pub use events::LayerTransition;
pub use events::NewInputPanelSettings;
pub use events::NewLayerShellSettings;
pub use events::NewPopUpSettings;
pub use events::NewXdgWindowSettings;
pub use events::OutputOption;
pub use events::RepositionPopUpSettings;
pub use waycrate_xkbkeycode::keyboard;
pub use waycrate_xkbkeycode::xkb_keyboard;

mod activation_token;
pub use activation_token::ActivationTokenReply;
pub mod adaptive_foreground;
pub mod background_effect;
pub mod blur;
pub mod corner_radius;
pub mod dpi;
mod events;
#[cfg(feature = "workspaces")]
pub mod ext_workspace;
#[cfg(feature = "foreign-toplevel")]
pub mod foreign_toplevel;
#[cfg(all(feature = "screencopy", feature = "workspaces"))]
pub mod kora_image_capture_size;
pub mod layer_auto_hide;
pub mod layer_edge_resize;
mod surrounding_text;
pub use surrounding_text::ImeSurroundingText;
use surrounding_text::{SurroundingChange, SurroundingText};
pub mod capture_exclusion;
pub mod layer_size_transition;
pub mod layer_surface_dismiss;
pub mod layer_surface_placement;
pub mod layer_surface_visibility;
pub mod layer_usable_area;
#[cfg(feature = "screencopy")]
pub mod screencopy;
pub mod shadow;
pub mod special_action;
mod strtoshape;
pub mod tooltip;
#[cfg(feature = "cosmic-toplevel")]
mod toplevel_identity;
pub mod workspace_transition;

use events::DispatchMessageInner;

pub mod id;

pub use events::{
    AxisScroll, DispatchMessage, Ime, LayerShellEvent, OutputLayoutItem, ReturnData,
    XdgInfoChangedType,
};

use strtoshape::str_to_shape;

use waycrate_xkbkeycode::xkb_keyboard::ElementState;
use waycrate_xkbkeycode::xkb_keyboard::RepeatInfo;

use wayland_client::{
    ConnectError, Connection, Dispatch, DispatchError, EventQueue, Proxy, QueueHandle, WEnum,
    backend::ObjectId,
    delegate_noop, event_created_child,
    globals::{BindError, GlobalError, GlobalList, GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::WlBuffer,
        wl_callback::{Event as WlCallbackEvent, WlCallback},
        wl_compositor::WlCompositor,
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::{DndAction, WlDataDeviceManager},
        wl_data_offer::{self, WlDataOffer},
        wl_data_source::{self, WlDataSource},
        wl_display::WlDisplay,
        wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard},
        wl_output::{self, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_region::WlRegion,
        wl_registry,
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::{self, WlSurface},
        wl_touch::{self, WlTouch},
    },
};

use wayland_cursor::{CursorImageBuffer, CursorTheme};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};

use wayland_protocols::xdg::shell::client::{
    xdg_popup::{self, XdgPopup},
    xdg_positioner::XdgPositioner,
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::XdgWmBase,
};

use wayland_protocols::{
    wp::fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::{self, WpFractionalScaleV1},
    },
    xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
};

// For `ScreencopyHandler::get_ext_workspace_handle`.
#[cfg(all(feature = "screencopy", feature = "workspaces"))]
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1;

use wayland_protocols::wp::input_method::zv1::client::{
    zwp_input_panel_surface_v1::{Position as ZwpInputPanelPosition, ZwpInputPanelSurfaceV1},
    zwp_input_panel_v1::ZwpInputPanelV1,
};

use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1,
    zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1,
};

use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};

use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};

use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
};
use wayland_protocols::xdg::activation::v1::client::{
    xdg_activation_token_v1::{self, XdgActivationTokenV1},
    xdg_activation_v1::XdgActivationV1,
};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
    zxdg_toplevel_decoration_v1::{self, ZxdgToplevelDecorationV1},
};

pub use calloop;
use calloop::{
    Error as CallLoopError, EventLoop, LoopHandle, RegistrationToken, channel,
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum LayerEventError {
    #[error("connect error")]
    ConnectError(#[from] ConnectError),
    #[error("Global Error")]
    GlobalError(#[from] GlobalError),
    #[error("Bind Error")]
    BindError(#[from] BindError),
    #[error("Error during queue")]
    DispatchError(#[from] DispatchError),
    #[error("create file failed")]
    TempFileCreateFailed(#[from] std::io::Error),
    #[error("Event Loop Error")]
    EventLoopInitError(#[from] CallLoopError),
}

pub mod reexport {
    pub use wayland_protocols_wlr::layer_shell::v1::client::{
        zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
        zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity},
    };
    pub mod wl_shm {
        pub use wayland_client::protocol::wl_shm::Format;
        pub use wayland_client::protocol::wl_shm::WlShm;
    }
    pub mod zwp_virtual_keyboard_v1 {
        pub use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
            zwp_virtual_keyboard_manager_v1::{self, ZwpVirtualKeyboardManagerV1},
            zwp_virtual_keyboard_v1::{self, ZwpVirtualKeyboardV1},
        };
    }
    pub mod wp_fractional_scale_v1 {
        pub use wayland_protocols::wp::fractional_scale::v1::client::{
            wp_fractional_scale_manager_v1::{self, WpFractionalScaleManagerV1},
            wp_fractional_scale_v1::{self, WpFractionalScaleV1},
        };
    }
    pub mod wayland_client {
        pub use wayland_client::{
            Connection, QueueHandle, WEnum,
            globals::GlobalList,
            protocol::{
                wl_compositor::WlCompositor,
                wl_keyboard::{self, KeyState},
                wl_pointer::{self, ButtonState},
                wl_region::WlRegion,
                wl_seat::WlSeat,
            },
        };
    }
    pub mod wp_cursor_shape_device_v1 {
        pub use crate::strtoshape::ShapeName;
        pub use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
    }
    pub mod xdg_toplevel {
        pub use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
    }
    pub mod wp_viewport {
        pub use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
    }
}

#[derive(Debug)]
struct BaseState;

// so interesting, it is just need to invoke once, it just used to get the globals
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for BaseState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

/// this struct store the xdg_output information
#[derive(Debug, Clone)]
pub struct ZxdgOutputInfo {
    name: String,
    description: String,
    zxdgoutput: ZxdgOutputV1,
    logical_size: (i32, i32),
    position: (i32, i32),
}

impl ZxdgOutputInfo {
    fn new(zxdgoutput: ZxdgOutputV1) -> Self {
        Self {
            zxdgoutput,
            name: "".to_owned(),
            description: "".to_owned(),
            logical_size: (0, 0),
            position: (0, 0),
        }
    }

    /// you can get the Logic position of the screen current surface in
    pub fn get_position(&self) -> (i32, i32) {
        self.position
    }

    /// you can get the LogicalPosition of the screen current surface in
    pub fn get_logical_size(&self) -> (i32, i32) {
        self.logical_size
    }
}

/// This is the unit, binding to per screen.
/// Because layer_shell is so unique, on surface bind to only one
/// wl_output, only one buffer, only one output, so it will store
/// includes the information of ZxdgOutput, size, and layer_shell
///
/// and it can set a binding, you to store the related data. like
/// a cario_context, which is binding to the buffer on the wl_surface.
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
enum Shell {
    LayerShell(ZwlrLayerSurfaceV1),
    PopUp((XdgPopup, XdgSurface)),
    XdgTopLevel((XdgToplevel, XdgSurface, Option<ZxdgToplevelDecorationV1>)),
    InputPanel(#[allow(unused)] ZwpInputPanelSurfaceV1),
}

impl PartialEq<ZwlrLayerSurfaceV1> for Shell {
    fn eq(&self, other: &ZwlrLayerSurfaceV1) -> bool {
        match self {
            Self::LayerShell(shell) => shell == other,
            _ => false,
        }
    }
}

impl PartialEq<XdgPopup> for Shell {
    fn eq(&self, other: &XdgPopup) -> bool {
        match self {
            Self::PopUp((popup, _)) => popup == other,
            _ => false,
        }
    }
}

impl PartialEq<XdgSurface> for Shell {
    fn eq(&self, other: &XdgSurface) -> bool {
        match self {
            Self::PopUp((_, surface)) | Self::XdgTopLevel((_, surface, _)) => surface == other,
            _ => false,
        }
    }
}
impl PartialEq<XdgToplevel> for Shell {
    fn eq(&self, other: &XdgToplevel) -> bool {
        match self {
            Self::XdgTopLevel((level, _, _)) => level == other,
            _ => false,
        }
    }
}
impl Shell {
    fn destroy(&self) {
        match self {
            Self::PopUp((popup, xdg_surface)) => {
                popup.destroy();
                xdg_surface.destroy();
            }
            Self::XdgTopLevel((top_level, xdg_surface, decoration)) => {
                if let Some(decoration) = decoration {
                    decoration.destroy();
                }
                top_level.destroy();
                xdg_surface.destroy();
            }
            Self::LayerShell(shell) => shell.destroy(),
            Self::InputPanel(_) => {}
        }
    }

    fn is_popup(&self) -> bool {
        matches!(self, Self::PopUp(_))
    }

    fn top_level(&self) -> Option<XdgToplevel> {
        match self {
            Self::XdgTopLevel((level, _, _)) => Some(level.clone()),
            _ => None,
        }
    }
}

/// The state of if we can call a `present` for the window.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum PresentAvailableState {
    /// A `wl_surface.frame` request has been sent, and there is no callback yet.
    Requested,
    /// A notification has been received, it is a good time to start drawing a new frame. Because
    /// there is no present at first, so the default state is available.
    #[default]
    Available,
    /// Availability is taken.
    Taken,
}

struct WindowStateUnitBuilder<T> {
    inner: WindowStateUnit<T>,
}

impl<T> WindowStateUnitBuilder<T> {
    fn new(
        id: id::Id,
        qh: QueueHandle<WindowState<T>>,
        display: WlDisplay,
        wl_surface: WlSurface,
        shell: Shell,
    ) -> Self {
        Self {
            inner: WindowStateUnit {
                id,
                qh,
                display,
                wl_surface,
                // An input panel surface gets no configure.
                configured: matches!(shell, Shell::InputPanel(_)),
                shell,
                size: (0, 0),
                buffer: Default::default(),
                zxdgoutput: Default::default(),
                fractional_scale: Default::default(),
                viewport: Default::default(),
                wl_output: Default::default(),
                binding: Default::default(),
                becreated: Default::default(),
                initial_refresh_sent: false,
                unanswered_sizes: std::sync::atomic::AtomicU32::new(0),
                popup_placement: None,
                fade_close: None,
                // Unknown why it is 120
                scale: 120,
                request_flag: Default::default(),
                present_available_state: Default::default(),
            },
        }
    }

    fn build(self) -> WindowStateUnit<T> {
        self.inner
    }

    fn size(mut self, size: (u32, u32)) -> Self {
        self.inner.size = size;
        self
    }

    fn zxdgoutput(mut self, zxdgoutput: Option<ZxdgOutputInfo>) -> Self {
        self.inner.zxdgoutput = zxdgoutput;
        self
    }

    fn fractional_scale(mut self, fractional_scale: Option<WpFractionalScaleV1>) -> Self {
        self.inner.fractional_scale = fractional_scale;
        self
    }

    fn viewport(mut self, viewport: Option<WpViewport>) -> Self {
        self.inner.viewport = viewport;
        self
    }

    fn wl_output(mut self, wl_output: Option<WlOutput>) -> Self {
        self.inner.wl_output = wl_output;
        self
    }

    fn binding(mut self, binding: Option<T>) -> Self {
        self.inner.binding = binding;
        self
    }

    fn becreated(mut self, becreated: bool) -> Self {
        self.inner.becreated = becreated;
        self
    }

    fn popup_placement(mut self, placement: PopupPlacement) -> Self {
        self.inner.popup_placement = Some(placement);
        self
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefreshRequest {
    /// Redraw the next frame.
    NextFrame,

    /// Redraw at the given time.
    At(Instant),

    /// No redraw is needed.
    #[default]
    Wait,
}

#[derive(Debug, Default)]
struct WindowStateUnitRequestFlag {
    /// The flag of if this window has been requested to be closed.
    close: bool,
    /// The flag of if this window has been requested to be refreshed.
    refresh: RefreshRequest,
}

/// Where a popup sits against its parent, kept so it can be resized: xdg-shell takes a
/// popup's size only through a positioner.
#[derive(Debug, Clone)]
struct PopupPlacement {
    /// The surface it opens from.
    parent: id::Id,
    /// Whether it holds the keyboard and pointer grab.
    grab: bool,
    wm_base: XdgWmBase,
    anchor_rect: (i32, i32, i32, i32),
    anchor: u32,
    gravity: u32,
    constraint_adjustment: u32,
    offset: Option<(i32, i32)>,
    reactive: bool,
}

impl PopupPlacement {
    /// A positioner for a `width` x `height` popup placed this way. Destroy it once used.
    fn positioner<T: 'static>(
        &self,
        qh: &QueueHandle<WindowState<T>>,
        (width, height): (u32, u32),
    ) -> XdgPositioner {
        use wayland_protocols::xdg::shell::client::xdg_positioner::{
            Anchor, ConstraintAdjustment, Gravity,
        };
        let positioner = self.wm_base.create_positioner(qh, ());
        positioner.set_size(width as i32, height as i32);
        let (x, y, w, h) = self.anchor_rect;
        positioner.set_anchor_rect(x, y, w, h);
        if self.anchor != 0 {
            positioner.set_anchor(Anchor::try_from(self.anchor).unwrap_or(Anchor::None));
        }
        if self.gravity != 0 {
            positioner.set_gravity(Gravity::try_from(self.gravity).unwrap_or(Gravity::None));
        }
        if self.constraint_adjustment != 0 {
            positioner.set_constraint_adjustment(ConstraintAdjustment::from_bits_truncate(
                self.constraint_adjustment,
            ));
        }
        if let Some((x, y)) = self.offset {
            positioner.set_offset(x, y);
        }
        if self.reactive {
            positioner.set_reactive();
        }
        positioner
    }
}

/// A fresh token for `xdg_popup.reposition`.
fn next_reposition_token() -> u32 {
    static TOKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug)]
pub struct WindowStateUnit<T> {
    id: id::Id,
    qh: QueueHandle<WindowState<T>>,
    display: WlDisplay,
    wl_surface: WlSurface,
    size: (u32, u32),
    buffer: Option<WlBuffer>,
    shell: Shell,
    zxdgoutput: Option<ZxdgOutputInfo>,
    fractional_scale: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
    wl_output: Option<WlOutput>,
    binding: Option<T>,
    becreated: bool,
    /// Whether this unit has had its initial Refresh event dispatched.
    /// Surfaces created with `start_hidden` need at least one Refresh
    /// so the iced window_manager registers them (even when all
    /// surfaces are hidden and the normal refresh cycle is skipped).
    initial_refresh_sent: bool,
    /// Whether the compositor has sent the first configure. A buffer committed before it is a
    /// protocol error, so nothing presents until then.
    configured: bool,
    /// Size requests whose answer is still on its way (see [`WindowStateUnit::size_answered`]).
    unanswered_sizes: std::sync::atomic::AtomicU32,
    /// A popup's placement, for resizing it.
    popup_placement: Option<PopupPlacement>,
    /// When a popup fading out on hide is destroyed. Nothing is drawn meanwhile: the
    /// compositor fades the last frame committed.
    fade_close: Option<Instant>,

    scale: u32,
    request_flag: WindowStateUnitRequestFlag,
    present_available_state: PresentAvailableState,
}

impl<T> WindowStateUnit<T> {
    /// Whether this surface is an xdg popup.
    pub fn is_popup(&self) -> bool {
        self.shell.is_popup()
    }
}

impl<T: 'static> WindowStateUnit<T> {
    /// set the layer size of current unit
    ///
    /// A popup is placed again at the new size, flipped or slid as its positioner
    /// allows, once the compositor's configure arrives. That needs `xdg_wm_base` v3.
    pub fn set_size(&self, (width, height): (u32, u32)) {
        match &self.shell {
            Shell::LayerShell(layer_shell) => {
                layer_shell.set_size(width, height);
                self.wl_surface.commit();
                // A compositor that keeps the size it gave sends no configure, so ask
                // when it has read the request; any configure comes before that answer.
                self.unanswered_sizes
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.display.sync(&self.qh, SizeAnswer(self.id));
            }
            Shell::PopUp((popup, xdg_surface)) => {
                let Some(placement) = &self.popup_placement else {
                    return;
                };
                if popup.version() < 3 {
                    log::warn!(
                        "xdg_popup.reposition needs xdg_wm_base v3, the popup keeps its size"
                    );
                    return;
                }
                let positioner = placement.positioner(&self.qh, (width, height));
                popup.reposition(&positioner, next_reposition_token());
                positioner.destroy();
                // Both apply with the next commit, the frame drawn at the new size.
                xdg_surface.set_window_geometry(0, 0, width as i32, height as i32);
                self.try_set_viewport_destination(width as i32, height as i32);
            }
            _ => {}
        }
    }
}

impl<T> WindowStateUnit<T> {
    /// get the WindowState id
    pub fn id(&self) -> id::Id {
        self.id
    }

    pub fn try_set_viewport_destination(&self, width: i32, height: i32) -> Option<()> {
        let viewport = self.viewport.as_ref()?;
        viewport.set_destination(width, height);
        Some(())
    }

    pub fn try_set_viewport_source(&self, x: f64, y: f64, width: f64, height: f64) -> Option<()> {
        let viewport = self.viewport.as_ref()?;
        viewport.set_source(x, y, width, height);
        Some(())
    }

    /// gen the WindowState [WindowWrapper]
    pub fn gen_wrapper(&self) -> WindowWrapper {
        WindowWrapper {
            id: self.id,
            display: self.display.clone(),
            wl_surface: self.wl_surface.clone(),
            viewport: self.viewport.clone(),
            toplevel: self.shell.top_level(),
        }
    }
}
impl<T> WindowStateUnit<T> {
    #[inline]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandWindowHandle::new({
            let ptr = self.wl_surface.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        })
        .into())
    }

    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        })
        .into())
    }
}

impl<T> rwh_06::HasWindowHandle for WindowStateUnit<T> {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::WindowHandle::borrow_raw(raw) })
    }
}

impl<T> rwh_06::HasDisplayHandle for WindowStateUnit<T> {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}

// if is only one window, use it will be easy
impl<T> rwh_06::HasWindowHandle for WindowState<T> {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.main_window().raw_window_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::WindowHandle::borrow_raw(raw) })
    }
}

// if is only one window, use it will be easy
impl<T> rwh_06::HasDisplayHandle for WindowState<T> {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.main_window().raw_display_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}
impl<T> WindowStateUnit<T> {
    /// get the wl surface from WindowState
    /// The queue handle this surface was created with.
    ///
    /// Exposed so callers can create short-lived protocol objects — a
    /// `wl_region` per request, rather than sharing one across every surface.
    pub fn qh(&self) -> &QueueHandle<WindowState<T>> {
        &self.qh
    }

    pub fn get_wlsurface(&self) -> &WlSurface {
        &self.wl_surface
    }

    /// Ask for this window to be fullscreen on its output, or no longer.
    /// Only an xdg toplevel can be; any other surface ignores it.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        let Some(toplevel) = self.shell.top_level() else {
            return;
        };
        if fullscreen {
            toplevel.set_fullscreen(None);
        } else {
            toplevel.unset_fullscreen();
        }
    }

    /// get the xdg_output info related to this unit
    pub fn get_xdgoutput_info(&self) -> Option<&ZxdgOutputInfo> {
        self.zxdgoutput.as_ref()
    }

    /// Whether the compositor has answered every size this surface asked for, with a
    /// configure or by keeping the size it gave. [`Self::get_size`] is then final.
    pub fn size_answered(&self) -> bool {
        self.unanswered_sizes
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
    }

    /// Its output's size and place as the message `zxdg_output` sends, once known.
    fn output_info(&self) -> Option<DispatchMessage> {
        let info = self.zxdgoutput.as_ref()?;
        let (width, height) = info.logical_size;
        (width > 0 && height > 0).then(|| DispatchMessage::XdgInfoChanged {
            width,
            height,
            output_name: info.name.clone(),
            output_x: info.position.0,
            output_y: info.position.1,
        })
    }

    /// set the anchor of the current unit. please take the simple.rs as reference
    pub fn set_anchor(&self, anchor: Anchor) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_anchor(anchor);
            self.wl_surface.commit();
        }
    }

    /// you can reset the margin which bind to the surface
    pub fn set_margin(&self, (top, right, bottom, left): (i32, i32, i32, i32)) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_margin(top, right, bottom, left);
            self.wl_surface.commit();
        }
    }

    /// set the layer
    pub fn set_layer(&self, layer: Layer) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_layer(layer);
            self.wl_surface.commit();
        }
    }

    /// set the anchor and set the size together
    /// When you want to change layer from LEFT|RIGHT|BOTTOM to TOP|LEFT|BOTTOM, use it
    pub fn set_anchor_with_size(&self, anchor: Anchor, (width, height): (u32, u32)) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_anchor(anchor);
            layer_shell.set_size(width, height);
            self.wl_surface.commit();
        }
    }

    /// set current exclusive_zone
    pub fn set_exclusive_zone(&self, zone: i32) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_exclusive_zone(zone);
            self.wl_surface.commit();
        }
    }

    /// Set the edge the exclusive zone is taken from, which must be one of the surface's
    /// anchors, or none (an empty `Anchor`) to let the anchors imply it. A surface anchored to
    /// a corner reserves nothing without one. Needs layer shell version 5; older compositors
    /// keep the implied edge.
    pub fn set_exclusive_edge(&self, edge: Anchor) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            if layer_shell.version() < 5 {
                return;
            }
            layer_shell.set_exclusive_edge(edge);
            self.wl_surface.commit();
        }
    }

    /// set keyboard interactivity for the layer surface
    pub fn set_keyboard_interactivity(
        &self,
        interactivity: zwlr_layer_surface_v1::KeyboardInteractivity,
    ) {
        if let Shell::LayerShell(layer_shell) = &self.shell {
            layer_shell.set_keyboard_interactivity(interactivity);
            self.wl_surface.commit();
        }
    }

    /// you can use this function to set a binding data. the message passed back contain
    /// a index, you can use that to get the unit. It will be very useful, because you can
    /// use the binding data to operate the file binding to the buffer. you can take
    /// startcolorkeyboard as reference.
    pub fn set_binding(&mut self, binding: T) {
        self.binding = Some(binding);
    }

    /// return the binding data, with mut reference
    pub fn get_binding_mut(&mut self) -> Option<&mut T> {
        self.binding.as_mut()
    }

    /// get the binding data
    pub fn get_binding(&self) -> Option<&T> {
        self.binding.as_ref()
    }

    /// get the size of the surface
    pub fn get_size(&self) -> (u32, u32) {
        self.size
    }

    /// this function will refresh whole surface. it will reattach the buffer, and damage whole,
    /// and final commit
    pub fn refresh(&self) {
        self.wl_surface.attach(self.buffer.as_ref(), 0, 0);
        self.wl_surface
            .damage(0, 0, self.size.0 as i32, self.size.1 as i32);
        self.wl_surface.commit();
    }

    pub fn scale_u32(&self) -> u32 {
        self.scale
    }

    pub fn scale_float(&self) -> f64 {
        self.scale as f64 / 120.
    }

    pub fn request_close(&mut self) {
        self.request_flag.close = true;
    }

    pub fn request_refresh(&mut self, request: RefreshRequest) {
        // refresh request in nearest future has the highest priority.
        match self.request_flag.refresh {
            RefreshRequest::NextFrame => {}
            RefreshRequest::At(instant) => match request {
                RefreshRequest::NextFrame => self.request_flag.refresh = request,
                RefreshRequest::At(other_instant) => {
                    if other_instant < instant {
                        self.request_flag.refresh = request;
                    }
                }
                RefreshRequest::Wait => {}
            },
            RefreshRequest::Wait => self.request_flag.refresh = request,
        }
    }

    /// The redraw this surface is waiting for, if any.
    pub fn refresh_request(&self) -> RefreshRequest {
        self.request_flag.refresh
    }

    fn should_refresh(&self) -> bool {
        match self.request_flag.refresh {
            RefreshRequest::NextFrame => true,
            RefreshRequest::At(instant) => instant <= Instant::now(),
            RefreshRequest::Wait => false,
        }
    }

    pub fn take_present_slot(&mut self) -> bool {
        if !self.configured || self.fade_close.is_some() {
            return false;
        }
        if !self.should_refresh() {
            return false;
        }
        if self.present_available_state != PresentAvailableState::Available {
            return false;
        }
        self.request_flag.refresh = RefreshRequest::Wait;
        self.present_available_state = PresentAvailableState::Taken;
        true
    }

    pub fn reset_present_slot(&mut self) -> bool {
        if self.present_available_state == PresentAvailableState::Taken {
            self.present_available_state = PresentAvailableState::Available;
            true
        } else {
            false
        }
    }
}

impl<T: 'static> WindowStateUnit<T> {
    pub fn request_next_present(&mut self) {
        match self.present_available_state {
            PresentAvailableState::Taken => {
                self.present_available_state = PresentAvailableState::Requested;
                self.wl_surface
                    .frame(&self.qh, (self.id, PresentAvailableState::Available));
            }
            PresentAvailableState::Requested | PresentAvailableState::Available => {}
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ImePurpose {
    /// No special hints for the IME (default).
    Normal,
    /// The IME is used for password input.
    Password,
    /// The IME is used to input into a terminal.
    ///
    /// For example, that could alter OSK on Wayland to show extra buttons.
    Terminal,
}

#[derive(Debug)]
struct KeyboardTokenState {
    delay: Duration,
    key: u32,
    surface_id: Option<id::Id>,
    pressed_state: ElementState,
}

#[derive(Debug)]
pub struct VirtualKeyRelease {
    pub delay: Duration,
    pub time: u32,
    pub key: u32,
}

/// Per-surface frosted-glass blur params `(radius, saturation, tint, border)`,
/// each optional (None = compositor default). Captured at surface creation so a
/// later blur re-enable can re-apply them instead of the compositor defaults.
type BlurParams = (Option<f32>, Option<f32>, Option<f32>, Option<f32>);

/// The MIME type carrying a `\r\n`-separated list of `file://` URIs in a
/// drag-and-drop offer — what file managers use to advertise dragged files.
const URI_LIST_MIME: &str = "text/uri-list";

/// How long to wait for a source to write its URI list to the pipe we hand it.
/// A cooperating source writes immediately; this only bounds a misbehaving one,
/// and the read happens on the event-loop thread.
const URI_LIST_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// The drag-and-drop offer currently hovering one of our surfaces.
#[derive(Debug, Clone)]
struct DndCurrent {
    /// The hovering offer (destroyed on leave, or after a completed drop).
    offer: WlDataOffer,
    /// The surface the offer is over (for routing the drop to the right window).
    surface_id: Option<id::Id>,
    /// Whether the offer advertises [`URI_LIST_MIME`] (i.e. is droppable here).
    has_uri_list: bool,
    /// The action the compositor has settled on for this drag, from
    /// `wl_data_offer.action`. Empty until one is negotiated — and it stays
    /// empty if none ever is, because that event is only sent when the action
    /// CHANGES and it starts out empty. See [`DndCurrent::finish`].
    action: DndAction,
}

impl DndCurrent {
    /// Tell the source we took the data — if we are allowed to say so.
    ///
    /// `wl_data_offer.finish` is not merely ignored when it is out of turn: the
    /// compositor answers it with the `invalid_finish` protocol error, which
    /// KILLS this process. Two things have to hold, and neither is ours to
    /// assume:
    ///
    /// * **version ≥ 3.** `finish` does not exist below it.
    /// * **an action was negotiated.** We ask for `Copy` only, and the
    ///   compositor intersects that with what the source offers — so a source
    ///   advertising `Move` alone leaves the intersection empty, and smithay
    ///   posts `invalid_finish` on any `finish` in that state ("Cannot finish a
    ///   data offer with no valid action"). A source that dies mid-drag lands in
    ///   the same place.
    ///
    /// Declining to finish is safe: the `destroy` that follows is what tells the
    /// source the drop was not taken.
    fn finish(&self) {
        if may_finish(self.offer.version(), self.action) {
            self.offer.finish();
        }
    }
}

/// Whether `wl_data_offer.finish` is legal on an offer at this version with this
/// negotiated action.
///
/// Split out from [`DndCurrent::finish`] because the answer cannot be tested
/// through it — that needs a live `WlDataOffer`, and getting it wrong does not
/// misbehave, it kills the process.
fn may_finish(version: u32, action: DndAction) -> bool {
    version >= 3 && !action.is_empty()
}

/// Pre-serialized payload attached to a `wl_data_source` we start, so the
/// `Send` handler can write the requested MIME data to the destination app over
/// the fd without needing any external state.
#[derive(Debug, Clone, Default)]
struct DndSourceData {
    mime_types: Vec<String>,
    data: Vec<Vec<u8>>,
}

/// Pre-multiplied ARGB (`Argb8888`, little-endian) pixels for an outgoing drag
/// icon, with size and buffer scale.
pub struct DndIconPixels {
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub argb: Vec<u8>,
}

/// Live wl_surface + SHM buffer backing an outgoing drag icon. Kept alive for
/// the duration of the drag and destroyed (surface first) when it ends.
#[derive(Debug)]
struct DndIconResources {
    surface: WlSurface,
    buffer: WlBuffer,
    pool: WlShmPool,
    _file: std::fs::File,
    width: i32,
    height: i32,
}

impl DndIconResources {
    fn destroy(self) {
        self.surface.destroy();
        self.buffer.destroy();
        self.pool.destroy();
    }
}

/// main state, store the main information
#[derive(Debug)]
/// Accumulator for one `wl_pointer` axis frame. On v5+ the axis, axis_discrete,
/// axis_source and axis_stop sub-events arrive as separate events grouped by a
/// `frame`. We combine them here and emit a single axis message on `frame`, so
/// `discrete` and `absolute` land together — mirroring winit/SCTK, which lets a
/// discrete wheel notch surface as a `LineDelta` instead of a continuous
/// `PixelDelta`. (The Wayland spec sends axis_discrete/value120 *before* the
/// coupled axis event, so combining per-frame is the only order-safe approach.)
#[derive(Default)]
struct PendingAxisFrame {
    /// Whether any scroll-bearing sub-event (axis / discrete / stop) accumulated.
    active: bool,
    surface_id: Option<id::Id>,
    scale: f64,
    time: u32,
    horizontal: AxisScroll,
    vertical: AxisScroll,
}

pub struct WindowState<T> {
    outputs: Vec<(u32, wl_output::WlOutput)>,
    current_surface: Option<WlSurface>,
    active_surfaces: HashMap<Option<i32>, (WlSurface, Option<id::Id>)>,
    units: Vec<WindowStateUnit<T>>,
    message: Vec<(Option<id::Id>, DispatchMessageInner)>,
    /// In-progress pointer axis frame, flushed on `wl_pointer::Event::Frame`.
    pending_axis: PendingAxisFrame,
    /// Axis source (wheel vs touchpad) of the current scroll sequence. The
    /// compositor sends `axis_source` only once per sequence, so it is persisted
    /// across frames and used to classify every frame's scroll as discrete
    /// (wheel → lines) or continuous (touchpad → pixels) — the per-frame
    /// presence of `axis_discrete` is not reliable (e.g. under a reduced
    /// scroll-speed setting the compositor drops it on some notches).
    scroll_axis_source: Option<wl_pointer::AxisSource>,

    connection: Option<Connection>,
    event_queue: Option<EventQueue<WindowState<T>>>,
    wl_compositor: Option<WlCompositor>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
    wmbase: Option<XdgWmBase>,
    shm: Option<WlShm>,
    cursor_manager: Option<WpCursorShapeManagerV1>,
    viewporter: Option<WpViewporter>,
    fractional_scale_manager: Option<WpFractionalScaleManagerV1>,
    globals: Option<GlobalList>,

    // background
    background_surface: Option<WlSurface>,
    display: Option<WlDisplay>,

    // base managers
    seat: Option<WlSeat>,
    keyboard_state: Option<xkb_keyboard::KeyboardState>,

    pointer: Option<WlPointer>,
    touch: Option<WlTouch>,
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,

    // drag-and-drop (receive only): the data device + the offered MIME types per
    // live `wl_data_offer`, and the currently-hovering drag offer (if any).
    data_device_manager: Option<WlDataDeviceManager>,
    data_device: Option<WlDataDevice>,
    /// MIME types advertised by each live `wl_data_offer`, keyed by its object id.
    dnd_offer_mimes: HashMap<ObjectId, Vec<String>>,
    /// The drag offer currently hovering a surface, plus the surface it's over.
    dnd_current: Option<DndCurrent>,
    /// The clipboard selection offer and the MIME types it advertises. Held
    /// rather than released on arrival so a paste can read `text/uri-list` off
    /// it on demand — see [`WindowState::read_selection_uri_list`].
    selection_offer: Option<(WlDataOffer, Vec<String>)>,
    /// Surface that initiated an outgoing drag — routes source-side events.
    dnd_source_origin: Option<id::Id>,
    /// Serial of the most recent pointer button press (required by `start_drag`).
    last_button_serial: Option<u32>,
    /// Serial of the latest user input (a pointer button, key or touch): what
    /// an activation token is asked against.
    last_input_serial: Option<u32>,
    /// Live drag-icon resources, kept alive for the duration of an outgoing drag.
    dnd_icon: Option<DndIconResources>,
    /// Compositor + shm cached at loop start (the originals are taken by the loop)
    /// so `start_drag` can build the drag-icon surface.
    cached_compositor: Option<WlCompositor>,
    cached_shm: Option<WlShm>,

    // states
    namespace: String,
    keyboard_interactivity: zwlr_layer_surface_v1::KeyboardInteractivity,
    anchor: Anchor,
    layer: Layer,
    size: Option<(u32, u32)>,
    exclusive_zone: Option<i32>,
    margin: Option<(i32, i32, i32, i32)>,

    // settings
    use_display_handle: bool,
    repeat_delay: Option<KeyboardTokenState>,
    to_remove_tokens: Vec<RegistrationToken>,
    closed_ids: Vec<id::Id>,

    to_be_released_key: Option<VirtualKeyRelease>,

    last_unit_index: usize,
    last_wloutput: Option<WlOutput>,

    return_data: Vec<ReturnData<T>>,
    finger_locations: HashMap<i32, (f64, f64)>,
    enter_serial: Option<u32>,

    xdg_info_cache: Vec<(wl_output::WlOutput, ZxdgOutputInfo)>,
    /// Every live output's xdg_output, bound for as long as the output exists.
    /// `output_layout` and `output_handles` are derived from it, so both follow
    /// monitors added or removed after startup.
    output_watch: Vec<(wl_output::WlOutput, ZxdgOutputInfo)>,
    /// Logical layout of every output (global coords), kept current on hotplug.
    /// Exposed via [`WindowState::output_layout`] for cross-monitor positioning.
    output_layout: Vec<OutputLayoutItem>,
    /// Maps each output's xdg name → its `WlOutput`, so a runtime `NewLayerShell`
    /// with `OutputOption::OutputName` can be bound to that monitor. Rebuilt
    /// with `output_layout`: a stale entry would hand the compositor a removed
    /// output, and it places such a surface on the active output instead.
    output_handles: Vec<(String, wl_output::WlOutput)>,

    start_mode: StartMode,
    init_finished: bool,
    events_transparent: bool,
    /// Whether to request blur effect for surfaces
    blur: bool,
    /// Custom blur radius in pixels (None = compositor default). Applies to the
    /// initial + per-output (`AllScreens`/`NewDisplay`) surfaces.
    blur_radius: Option<f32>,
    /// Backdrop saturation multiplier for the blur (None = no change). Applies to
    /// the initial + per-output surfaces.
    blur_saturation: Option<f32>,
    /// Frosted-glass white-tint strength for the blur (None = compositor default).
    /// Applies to the initial + per-output surfaces.
    blur_tint: Option<f32>,
    /// Frosted-glass border strength for the blur (None = compositor default).
    /// Applies to the initial + per-output surfaces.
    blur_border: Option<f32>,
    /// Blur manager (bound lazily when blur is enabled)
    blur_manager: Option<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager>,
    /// The standard background-effect manager. Compositors commonly implement
    /// only one of this and KDE blur, so both are bound and both are set.
    background_effect_manager:
        Option<background_effect::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    background_effect_surfaces: HashMap<
        u32,
        background_effect::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
    >,
    /// Blur objects per surface (keyed by surface protocol ID)
    blur_surfaces: HashMap<u32, blur::org_kde_kwin_blur::OrgKdeKwinBlur>,
    /// Per-surface frosted-glass params `(radius, saturation, tint, border)`,
    /// captured at surface creation so a later blur re-enable (the auto-size
    /// deferred-effects path) re-applies them instead of compositor defaults.
    blur_params: HashMap<u32, BlurParams>,
    /// Adaptive-foreground manager (bound with the other effect managers).
    adaptive_foreground_manager: Option<
        adaptive_foreground::adaptive_foreground_manager_v1::AdaptiveForegroundManagerV1,
    >,
    /// Adaptive-foreground objects per surface (keyed by surface protocol ID).
    adaptive_foreground_surfaces: HashMap<
        u32,
        adaptive_foreground::adaptive_foreground_surface_v1::AdaptiveForegroundSurfaceV1,
    >,
    /// Luminance readings arrived since the last `done`, keyed by surface
    /// protocol ID. The protocol batches so a surface with several zones
    /// restyles once rather than once per zone, so they are held here until
    /// `done` releases them.
    adaptive_foreground_pending: HashMap<u32, Vec<(u32, f32)>>,
    /// Corner radius for surfaces (all four corners)
    corner_radius: Option<[u32; 4]>,
    /// Corner radius manager (bound lazily when corner_radius is set)
    corner_radius_manager:
        Option<corner_radius::layer_corner_radius_manager_v1::LayerCornerRadiusManagerV1>,
    /// Corner radius surfaces per surface (keyed by surface protocol ID)
    corner_radius_surfaces:
        HashMap<u32, corner_radius::layer_corner_radius_surface_v1::LayerCornerRadiusSurfaceV1>,
    /// Compositor-side placement manager (bound lazily when a placement is set)
    layer_surface_placement_manager: Option<
        layer_surface_placement::layer_surface_placement_manager_v1::LayerSurfacePlacementManagerV1,
    >,
    /// Placement objects per surface (keyed by surface protocol ID)
    layer_surface_placement_surfaces: HashMap<
        u32,
        layer_surface_placement::layer_surface_placement_v1::LayerSurfacePlacementV1,
    >,
    /// Compositor-side edge-resize manager (bound lazily when opted in)
    layer_edge_resize_manager:
        Option<layer_edge_resize::layer_edge_resize_manager_v1::LayerEdgeResizeManagerV1>,
    /// Edge-resize objects per surface (keyed by surface protocol ID)
    layer_edge_resize_surfaces:
        HashMap<u32, layer_edge_resize::layer_edge_resize_v1::LayerEdgeResizeV1>,
    /// Whether to request shadow effect for surfaces
    shadow: bool,
    /// Shadow manager (bound lazily when shadow is enabled)
    shadow_manager: Option<shadow::layer_shadow_manager_v1::LayerShadowManagerV1>,
    /// xdg-activation, used to ask the compositor for keyboard focus. A layer
    /// surface has no other way to request it: the compositor only hands focus to
    /// one on a pointer click, or when it is shown/made exclusive on Top/Overlay.
    xdg_activation: Option<XdgActivationV1>,
    /// Shadow objects per surface (keyed by surface protocol ID)
    shadow_surfaces: HashMap<u32, shadow::layer_shadow_surface_v1::LayerShadowSurfaceV1>,
    /// Keyboard-shortcuts-inhibit manager (bound lazily when first requested)
    keyboard_shortcuts_inhibit_manager: Option<ZwpKeyboardShortcutsInhibitManagerV1>,
    /// Active shortcut inhibitors per surface (keyed by surface protocol ID).
    /// While an inhibitor is active, the compositor forwards ALL keys — including
    /// its own global shortcuts (e.g. Alt+Tab) — to that surface instead of
    /// handling them, so an overlay (e.g. the Alt-Tab switcher) can receive Tab
    /// presses + key-repeat directly.
    keyboard_shortcuts_inhibitors: HashMap<u32, ZwpKeyboardShortcutsInhibitorV1>,
    /// Global show/hide transition animation requested for surfaces (via the
    /// `layer_surface_visibility` protocol).  `None` lets the compositor decide
    /// based on the surface anchor.  Applied when a visibility controller is
    /// created (requires compositor support for the protocol version 2).
    transition: Option<LayerTransition>,
    /// Per-surface transition overrides (keyed by surface protocol ID), set from
    /// [`NewLayerShellSettings::transition`].  Takes precedence over `transition`
    /// for that surface.  See [`WindowState::surface_transition`].
    transitions: HashMap<u32, LayerTransition>,

    /// Auto-hide manager (bound lazily when needed)
    auto_hide_manager:
        Option<layer_auto_hide::layer_auto_hide_manager_v1::LayerAutoHideManagerV1>,
    /// Auto-hide objects per surface (keyed by surface protocol ID)
    auto_hide_surfaces:
        HashMap<u32, layer_auto_hide::layer_auto_hide_v1::LayerAutoHideV1>,
    /// Whether the compositor currently considers us visible via auto-hide.
    /// Updated immediately when the compositor sends a visibility-changed event.
    /// Defaults to `true` (visible) until auto-hide is configured and the
    /// compositor tells us otherwise.
    auto_hide_visible: bool,

    /// Usable-area manager (bound at startup when the compositor supports it).
    /// Reports each surface's output usable (non-exclusive) area so consumers
    /// can center within the space free of panels/docks rather than the full
    /// output. Delivered to the app via `output_info::usable_area_subscription`.
    usable_area_manager:
        Option<layer_usable_area::layer_usable_area_manager_v1::LayerUsableAreaManagerV1>,
    /// Usable-area objects per surface (keyed by surface protocol ID).
    usable_area_surfaces:
        HashMap<u32, layer_usable_area::layer_usable_area_v1::LayerUsableAreaV1>,
    /// Announces animated size changes so a surface can move its own content.
    size_transition_manager: Option<
        layer_size_transition::layer_size_transition_manager_v1::LayerSizeTransitionManagerV1,
    >,
    size_transition_surfaces:
        HashMap<u32, layer_size_transition::layer_size_transition_v1::LayerSizeTransitionV1>,
    /// Reports when a workspace switch is animating, so a surface can fade its
    /// contents across it instead of swapping mid-animation.
    workspace_transition_manager: Option<
        workspace_transition::workspace_transition_manager_v1::WorkspaceTransitionManagerV1,
    >,

    /// Tooltip manager (bound lazily when a popup with tooltip settings is created)
    tooltip_manager:
        Option<tooltip::zcosmic_tooltip_manager_v1::ZcosmicTooltipManagerV1>,
    /// Register surfaces to receive the device's special key.
    special_action: bool,
    /// Whether the first registered surface is also the fallback receiver.
    special_action_default: bool,
    /// Special action manager (bound lazily when `special_action` is set)
    special_action_manager:
        Option<special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1>,
    /// Special action receivers per surface (keyed by surface protocol ID)
    special_action_receivers:
        HashMap<u32, special_action::zcosmic_special_action_v1::ZcosmicSpecialActionV1>,

    /// Tooltip objects per popup surface (keyed by surface protocol ID)
    tooltip_surfaces:
        HashMap<u32, tooltip::zcosmic_tooltip_v1::ZcosmicTooltipV1>,

    /// Capture exclusion manager, bound when the loop starts
    capture_exclusion_manager: Option<
        capture_exclusion::kora_capture_exclusion_manager_v1::KoraCaptureExclusionManagerV1,
    >,
    /// Capture exclusion objects per surface (keyed by surface protocol ID)
    capture_exclusions: HashMap<
        u32,
        capture_exclusion::kora_surface_capture_exclusion_v1::KoraSurfaceCaptureExclusionV1,
    >,


    /// Layer surface visibility manager (bound lazily when needed)
    layer_surface_visibility_manager: Option<
        layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1,
    >,
    /// Visibility controllers per surface (keyed by surface protocol ID)
    layer_surface_visibility_controllers: HashMap<
        u32,
        layer_surface_visibility::zcosmic_layer_surface_visibility_v1::ZcosmicLayerSurfaceVisibilityV1,
    >,
    /// Surface protocol IDs that are currently hidden via the
    /// `layer_surface_visibility` protocol.  Updated immediately on
    /// `hide_surface`/`show_surface` calls and confirmed by the
    /// compositor's `VisibilityChanged` events.
    hidden_surfaces: HashSet<u32>,

    /// Layer surface dismiss manager (bound lazily when needed)
    layer_surface_dismiss_manager: Option<
        layer_surface_dismiss::zcosmic_layer_surface_dismiss_manager_v1::ZcosmicLayerSurfaceDismissManagerV1,
    >,
    /// Dismiss controllers per surface (keyed by surface protocol ID)
    layer_surface_dismiss_controllers: HashMap<
        u32,
        layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::ZcosmicLayerSurfaceDismissV1,
    >,
    /// Pending dismiss events from compositor
    dismiss_requested: bool,

    /// Whether to track foreign toplevel windows (taskbar/dock functionality)
    #[cfg(feature = "foreign-toplevel")]
    foreign_toplevel_enabled: bool,

    // ext_foreign_toplevel_list_v1 (preferred, standard Wayland protocol)
    #[cfg(feature = "foreign-toplevel")]
    ext_foreign_toplevel_list: Option<
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    >,

    // zcosmic_toplevel_info_v1 (COSMIC extension for state info)
    #[cfg(feature = "cosmic-toplevel")]
    cosmic_toplevel_info: Option<
        cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    >,

    #[cfg(feature = "cosmic-toplevel")]
    toplevel_identity: toplevel_identity::State,

    // zcosmic_toplevel_manager_v1 (COSMIC extension for control - activate, close, etc.)
    #[cfg(feature = "cosmic-toplevel")]
    cosmic_toplevel_manager: Option<
        cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
    >,

    /// Mapping from cosmic handle protocol ID to ext handle protocol ID
    #[cfg(feature = "cosmic-toplevel")]
    cosmic_to_ext_handle_map: HashMap<u32, u32>,

    /// COSMIC toplevel handles for control operations (keyed by ext handle protocol ID)
    #[cfg(feature = "cosmic-toplevel")]
    cosmic_toplevel_handles: HashMap<u32, cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>,

    // zwlr_foreign_toplevel_manager_v1 (wlroots fallback)
    #[cfg(feature = "foreign-toplevel")]
    foreign_toplevel_manager: Option<
        wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
    >,

    /// Data for each tracked foreign toplevel window (keyed by protocol object ID)
    #[cfg(feature = "foreign-toplevel")]
    foreign_toplevel_data: HashMap<u32, foreign_toplevel::ToplevelHandleData>,

    /// Handles for each tracked foreign toplevel window (for sending commands)
    #[cfg(feature = "foreign-toplevel")]
    foreign_toplevel_handles: HashMap<u32, wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1>,

    /// Ext foreign toplevel handles (for screencopy capture sources)
    #[cfg(feature = "foreign-toplevel")]
    ext_toplevel_handles: HashMap<u32, wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1>,

    /// Screencopy protocol state
    #[cfg(feature = "workspaces")]
    workspaces: ext_workspace::WorkspacesState,
    #[cfg(feature = "screencopy")]
    screencopy: screencopy::ScreencopyState,
    /// Cached QueueHandle for use after event_queue is taken by the event loop
    /// (needed by screencopy and by `start_drag_for_surface`).
    queue_handle: Option<wayland_client::QueueHandle<WindowState<T>>>,
    /// Cached WlShm for use after shm is taken by the event loop
    #[cfg(feature = "screencopy")]
    screencopy_shm: Option<wayland_client::protocol::wl_shm::WlShm>,

    text_input_manager: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,
    text_inputs: Vec<ZwpTextInputV3>,

    xdg_decoration_manager: Option<ZxdgDecorationManagerV1>,

    ime_purpose: ImePurpose,
    ime_allowed: bool,
    /// The focused field's surrounding text, as last sent to the text inputs.
    ime_surrounding: Option<SurroundingText>,
    /// Whether an input method commit is what last changed the focused field.
    ime_committed: bool,

    /// Ping sender for waking the event loop immediately after a channel
    /// message is processed.  Populated once in `running_with_proxy_option`.
    ping_sender: Option<calloop::ping::Ping>,

    /// Wakes the pass that handles queued messages and presents due frames.
    tick_sender: Option<calloop::ping::Ping>,
    /// Another pass was asked for after [`RETRY_DELAY`] (see [`WindowState::wake_soon`]).
    retry_requested: bool,
    retry_timer: Option<RegistrationToken>,
    /// The one-shot timer for the earliest timed redraw.
    refresh_timer: Option<(Instant, RegistrationToken)>,
}

/// How long a pass that can't finish yet waits before trying again.
const RETRY_DELAY: Duration = Duration::from_millis(50);

/// Register a surface to receive the device's special key.
fn register_special_action_for_surface<T: 'static>(
    manager: &Option<
        special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1,
    >,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
    is_default: bool,
) -> Option<special_action::zcosmic_special_action_v1::ZcosmicSpecialActionV1> {
    let manager = manager.as_ref()?;
    let data = special_action::SpecialActionData {
        surface: surface.clone(),
    };
    let receiver = manager.get_special_action(surface, u32::from(is_default), qh, data);
    log::info!(
        "Registered layer surface as a special action receiver (default: {})",
        is_default
    );
    Some(receiver)
}

impl<T> WindowState<T> {
    pub fn append_return_data(&mut self, data: ReturnData<T>) {
        self.return_data.push(data);
    }

    /// The surfaces asked for with [`append_return_data`](Self::append_return_data)
    /// that the loop hasn't made yet.
    pub fn requested_surfaces(&self) -> impl Iterator<Item = id::Id> + '_ {
        self.return_data.iter().filter_map(|data| match data {
            ReturnData::NewLayerShell((_, id, _))
            | ReturnData::NewPopUp((_, id, _))
            | ReturnData::NewXdgBase((_, id, _))
            | ReturnData::NewInputPanel((_, id, _)) => Some(*id),
            _ => None,
        })
    }
    /// remove a shell, destroy the surface
    fn remove_shell(&mut self, id: id::Id) -> Option<()> {
        let index = self
            .units
            .iter()
            .position(|unit| unit.id == id && unit.becreated)?;

        // Clean up per-surface protocol objects BEFORE destroying the surface.
        // Protocol objects reference the wl_surface; using them after destruction
        // causes "surface_destroyed" protocol errors. Wayland reuses protocol IDs,
        // so stale entries would be found by new surfaces with the same ID.
        let surface_id = self.units[index].wl_surface.id().protocol_id();
        if let Some(corner_obj) = self.corner_radius_surfaces.remove(&surface_id) {
            corner_obj.destroy();
        }
        if let Some(placement_obj) = self.layer_surface_placement_surfaces.remove(&surface_id) {
            placement_obj.destroy();
        }
        if let Some(edge_resize_obj) = self.layer_edge_resize_surfaces.remove(&surface_id) {
            edge_resize_obj.destroy();
        }
        if let Some(blur_obj) = self.blur_surfaces.remove(&surface_id) {
            blur_obj.release();
        }
        if let Some(effect) = self.background_effect_surfaces.remove(&surface_id) {
            effect.destroy();
        }
        self.blur_params.remove(&surface_id);
        if let Some(adaptive_fg_obj) = self.adaptive_foreground_surfaces.remove(&surface_id) {
            adaptive_fg_obj.destroy();
        }
        // Readings for a surface that is going away would otherwise be flushed
        // onto whichever surface next reused the protocol id.
        self.adaptive_foreground_pending.remove(&surface_id);
        if let Some(shadow_obj) = self.shadow_surfaces.remove(&surface_id) {
            shadow_obj.destroy();
        }
        if let Some(inhibitor) = self.keyboard_shortcuts_inhibitors.remove(&surface_id) {
            inhibitor.destroy();
        }
        if let Some(auto_hide_obj) = self.auto_hide_surfaces.remove(&surface_id) {
            auto_hide_obj.destroy();
        }
        if let Some(usable_area_obj) = self.usable_area_surfaces.remove(&surface_id) {
            usable_area_obj.destroy();
        }
        if let Some(obj) = self.size_transition_surfaces.remove(&surface_id) {
            obj.destroy();
        }
        if let Some(tooltip_obj) = self.tooltip_surfaces.remove(&surface_id) {
            tooltip_obj.destroy();
        }
        if let Some(exclusion) = self.capture_exclusions.remove(&surface_id) {
            exclusion.destroy();
        }
        if let Some(visibility_obj) = self
            .layer_surface_visibility_controllers
            .remove(&surface_id)
        {
            visibility_obj.destroy();
        }
        self.hidden_surfaces.remove(&surface_id);
        self.transitions.remove(&surface_id);
        if let Some(dismiss_obj) = self.layer_surface_dismiss_controllers.remove(&surface_id) {
            dismiss_obj.destroy();
        }
        if let Some(action_obj) = self.special_action_receivers.remove(&surface_id) {
            action_obj.destroy();
        }

        self.units[index].shell.destroy();
        self.units[index].wl_surface.destroy();

        if let Some(buffer) = self.units[index].buffer.as_ref() {
            buffer.destroy()
        }
        self.units.remove(index);
        Some(())
    }

    /// forget the remembered last output, next time it will get the new activated output to set the
    /// layershell
    pub fn forget_last_output(&mut self) {
        self.last_wloutput.take();
    }
}

/// Simple WindowState, without any data binding or info
pub type WindowStateSimple = WindowState<()>;

impl<T> WindowState<T> {
    // return the first window
    // I will use it in iced
    pub fn main_window(&self) -> &WindowStateUnit<T> {
        &self.units[0]
    }

    /// use iced id to find WindowStateUnit
    pub fn get_window_with_id(&self, id: id::Id) -> Option<&WindowStateUnit<T>> {
        self.units.iter().find(|w| w.id() == id)
    }
    // return all windows
    pub fn windows(&self) -> &Vec<WindowStateUnit<T>> {
        &self.units
    }

    fn push_window(&mut self, window_state_unit: WindowStateUnit<T>) {
        let surface = window_state_unit.wl_surface.clone();
        self.units.push(window_state_unit);
        // new created surface will be current_surface.
        self.update_current_surface(Some(surface));
    }

    /// Asks for `unit_id` to be drawn. Its output's size usually arrives before
    /// the first draw, when the consumer has no window to give it to, so say it again.
    fn refresh_unit<F, Message>(
        &mut self,
        event_handler: &mut F,
        unit_id: id::Id,
        (width, height): (u32, u32),
        is_created: bool,
        scale_float: f64,
    ) where
        T: 'static,
        Message: std::marker::Send + 'static,
        F: FnMut(LayerShellEvent<T, Message>, &mut WindowState<T>, Option<id::Id>) -> ReturnData<T>,
    {
        self.handle_event(
            &mut *event_handler,
            LayerShellEvent::RequestMessages(&DispatchMessage::RequestRefresh {
                width,
                height,
                is_created,
                scale_float,
            }),
            Some(unit_id),
        );
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == unit_id) else {
            return;
        };
        if std::mem::replace(&mut unit.initial_refresh_sent, true) {
            return;
        }
        if let Some(output) = unit.output_info() {
            self.handle_event(
                &mut *event_handler,
                LayerShellEvent::RequestMessages(&output),
                Some(unit_id),
            );
        }
    }
}

#[derive(Debug)]
pub struct WindowWrapper {
    pub id: id::Id,
    display: WlDisplay,
    wl_surface: WlSurface,
    pub viewport: Option<WpViewport>,
    pub toplevel: Option<XdgToplevel>,
}

/// Define the way layershell program is start
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StartMode {
    /// default is use the activated display, in layershell, the param is `None`
    #[default]
    Active,
    /// be started as background program, be used with some programs like xdg-desktop-portal
    Background,
    /// listen on the create event of display, always shown on all screens
    AllScreens,
    /// only shown on target screen
    TargetScreen(String),

    /// Target the output
    /// NOTE: use the same wayland connection
    TargetOutput(WlOutput),
}

impl StartMode {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_background(&self) -> bool {
        matches!(self, Self::Background)
    }
    pub fn is_allscreens(&self) -> bool {
        matches!(self, Self::AllScreens)
    }
    pub fn is_with_target(&self) -> bool {
        matches!(self, Self::TargetScreen(_))
    }
}

impl WindowWrapper {
    pub fn id(&self) -> id::Id {
        self.id
    }
}

impl<T> WindowState<T> {
    /// get a seat from state
    pub fn get_seat(&self) -> &WlSeat {
        self.seat.as_ref().unwrap()
    }

    /// get the keyboard
    pub fn get_keyboard(&self) -> Option<&WlKeyboard> {
        Some(&self.keyboard_state.as_ref()?.keyboard)
    }

    /// get the pointer
    pub fn get_pointer(&self) -> Option<&WlPointer> {
        self.pointer.as_ref()
    }

    /// get the touch
    pub fn get_touch(&self) -> Option<&WlTouch> {
        self.touch.as_ref()
    }
}

impl<T> WindowState<T> {
    /// gen the wrapper to the main window
    /// used to get display and etc
    pub fn gen_mainwindow_wrapper(&self) -> WindowWrapper {
        self.main_window().gen_wrapper()
    }

    pub fn is_active(&self) -> bool {
        self.start_mode.is_active()
    }

    pub fn is_background(&self) -> bool {
        self.start_mode.is_background()
    }

    pub fn is_allscreens(&self) -> bool {
        self.start_mode.is_allscreens()
    }

    pub fn is_with_target(&self) -> bool {
        self.start_mode.is_with_target()
    }

    /// True when at least one surface unit still has a live `wl_surface`.
    ///
    /// Goes false when the only surface was destroyed because its output was
    /// disabled (the dead unit is dropped by the `GlobalRemove`/`Closed`
    /// paths).  Used to decide whether a single-surface `Active` app needs its
    /// surface re-created when a display is re-enabled.
    fn has_live_surface(&self) -> bool {
        self.units.iter().any(|unit| unit.wl_surface.is_alive())
    }

    /// Execute a toplevel action (activate, close, minimize, etc.)
    ///
    /// Returns true if the action was executed, false if the handle was not found.
    /// Requires the `foreign-toplevel` feature.
    #[cfg(feature = "foreign-toplevel")]
    pub fn execute_toplevel_action(&self, action: foreign_toplevel::ToplevelAction) -> bool
    where
        T: 'static,
    {
        foreign_toplevel::execute_toplevel_action(self, action, self.seat.as_ref())
    }

    /// Execute a screencopy action (capture a toplevel window screenshot)
    ///
    /// Requires the `screencopy` feature.
    #[cfg(feature = "screencopy")]
    pub fn execute_screencopy_action(
        &mut self,
        action: screencopy::ScreencopyAction,
        qh: &QueueHandle<Self>,
    ) where
        T: 'static,
    {
        match action {
            screencopy::ScreencopyAction::Capture(toplevel_id) => {
                screencopy::start_capture(self, toplevel_id, qh);
            }
            #[cfg(feature = "workspaces")]
            screencopy::ScreencopyAction::CaptureWorkspace(id) => {
                screencopy::start_workspace_capture(self, id, qh);
            }
            screencopy::ScreencopyAction::StartContinuous {
                target_width,
                target_height,
            } => {
                log::debug!(
                    "Starting continuous screencopy capture (target {}x{})",
                    target_width,
                    target_height
                );
                self.screencopy.continuous = true;
                self.screencopy.target_size = Some((target_width, target_height));
                let ids: Vec<u32> = self.screencopy.sessions.keys().copied().collect();
                for tid in ids {
                    screencopy::start_capture(self, tid, qh);
                }
            }
            screencopy::ScreencopyAction::StopContinuous => {
                log::debug!("Stopping continuous screencopy capture");
                self.screencopy.continuous = false;
                self.screencopy.target_size = None;
                self.screencopy.last_capture.clear();
            }
            #[cfg(feature = "workspaces")]
            screencopy::ScreencopyAction::SetWorkspacePreviewSize { width, height } => {
                self.screencopy.workspace_preview_size =
                    (width > 0 && height > 0).then_some((width, height));
            }
            #[cfg(feature = "workspaces")]
            screencopy::ScreencopyAction::WatchWorkspace(id) => {
                self.screencopy.live_workspaces.insert(id);
                screencopy::start_workspace_capture(self, id, qh);
            }
            #[cfg(feature = "workspaces")]
            screencopy::ScreencopyAction::UnwatchWorkspaces => {
                screencopy::stop_workspace_captures(self);
            }
        }
    }

    /// Execute a screencopy action using the internal event queue handle
    #[cfg(feature = "screencopy")]
    pub fn execute_screencopy_action_internal(&mut self, action: screencopy::ScreencopyAction)
    where
        T: 'static,
    {
        let qh = match self.queue_handle.as_ref() {
            Some(qh) => qh.clone(),
            None => {
                log::warn!("Screencopy action dropped: queue_handle not yet initialized");
                return;
            }
        };
        self.execute_screencopy_action(action, &qh);
    }

    pub fn ime_allowed(&self) -> bool {
        self.ime_allowed
    }

    pub fn set_ime_allowed(&mut self, ime_allowed: bool) {
        self.ime_allowed = ime_allowed;
        if !ime_allowed {
            self.ime_surrounding = None;
        }
        for text_input in &self.text_inputs {
            if ime_allowed {
                self.enable_text_input(text_input);
            } else {
                text_input.disable();
            }
            text_input.commit();
        }
    }

    /// Enable `text_input` with the IME state held here; the caller commits.
    fn enable_text_input(&self, text_input: &ZwpTextInputV3) {
        text_input.enable();
        text_input.set_content_type_by_purpose(self.ime_purpose);
        if let Some(surrounding) = &self.ime_surrounding {
            text_input.set_surrounding(surrounding);
        }
    }

    pub fn set_ime_cursor_area<P: Into<dpi::Position>, S: Into<dpi::Size>>(
        &self,
        position: P,
        size: S,
        id: id::Id,
    ) {
        if !self.ime_allowed() {
            return;
        }
        let position: dpi::Position = position.into();
        let size: dpi::Size = size.into();
        let Some(unit) = self.get_window_with_id(id) else {
            return;
        };
        let scale_factor = unit.scale_float();
        let position: dpi::LogicalPosition<u32> = position.to_logical(scale_factor);
        let size: dpi::LogicalSize<u32> = size.to_logical(scale_factor);
        let (x, y) = (position.x as i32, position.y as i32);
        let (width, height) = (size.width as i32, size.height as i32);
        for text_input in self.text_inputs.iter() {
            text_input.set_cursor_rectangle(x, y, width, height);
            text_input.commit();
        }
    }

    pub fn set_ime_purpose(&mut self, purpose: ImePurpose) {
        self.ime_purpose = purpose;
        // A password field's text is never shared, nor the text of the field before it.
        let forget = purpose == ImePurpose::Password
            && self.ime_surrounding.take().is_some()
            && self.ime_allowed;
        self.text_input.iter().for_each(|text_input| {
            if forget {
                self.enable_text_input(text_input);
            } else {
                text_input.set_content_type_by_purpose(purpose);
            }
            text_input.commit();
        });
    }

    /// Tell input methods the text around the caret of the focused field, so what they
    /// insert, such as dictated words, can be spaced and cased to fit. `None` for a field
    /// that keeps its text to itself: the text input is then enabled again, which makes
    /// input methods forget it. Nothing is sent while the purpose is
    /// [`ImePurpose::Password`], and text past the protocol's 4000-byte limit is cut to a
    /// window around the caret and selection.
    pub fn set_ime_surrounding_text(&mut self, surrounding: Option<&ImeSurroundingText>) {
        let next = SurroundingText::for_purpose(self.ime_purpose, surrounding);
        let change = SurroundingChange::between(
            self.ime_surrounding.as_ref(),
            next.as_ref(),
            self.ime_committed,
        );
        if change == SurroundingChange::Unchanged {
            return;
        }
        self.ime_surrounding = next;
        self.ime_committed = false;
        if !self.ime_allowed {
            return;
        }
        for text_input in &self.text_inputs {
            match (change, &self.ime_surrounding) {
                (SurroundingChange::Send(cause), Some(surrounding)) => {
                    text_input.set_surrounding(surrounding);
                    text_input.set_text_change_cause(cause);
                }
                _ => self.enable_text_input(text_input),
            }
            text_input.commit();
        }
    }

    #[inline]
    pub fn text_input_entered(&mut self, text_input: &ZwpTextInputV3) {
        if !self.text_inputs.iter().any(|t| t == text_input) {
            self.text_inputs.push(text_input.clone());
        }
    }

    #[inline]
    pub fn text_input_left(&mut self, text_input: &ZwpTextInputV3) {
        if let Some(position) = self.text_inputs.iter().position(|t| t == text_input) {
            self.text_inputs.remove(position);
        }
    }
}

impl<T: 'static> WindowState<T> {
    /// Apply the configured surface effects (blur, corner radius, shadow, layer
    /// visibility) to a freshly created layer-shell surface.
    ///
    /// Shared by the initial `AllScreens` surface creation and the runtime
    /// `NewDisplay` path, so a monitor that is enabled after startup gets the
    /// exact same effects as the monitors that were present at boot. Keeping a
    /// single implementation avoids the two paths drifting apart (which
    /// previously left runtime-added monitors with no blur/shadow).
    fn apply_surface_effects(&mut self, wl_surface: &WlSurface, qh: &QueueHandle<Self>) {
        let surface_id = wl_surface.id().protocol_id();

        // Apply blur effect if enabled
        if self.blur {
            apply_blur_to_surface(
                &self.blur_manager,
                wl_surface,
                qh,
                self.blur_radius,
                self.blur_saturation,
                self.blur_tint,
                self.blur_border,
            );
        }

        // Apply corner radius if set
        if self.corner_radius.is_some()
            && let Some(corner_obj) = apply_corner_radius_to_surface(
                &self.corner_radius_manager,
                self.corner_radius,
                wl_surface,
                qh,
            )
        {
            self.corner_radius_surfaces.insert(surface_id, corner_obj);
        }

        // Apply shadow if enabled
        if self.shadow {
            apply_shadow_to_surface(&self.shadow_manager, wl_surface, qh);
        }

        // Register the surface to receive the device's special key.
        if self.special_action
            && let Some(receiver) = register_special_action_for_surface(
                &self.special_action_manager,
                wl_surface,
                qh,
                self.special_action_default && self.special_action_receivers.is_empty(),
            )
        {
            self.special_action_receivers.insert(surface_id, receiver);
        }

        // Register surface for compositor usable-area reporting.
        if let Some(obj) =
            register_usable_area_for_surface(&self.usable_area_manager, wl_surface, qh)
        {
            self.usable_area_surfaces.insert(surface_id, obj);
        }
        if let Some(obj) =
            register_size_transition_for_surface(&self.size_transition_manager, wl_surface, qh)
        {
            self.size_transition_surfaces.insert(surface_id, obj);
        }
    }

    /// Set corner radius for a specific surface
    /// radii: [top_left, top_right, bottom_right, bottom_left] or None to unset
    /// Ask the compositor to give `surface` keyboard focus.
    ///
    /// A layer surface cannot otherwise be focused on demand: the compositor
    /// hands it focus on a pointer click, or when it is shown / made exclusive on
    /// the Top or Overlay layer — none of which covers "the user just dropped a
    /// file on me and wants to type". xdg-activation is the standard way to ask,
    /// and cosmic-comp routes an activation request for a layer surface straight
    /// to `set_focus`.
    ///
    /// One-shot, and advisory: the compositor is free to refuse (focus-stealing
    /// prevention, a lock screen, an exclusive surface elsewhere). It is not a
    /// grab — focus moves away again normally.
    pub fn request_focus_for_surface(&mut self, surface: &WlSurface) {
        let Some(activation) = self.xdg_activation.clone() else {
            log::debug!("xdg_activation_v1 unavailable — cannot request focus");
            return;
        };
        let Some(unit) = self.units.first() else {
            return;
        };
        let qh = unit.qh.clone();

        // The token carries its target, so `Done` knows what to activate.
        let token = activation.get_activation_token(&qh, surface.clone());
        if let (Some(seat), Some(serial)) = (self.seat.as_ref(), self.enter_serial) {
            // Lets the compositor judge the request against a real input event
            // rather than treating it as an unsolicited focus steal.
            token.set_serial(serial, seat);
        }
        token.set_surface(surface);
        token.commit();
    }

    pub fn set_corner_radius_for_surface(&mut self, surface: &WlSurface, radii: Option<[u32; 4]>) {
        let surface_id = surface.id().protocol_id();

        // Check if we already have a corner radius object for this surface
        if let Some(corner_obj) = self.corner_radius_surfaces.get(&surface_id) {
            if let Some(r) = radii {
                corner_obj.set_radius(r[0], r[1], r[2], r[3]);
                log::info!("Updated corner radius for surface: {:?}", r);
            } else {
                corner_obj.unset_radius();
                log::info!("Unset corner radius for surface");
            }
            surface.commit();
            return;
        }

        // Need to create a new corner radius object
        if let Some(manager) = &self.corner_radius_manager {
            let corner_data = corner_radius::CornerRadiusData {
                surface: surface.clone(),
            };
            // Get a queue handle from the first unit (they all share the same queue)
            if let Some(unit) = self.units.first() {
                let corner_obj = manager.get_corner_radius(surface, &unit.qh, corner_data);
                if let Some(r) = radii {
                    corner_obj.set_radius(r[0], r[1], r[2], r[3]);
                    log::info!("Updated corner radius for surface: {:?}", r);
                } else {
                    corner_obj.unset_radius();
                    log::info!("Unset corner radius for surface");
                }
                self.corner_radius_surfaces.insert(surface_id, corner_obj);
                surface.commit();
            }
        } else {
            log::warn!(
                "Corner radius manager not available - ensure corner_radius was set in settings"
            );
        }
    }

    /// Get (or lazily create) the compositor-side placement object for a surface.
    /// Returns a clone of the object handle, or `None` if the compositor lacks
    /// `layer_surface_placement_manager_v1` (or there is no queue yet).
    fn placement_object_for(
        &mut self,
        surface: &WlSurface,
    ) -> Option<layer_surface_placement::layer_surface_placement_v1::LayerSurfacePlacementV1> {
        let surface_id = surface.id().protocol_id();
        if !self
            .layer_surface_placement_surfaces
            .contains_key(&surface_id)
        {
            // `manager` and `units` are disjoint fields from the map we insert
            // into, so these borrows don't conflict with the insert.
            let manager = self.layer_surface_placement_manager.as_ref()?;
            let unit = self.units.first()?;
            let data = layer_surface_placement::LayerSurfacePlacementData {
                surface: surface.clone(),
            };
            let obj = manager.get_layer_surface_placement(surface, &unit.qh, data);
            self.layer_surface_placement_surfaces
                .insert(surface_id, obj);
        }
        self.layer_surface_placement_surfaces
            .get(&surface_id)
            .cloned()
    }

    /// Ask the compositor to position `surface` vertically within its output's
    /// usable (non-exclusive) area: top edge at
    /// `usable_top + round(fraction * usable_height) + offset`, clamped to at
    /// least `min_margin` below the usable top. The compositor maintains the
    /// placement across relayouts, so the surface never jumps from a stale
    /// position when it first appears on an output. Requires compositor support
    /// for `layer_surface_placement_manager_v1`; a no-op otherwise.
    pub fn set_vertical_placement_for_surface(
        &mut self,
        surface: &WlSurface,
        fraction: f64,
        offset: i32,
        min_margin: i32,
    ) {
        if let Some(obj) = self.placement_object_for(surface) {
            obj.set_vertical_placement(fraction, offset, min_margin);
            surface.commit();
        } else {
            log::warn!(
                "Layer surface placement manager not available - compositor lacks layer_surface_placement_manager_v1"
            );
        }
    }

    /// Clear any compositor-side vertical placement for `surface`, reverting it
    /// to standard layer-shell positioning. A no-op if no placement was set.
    pub fn unset_vertical_placement_for_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();
        if let Some(obj) = self.layer_surface_placement_surfaces.get(&surface_id) {
            obj.unset_vertical_placement();
            surface.commit();
        }
    }

    /// Ask the compositor to cap `surface`'s height to
    /// `max(round(fraction * usable_height), min_height)` logical px. Lets the
    /// client commit its full content height and have the compositor clamp it to
    /// a fraction of the usable area, across relayouts, with no stale-cap recompute.
    pub fn set_max_height_for_surface(
        &mut self,
        surface: &WlSurface,
        fraction: f64,
        min_height: i32,
    ) {
        if let Some(obj) = self.placement_object_for(surface) {
            obj.set_max_height(fraction, min_height);
            surface.commit();
        } else {
            log::warn!(
                "Layer surface placement manager not available - compositor lacks layer_surface_placement_manager_v1"
            );
        }
    }

    /// Clear any compositor-side height cap for `surface`. A no-op if none was set.
    pub fn unset_max_height_for_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();
        if let Some(obj) = self.layer_surface_placement_surfaces.get(&surface_id) {
            obj.unset_max_height();
            surface.commit();
        }
    }

    /// Get (or lazily create) the compositor-side edge-resize object for a surface.
    /// Returns a clone, or `None` if the compositor lacks
    /// `layer_edge_resize_manager_v1` (or there is no queue yet).
    fn edge_resize_object_for(
        &mut self,
        surface: &WlSurface,
    ) -> Option<layer_edge_resize::layer_edge_resize_v1::LayerEdgeResizeV1> {
        let surface_id = surface.id().protocol_id();
        if !self.layer_edge_resize_surfaces.contains_key(&surface_id) {
            let manager = self.layer_edge_resize_manager.as_ref()?;
            let unit = self.units.first()?;
            let data = layer_edge_resize::LayerEdgeResizeData {
                surface: surface.clone(),
            };
            let obj = manager.get_edge_resize(surface, &unit.qh, data);
            self.layer_edge_resize_surfaces.insert(surface_id, obj);
        }
        self.layer_edge_resize_surfaces.get(&surface_id).cloned()
    }

    /// Opt `surface` into the compositor-drawn edge resize sash, with width bounds
    /// in logical px (`max_width == 0` means full output width). Requires compositor
    /// support for `layer_edge_resize_manager_v1`; a no-op otherwise.
    pub fn set_edge_resize_for_surface(
        &mut self,
        surface: &WlSurface,
        min_width: i32,
        max_width: i32,
    ) {
        if let Some(obj) = self.edge_resize_object_for(surface) {
            obj.set_min_width(min_width);
            obj.set_max_width(max_width);
            surface.commit();
        } else {
            log::warn!(
                "Layer edge-resize manager not available - compositor lacks layer_edge_resize_manager_v1"
            );
        }
    }

    /// Disable the compositor-drawn edge resize for `surface`. A no-op if it was
    /// never opted in.
    pub fn unset_edge_resize_for_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();
        if let Some(obj) = self.layer_edge_resize_surfaces.remove(&surface_id) {
            obj.destroy();
            surface.commit();
        }
    }

    /// Enable or disable blur effect for a specific surface, over the KDE blur
    /// protocol and the standard background effect, whichever the compositor offers.
    pub fn set_blur_for_surface(&mut self, surface: &WlSurface, enabled: bool) {
        let surface_id = surface.id().protocol_id();

        if enabled {
            // Check if blur is already enabled for this surface
            if self.blur_surfaces.contains_key(&surface_id)
                || self.background_effect_surfaces.contains_key(&surface_id)
            {
                return;
            }

            // Need to bind the blur manager if not already bound
            if self.blur_manager.is_none()
                && let Some(globals) = &self.globals
                && let Some(unit) = self.units.first()
            {
                self.blur_manager = globals
                    .bind::<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, _, _>(
                        &unit.qh,
                        1..=6,
                        (),
                    )
                    .ok();
                if self.blur_manager.is_some() {
                    log::info!("Bound blur manager");
                }
            }

            let Some(unit) = self.units.first() else {
                return;
            };
            // Re-apply the per-surface frosted-glass params captured at
            // creation, so a deferred (auto-size) re-enable keeps the
            // requested radius/saturation/tint/border instead of the
            // compositor default (which would wash the blur white).
            let (radius, saturation, tint, border) = self
                .blur_params
                .get(&surface_id)
                .copied()
                .unwrap_or((None, None, None, None));

            if let Some(manager) = &self.blur_manager {
                let blur_data = blur::BlurData {
                    surface: surface.clone(),
                };
                let blur_obj = manager.create(surface, &unit.qh, blur_data);
                // Set region to null (entire surface)
                blur_obj.set_region(None);
                apply_blur_params(&blur_obj, radius, saturation, tint, border);
                blur_obj.commit();
                self.blur_surfaces.insert(surface_id, blur_obj);
            }

            // The loop takes `wl_compositor` when it starts; this copy stays.
            if let Some(effect_manager) = &self.background_effect_manager
                && let Some(compositor) = &self.cached_compositor
            {
                let effect = effect_manager.get_background_effect(
                    surface,
                    &unit.qh,
                    background_effect::BackgroundEffectData {
                        surface: surface.clone(),
                    },
                );
                // This protocol has no whole-surface request, so ask for
                // an oversized region and let the compositor clip it to
                // the surface. That keeps it right across resizes, which
                // a surface-sized region would not without a resend.
                let (x, y, w, h) = background_effect::WHOLE_SURFACE;
                let region = compositor.create_region(&unit.qh, ());
                region.add(x, y, w, h);
                background_effect::apply(&effect, Some(&region));
                region.destroy();
                self.background_effect_surfaces.insert(surface_id, effect);
            }

            if !self.blur_surfaces.contains_key(&surface_id)
                && !self.background_effect_surfaces.contains_key(&surface_id)
            {
                log::warn!("No blur protocol available - compositor may not support blur");
                return;
            }
            surface.commit();
            log::info!(
                "Enabled blur for surface (radius={radius:?}, saturation={saturation:?}, tint={tint:?}, border={border:?})"
            );
        } else {
            // Tear down both, or the surface keeps whichever protocol the
            // compositor actually honoured.
            let blur_obj = self.blur_surfaces.remove(&surface_id);
            let effect = self.background_effect_surfaces.remove(&surface_id);
            if blur_obj.is_none() && effect.is_none() {
                return;
            }
            if let Some(blur_obj) = blur_obj {
                blur_obj.release();
            }
            if let Some(effect) = effect {
                effect.set_blur_region(None);
                effect.destroy();
            }
            surface.commit();
            log::info!("Disabled blur for surface");
        }
    }

    /// Enable blur on a surface with explicit frosted-glass params, storing them
    /// in `blur_params` so a later (auto-size) deferred re-enable keeps them.
    ///
    /// [`set_blur_for_surface`] reuses params captured when the surface was
    /// created; a popup created via `popup::show` captures none, so without this
    /// it would blur with the compositor default (washing the blur white). This
    /// lets the client set the params explicitly for any surface — layer or
    /// popup — by id.
    pub fn set_blur_for_surface_with_params(
        &mut self,
        surface: &WlSurface,
        radius: Option<f32>,
        saturation: Option<f32>,
        tint: Option<f32>,
        border: Option<f32>,
    ) {
        let surface_id = surface.id().protocol_id();
        self.blur_params
            .insert(surface_id, (radius, saturation, tint, border));
        // Replace any existing blur object so the new params take effect (the
        // `enabled` path below skips surfaces that already have one).
        if let Some(old) = self.blur_surfaces.remove(&surface_id) {
            old.release();
        }
        self.set_blur_for_surface(surface, true);
    }

    /// Set a specific blur region for a surface. The callback receives a WlRegion
    /// to which the caller adds rectangles. If no rectangles are added, blur is
    /// disabled for the surface.
    ///
    /// `radii` gives each rectangle its own corner radii — `[top-left, top-right,
    /// bottom-right, bottom-left]` in logical px — in the same order the callback
    /// adds the rectangles, so a surface can round each backdrop to the shape it
    /// draws there (a pill next to smaller-radius cards). Pass `&[]` to leave every
    /// rectangle on the compositor's single radius. Rectangles past the end of
    /// `radii` fall back the same way. Needs blur protocol v4; on an older
    /// compositor the radii are dropped (with a warning) and the region still applies.
    ///
    /// This creates/replaces the blur object each time to update the region.
    /// Mark the zones of `surface` to be told the backdrop luminance behind.
    ///
    /// `set_region` receives a fresh `WlRegion` to add one rectangle per zone;
    /// their order is the index the compositor reports readings against, so a
    /// caller with several zones must add them in a stable order.
    ///
    /// Passing a callback that adds nothing disables reporting for the surface.
    pub fn set_adaptive_foreground_region_for_surface<F>(
        &mut self,
        surface: &WlSurface,
        region: &WlRegion,
        set_region: F,
    ) where
        F: FnOnce(&WlRegion),
    {
        let surface_id = surface.id().protocol_id();

        if self.adaptive_foreground_manager.is_none()
            && let Some(globals) = &self.globals
            && let Some(unit) = self.units.first()
        {
            self.adaptive_foreground_manager = globals
                .bind::<adaptive_foreground::adaptive_foreground_manager_v1::AdaptiveForegroundManagerV1, _, _>(
                    &unit.qh,
                    1..=1,
                    (),
                )
                .ok();
        }

        let Some(manager) = self.adaptive_foreground_manager.clone() else {
            log::warn!(
                "No adaptive_foreground protocol available - compositor does not report backdrop luminance"
            );
            return;
        };
        let Some(unit) = self.units.first() else {
            return;
        };

        set_region(region);

        // One object per surface, kept for the surface's lifetime. The protocol
        // raises `already_constructed` on a second get for the same surface, so
        // unlike blur -- which creates a fresh object per region update -- this
        // must reuse the existing one.
        let object = self
            .adaptive_foreground_surfaces
            .entry(surface_id)
            .or_insert_with(|| {
                let data = adaptive_foreground::AdaptiveForegroundData {
                    surface: surface.clone(),
                };
                manager.get_adaptive_foreground(surface, &unit.qh, data)
            });

        object.set_region(Some(region));
    }

    pub fn set_blur_region_for_surface<F>(
        &mut self,
        surface: &WlSurface,
        region: &WlRegion,
        radii: &[[u32; 4]],
        geometry: &[(f32, f32, f32, f32)],
        opacity: &[f32],
        set_region: F,
    ) where
        F: FnOnce(&WlRegion),
    {
        let surface_id = surface.id().protocol_id();

        // Ensure blur manager is bound
        if self.blur_manager.is_none()
            && let Some(globals) = &self.globals
            && let Some(unit) = self.units.first()
        {
            self.blur_manager = globals
                .bind::<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, _, _>(
                    &unit.qh,
                    1..=6,
                    (),
                )
                .ok();
            if self.blur_manager.is_some() {
                log::info!("Bound blur manager");
            }
        }

        if self.blur_manager.is_none() && self.background_effect_manager.is_none() {
            log::warn!("No blur protocol available - compositor may not support blur");
            return;
        }

        let Some(unit) = self.units.first() else {
            return;
        };

        // Let the caller define the blur region via the WlRegion. Done once,
        // before either protocol reads it, since both take the same region.
        log::info!(
            "set_blur_region_for_surface: surface={}, calling set_region callback",
            surface_id
        );
        set_region(region);

        // The surface's frosted-glass parameters, captured before the borrows
        // below. Both protocols describe the same glass, and both have to be
        // re-sent whenever the region is, because a commit consumes them.
        let (radius_param, saturation, tint, border) = self
            .blur_params
            .get(&surface_id)
            .copied()
            .unwrap_or((None, None, None, None));

        if let Some(manager) = &self.blur_manager {
            // Reuse the surface's blur object rather than releasing and
            // recreating it. All of this state is double-buffered, so re-setting
            // it is enough -- and a client animating its region (a card lifting
            // on hover re-sends at 60Hz) would otherwise churn a protocol object
            // per frame and reset radius/saturation/tint/border to the
            // compositor defaults every time, because a fresh object carries
            // none of the previously-sent values.
            let blur_obj = self.blur_surfaces.entry(surface_id).or_insert_with(|| {
                manager.create(
                    surface,
                    &unit.qh,
                    blur::BlurData {
                        surface: surface.clone(),
                    },
                )
            });
            blur_obj.set_region(Some(region));
            // Re-assert the surface's frosted-glass parameters on every region
            // update. The KDE object carries them as pending state that a commit
            // consumes, so a region-only commit leaves the compositor falling
            // back to its defaults -- visible as the backdrop's appearance
            // changing the moment a client starts animating its region, e.g. a
            // card lifting on hover re-sends at 60Hz and the border the surface
            // asked to suppress reappears.
            apply_blur_params(blur_obj, radius_param, saturation, tint, border);
            if !radii.is_empty() {
                if blur_obj.version() >= 4 {
                    blur_obj.set_region_radii(blur::encode_region_radii(radii));
                } else {
                    log::warn!(
                        "Per-region blur radii requested but compositor only supports blur protocol v{}, ignoring",
                        blur_obj.version()
                    );
                }
            }
            // The exact sub-pixel area, where the caller knows it. The region
            // above stays the whole-pixel bound; this is what the backdrop is
            // actually drawn to, so a surface at a fractional position or under
            // a scale animation stops having its backdrop rounded away from the
            // shape it draws. A v4 compositor simply keeps using the region.
            if !geometry.is_empty() && blur_obj.version() >= 5 {
                blur_obj.set_region_geometry(blur::encode_region_geometry(geometry));
            }
            // Lets a rect fading out inside the surface take its backdrop with
            // it; a v5 compositor draws every rect at full strength.
            if !opacity.is_empty() && blur_obj.version() >= 6 {
                blur_obj.set_region_opacity(blur::encode_region_opacity(opacity));
            }
            blur_obj.commit();
        }

        if let Some(effect_manager) = &self.background_effect_manager {
            // Reuse the surface's effect object: the compositor treats a second
            // get_background_effect for one surface as a protocol error, and the
            // state is double-buffered anyway so re-setting it is enough.
            let effect = self
                .background_effect_surfaces
                .entry(surface_id)
                .or_insert_with(|| {
                    effect_manager.get_background_effect(
                        surface,
                        &unit.qh,
                        background_effect::BackgroundEffectData {
                            surface: surface.clone(),
                        },
                    )
                });
            // The region is all this protocol carries. Corner radii and the
            // frosted-glass appearance went over the KDE path above.
            background_effect::apply(effect, Some(region));
        }
        // Applied with the next frame, so the region and the drawing that
        // matches it land in one commit.
        if let Some(id) = self.get_id_from_surface(surface) {
            self.request_refresh(id, RefreshRequest::NextFrame);
        }
        log::info!("set_blur_region_for_surface: surface={}, done", surface_id);
    }

    /// Enable or disable shadow effect for a specific surface.
    /// Requires compositor support for layer_shadow_manager_v1 protocol.
    pub fn set_shadow_for_surface(&mut self, surface: &WlSurface, enabled: bool) {
        let surface_id = surface.id().protocol_id();

        if enabled {
            // Check if shadow is already enabled for this surface
            if self.shadow_surfaces.contains_key(&surface_id) {
                return;
            }

            // Need to bind the shadow manager if not already bound
            if self.shadow_manager.is_none()
                && let Some(globals) = &self.globals
                && let Some(unit) = self.units.first()
            {
                self.shadow_manager = globals
                    .bind::<shadow::layer_shadow_manager_v1::LayerShadowManagerV1, _, _>(
                        &unit.qh,
                        1..=1,
                        (),
                    )
                    .ok();
                if self.shadow_manager.is_some() {
                    log::info!("Bound shadow manager");
                }
            }

            if let Some(manager) = &self.shadow_manager {
                if let Some(unit) = self.units.first() {
                    let shadow_data = shadow::ShadowData {
                        surface: surface.clone(),
                    };
                    let shadow_obj = manager.get_shadow(surface, &unit.qh, shadow_data);
                    shadow_obj.enable();
                    self.shadow_surfaces.insert(surface_id, shadow_obj);
                    surface.commit();
                    log::info!("Enabled shadow for surface");
                }
            } else {
                log::warn!("Shadow manager not available - compositor may not support shadows");
            }
        } else {
            // Disable shadow by removing and destroying the shadow object
            if let Some(shadow_obj) = self.shadow_surfaces.remove(&surface_id) {
                shadow_obj.destroy();
                surface.commit();
                log::info!("Disabled shadow for surface");
            }
        }
    }

    /// Leave a surface out of screen captures and recordings while it stays on
    /// screen, or put it back in them, from the commit this makes. Requires
    /// compositor support for `kora_capture_exclusion_manager_v1`; without it
    /// nothing changes. The surface keeps its exclusion object until it goes.
    pub fn set_capture_excluded_for_surface(&mut self, surface: &WlSurface, excluded: bool) {
        let surface_id = surface.id().protocol_id();
        if !self.capture_exclusions.contains_key(&surface_id) {
            if !excluded {
                return;
            }
            let Some(unit) = self.units.first() else {
                return;
            };
            let qh = unit.qh.clone();
            let Some(manager) = &self.capture_exclusion_manager else {
                log::warn!(
                    "Capture exclusion manager not available - compositor may not support it"
                );
                return;
            };
            let exclusion = manager.get_surface_exclusion(surface, &qh, ());
            self.capture_exclusions.insert(surface_id, exclusion);
        }
        if let Some(exclusion) = self.capture_exclusions.get(&surface_id) {
            exclusion.set_excluded(u32::from(excluded));
            surface.commit();
        }
    }

    /// Enable or disable a keyboard-shortcuts inhibitor for a specific surface.
    ///
    /// While enabled, the compositor forwards every key (including its own global
    /// shortcuts, e.g. Alt+Tab) to this surface instead of acting on them — needed
    /// so an overlay such as the Alt-Tab switcher receives `Tab` presses and their
    /// key-repeat directly rather than the compositor consuming them. Requires
    /// compositor support for `zwp_keyboard_shortcuts_inhibit_manager_v1` and a
    /// bound seat. The inhibitor is per-surface and idempotent.
    pub fn set_keyboard_shortcuts_inhibit_for_surface(
        &mut self,
        surface: &WlSurface,
        enabled: bool,
    ) {
        let surface_id = surface.id().protocol_id();

        if enabled {
            // Already inhibited for this surface.
            if self.keyboard_shortcuts_inhibitors.contains_key(&surface_id) {
                return;
            }

            let Some(seat) = self.seat.clone() else {
                log::warn!("No seat available - cannot inhibit keyboard shortcuts");
                return;
            };

            if let Some(manager) = &self.keyboard_shortcuts_inhibit_manager {
                if let Some(unit) = self.units.first() {
                    let inhibitor = manager.inhibit_shortcuts(surface, &seat, &unit.qh, ());
                    self.keyboard_shortcuts_inhibitors
                        .insert(surface_id, inhibitor);
                    log::info!("Enabled keyboard-shortcuts inhibitor for surface");
                }
            } else {
                log::warn!(
                    "Keyboard-shortcuts-inhibit manager not available - compositor may not support it"
                );
            }
        } else if let Some(inhibitor) = self.keyboard_shortcuts_inhibitors.remove(&surface_id) {
            inhibitor.destroy();
            log::info!("Disabled keyboard-shortcuts inhibitor for surface");
        }
    }

    /// Bind the capture exclusion manager when the compositor offers it. Called when
    /// the loop is built, because the loop takes `globals` when it starts.
    fn bind_capture_exclusion_manager(&mut self, globals: &GlobalList, qh: &QueueHandle<Self>) {
        self.capture_exclusion_manager = globals
            .bind::<capture_exclusion::kora_capture_exclusion_manager_v1::KoraCaptureExclusionManagerV1, _, _>(
                qh,
                1..=1,
                (),
            )
            .ok();
    }

    /// Bind the keyboard-shortcuts-inhibit manager when the compositor offers it.
    /// Called when the loop is built, because the loop takes `globals` when it starts.
    fn bind_keyboard_shortcuts_inhibit_manager(
        &mut self,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
    ) {
        self.keyboard_shortcuts_inhibit_manager = globals
            .bind::<ZwpKeyboardShortcutsInhibitManagerV1, _, _>(qh, 1..=1, ())
            .ok();
        if self.keyboard_shortcuts_inhibit_manager.is_some() {
            log::info!("Bound keyboard-shortcuts-inhibit manager");
        }
    }

    /// Enable compositor-driven auto-hide for a specific surface.
    /// The compositor will animate hide/show transitions and handle hover detection.
    /// `edge`: which edge to slide off (0 = bottom)
    /// `edge_zone`: hover detection zone in pixels at the screen edge
    /// `mode`: 0 = always, 1 = maximized/fullscreen, 2 = fullscreen only (protocol v2)
    pub fn set_auto_hide_for_surface(
        &mut self,
        surface: &WlSurface,
        edge: u32,
        edge_zone: u32,
        mode: u32,
    ) {
        let surface_id = surface.id().protocol_id();
        let edge_enum = layer_auto_hide::layer_auto_hide_v1::Edge::try_from(edge)
            .unwrap_or(layer_auto_hide::layer_auto_hide_v1::Edge::Bottom);
        let mode_enum = layer_auto_hide::layer_auto_hide_v1::Mode::try_from(mode)
            .unwrap_or(layer_auto_hide::layer_auto_hide_v1::Mode::Always);
        let mode_enum = if mode_enum == layer_auto_hide::layer_auto_hide_v1::Mode::OnFullscreen
            && self
                .auto_hide_manager
                .as_ref()
                .is_some_and(|manager| manager.version() < 2)
        {
            log::warn!(
                "Fullscreen-only auto-hide requires protocol v2; falling back to on-maximize"
            );
            layer_auto_hide::layer_auto_hide_v1::Mode::OnMaximize
        } else {
            mode_enum
        };

        // Check if we already have an auto-hide object for this surface
        if let Some(auto_hide_obj) = self.auto_hide_surfaces.get(&surface_id) {
            auto_hide_obj.set_auto_hide(edge_enum, edge_zone, mode_enum);
            surface.commit();
            return;
        }

        // Need to create a new auto-hide object
        if let Some(manager) = &self.auto_hide_manager {
            let auto_hide_data = layer_auto_hide::LayerAutoHideData {
                surface: surface.clone(),
            };
            // Get a queue handle from the first unit (they all share the same queue)
            if let Some(unit) = self.units.first() {
                let auto_hide_obj = manager.get_auto_hide(surface, &unit.qh, auto_hide_data);
                auto_hide_obj.set_auto_hide(edge_enum, edge_zone, mode_enum);
                self.auto_hide_surfaces.insert(surface_id, auto_hide_obj);
                surface.commit();
            }
        } else {
            log::warn!(
                "Auto-hide manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Disable compositor-driven auto-hide for a specific surface.
    pub fn unset_auto_hide_for_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();

        if let Some(auto_hide_obj) = self.auto_hide_surfaces.get(&surface_id) {
            auto_hide_obj.unset_auto_hide();
            surface.commit();
        }
    }

    /// Read the clipboard selection as local file paths, for a paste.
    ///
    /// `None` when the clipboard holds no file list at all — nothing was ever
    /// offered, or what is offered is not `text/uri-list` (plain text, say,
    /// which the app's own clipboard already handles). An offered-but-empty
    /// list comes back as `Some(vec![])`.
    ///
    /// Mirrors the drop path: the source writes the URI list to a pipe we hand
    /// it, and we parse the paths out. The read blocks with the same short
    /// timeout — a cooperating source writes immediately, so the timeout only
    /// bounds the damage from one that does not.
    ///
    /// Note the selection is only offered to a client the compositor considers
    /// focused, so this reads what was on the clipboard when this surface took
    /// focus, which is exactly what a paste into it should see.
    pub fn read_selection_uri_list(&self) -> Option<Vec<std::path::PathBuf>> {
        use std::io::Read;
        use std::os::fd::AsFd;

        let Some((offer, mimes)) = self.selection_offer.as_ref() else {
            log::info!(target: "kcopy_dnd", "read_selection_uri_list: no selection offer held");
            return None;
        };
        if !mimes.iter().any(|mime| mime == URI_LIST_MIME) {
            log::info!(target: "kcopy_dnd", "read_selection_uri_list: no uri-list among {mimes:?}");
            return None;
        }
        // Taken from the offer rather than `self.connection`, which is only ever
        // populated by the optional `with_connection` builder — iced_layershell
        // leaves it `None`, so reading it here silently found nothing to flush.
        let Some(backend) = offer.backend().upgrade() else {
            log::info!(target: "kcopy_dnd", "read_selection_uri_list: offer backend is gone");
            return None;
        };
        let connection = Connection::from_backend(backend);

        let (mut reader, writer) = std::os::unix::net::UnixStream::pair().ok()?;
        offer.receive(URI_LIST_MIME.to_string(), writer.as_fd());
        let _ = connection.flush();
        // Our own end of the pipe must close, or the read below blocks until the
        // timeout even after the source has written everything.
        drop(writer);

        let _ = reader.set_read_timeout(Some(URI_LIST_READ_TIMEOUT));
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);

        let paths = parse_uri_list(&buf);
        log::info!(target: "kcopy_dnd", "read_selection_uri_list: {} bytes -> {} path(s)", buf.len(), paths.len());
        Some(paths)
    }

    /// Begin an outgoing Wayland drag-and-drop from the pointer-focused surface,
    /// offering `mime_types` with the parallel pre-serialized `data`, advertising
    /// the given DnD action `bits`. No custom drag icon for now — the compositor
    /// renders its default. Source-side completion events route back to that
    /// surface so the app can finish the operation.
    pub fn start_drag(
        &mut self,
        mime_types: Vec<String>,
        actions: u32,
        data: Vec<Vec<u8>>,
        icon: Option<DndIconPixels>,
        hotspot: Option<(i32, i32)>,
    ) where
        T: 'static,
    {
        let Some(surface) = self.current_surface.clone() else {
            log::warn!(target: "kcopy_dnd", "start_drag: no focused surface");
            return;
        };
        let Some(manager) = self.data_device_manager.clone() else {
            log::warn!(target: "kcopy_dnd", "start_drag: no data_device_manager");
            return;
        };
        let Some(device) = self.data_device.clone() else {
            log::warn!(target: "kcopy_dnd", "start_drag: no data_device");
            return;
        };
        let Some(serial) = self.last_button_serial else {
            log::warn!(target: "kcopy_dnd", "start_drag: no button serial yet");
            return;
        };
        let qh = match self.queue_handle.as_ref() {
            Some(qh) => qh.clone(),
            None => {
                log::warn!(target: "kcopy_dnd", "start_drag: queue_handle not initialized");
                return;
            }
        };

        let source = manager.create_data_source(
            &qh,
            DndSourceData {
                mime_types: mime_types.clone(),
                data,
            },
        );
        for mime in &mime_types {
            source.offer(mime.clone());
        }
        if source.version() >= 3 {
            source.set_actions(DndAction::from_bits_truncate(actions));
        }
        self.dnd_source_origin = self.current_surface_id();
        let icon_surface = icon.and_then(|i| self.build_dnd_icon(i));
        log::info!(target: "kcopy_dnd", "start_drag mimes={mime_types:?} serial={serial} icon={}", icon_surface.is_some());
        device.start_drag(Some(&source), &surface, icon_surface.as_ref(), serial);

        // Apply the icon hotspot now that the dnd_icon role is assigned: re-commit
        // the icon surface with a buffer offset so the grabbed point of the icon
        // stays under the cursor (the compositor renders the icon at
        // `cursor + offset`). Done post-`start_drag` because the offset is only
        // recorded from commits made after the surface has the dnd_icon role.
        if let (Some((hx, hy)), Some(res)) = (hotspot, self.dnd_icon.as_ref())
            && (hx != 0 || hy != 0)
        {
            {
                if res.surface.version() >= 5 {
                    res.surface.attach(Some(&res.buffer), 0, 0);
                    res.surface.offset(-hx, -hy);
                } else {
                    res.surface.attach(Some(&res.buffer), -hx, -hy);
                }
                res.surface.damage_buffer(0, 0, res.width, res.height);
                res.surface.commit();
            }
        }
    }

    /// Redraw the icon of the drag in flight, keeping where it sits under the
    /// pointer. Does nothing when no drag with an icon is in flight.
    pub fn update_drag_icon(&mut self, icon: DndIconPixels)
    where
        T: 'static,
    {
        let Some((file, pool, buffer)) = self.dnd_icon_buffer(&icon) else {
            return;
        };
        let Some(res) = self.dnd_icon.as_mut() else {
            buffer.destroy();
            pool.destroy();
            return;
        };
        if icon.scale > 1 {
            res.surface.set_buffer_scale(icon.scale);
        }
        // A zero offset leaves the icon where it is: the compositor adds it to the current one.
        res.surface.attach(Some(&buffer), 0, 0);
        res.surface
            .damage_buffer(0, 0, icon.width as i32, icon.height as i32);
        res.surface.commit();
        let old_buffer = std::mem::replace(&mut res.buffer, buffer);
        let old_pool = std::mem::replace(&mut res.pool, pool);
        res._file = file;
        res.width = icon.width as i32;
        res.height = icon.height as i32;
        old_buffer.destroy();
        old_pool.destroy();
    }

    /// An SHM buffer holding `icon`'s pre-multiplied ARGB pixels.
    fn dnd_icon_buffer(&self, icon: &DndIconPixels) -> Option<(std::fs::File, WlShmPool, WlBuffer)>
    where
        T: 'static,
    {
        use std::io::Write;
        use std::os::fd::AsFd;
        let DndIconPixels {
            width,
            height,
            argb,
            ..
        } = icon;
        let (width, height) = (*width, *height);
        let expected = (width * height * 4) as usize;
        if width == 0 || height == 0 || argb.len() < expected {
            return None;
        }
        let qh = self.queue_handle.as_ref()?;
        let shm = self.cached_shm.as_ref()?;

        let mut file = tempfile::tempfile().ok()?;
        file.write_all(&argb[..expected]).ok()?;
        let pool = shm.create_pool(file.as_fd(), expected as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            (width * 4) as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
            qh,
            (),
        );
        Some((file, pool, buffer))
    }

    /// Build the drag-icon `wl_surface` from pre-multiplied ARGB pixels, keeping
    /// its buffer/pool/file alive in `self.dnd_icon` for the drag's duration.
    fn build_dnd_icon(&mut self, icon: DndIconPixels) -> Option<WlSurface>
    where
        T: 'static,
    {
        let (file, pool, buffer) = self.dnd_icon_buffer(&icon)?;
        let DndIconPixels {
            width,
            height,
            scale,
            ..
        } = icon;
        let qh = self.queue_handle.as_ref()?.clone();
        let compositor = self.cached_compositor.clone()?;
        let surface = compositor.create_surface(&qh, ());
        if scale > 1 {
            surface.set_buffer_scale(scale);
        }
        // Attach at the origin. The hotspot offset is applied *after*
        // `start_drag` assigns the dnd_icon role — the compositor only records the
        // icon offset from commits made once the surface has that role, so an
        // offset committed here (pre-role) would be ignored.
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();

        if let Some(old) = self.dnd_icon.take() {
            old.destroy();
        }
        let surface_clone = surface.clone();
        self.dnd_icon = Some(DndIconResources {
            surface,
            buffer,
            pool,
            _file: file,
            width: width as i32,
            height: height as i32,
        });
        Some(surface_clone)
    }

    /// Whether the surface `id` is hidden with the `layer_surface_visibility`
    /// protocol (`hide_surface`): the compositor neither draws it nor gives it
    /// input, though it lives on.
    pub fn is_hidden(&self, id: id::Id) -> bool {
        self.get_unit_with_id(id).is_some_and(|unit| {
            self.hidden_surfaces
                .contains(&unit.wl_surface.id().protocol_id())
        })
    }

    /// Returns `true` when every live surface unit is currently hidden via the
    /// `layer_surface_visibility` protocol.  When this returns `true` the
    /// compositor is not rendering us, so we can save CPU by throttling updates.
    pub fn all_surfaces_hidden(&self) -> bool {
        !self.units.is_empty()
            && self.units.iter().all(|u| {
                self.hidden_surfaces
                    .contains(&u.wl_surface.id().protocol_id())
            })
    }

    /// Run another pass shortly, for work that can't finish yet (a surface still
    /// joining, a compositor still being made). The loop only wakes for events,
    /// so without this nothing would come back to it.
    pub fn wake_soon(&mut self) {
        self.retry_requested = true;
    }

    /// The surfaces a pass would present, as its refresh loop picks them.
    fn presentable_units(&self) -> impl Iterator<Item = &WindowStateUnit<T>> {
        let all_hidden = self.all_surfaces_hidden();
        self.units.iter().filter(move |unit| {
            let (width, height) = unit.size;
            width != 0
                && height != 0
                && unit.configured
                && !(all_hidden && unit.initial_refresh_sent)
                && unit.present_available_state == PresentAvailableState::Available
        })
    }

    /// Whether a surface can present now.
    fn has_due_frame(&self) -> bool {
        self.presentable_units().any(|unit| unit.should_refresh())
    }

    /// Whether the next pass must run now. A surface or exit asked for by a user
    /// event waits in `return_data`, and only a pass makes it.
    fn pass_due_now(&self, woke_by_events: bool) -> bool {
        let queued = !self.message.is_empty() && self.retry_timer.is_none();
        let faded = self
            .next_fade_close()
            .is_some_and(|at| at <= Instant::now());
        woke_by_events || queued || faded || !self.return_data.is_empty() || self.has_due_frame()
    }

    /// The earliest timed redraw still to come.
    fn next_timed_refresh(&self) -> Option<Instant> {
        let now = Instant::now();
        self.presentable_units()
            .filter_map(|unit| match unit.refresh_request() {
                RefreshRequest::At(at) if at > now => Some(at),
                _ => None,
            })
            .chain(self.next_fade_close().filter(|at| *at > now))
            .min()
    }

    /// Put a layer surface in a Kora workspace: the compositor draws,
    /// hit-tests and captures it only while that workspace is on screen, as
    /// it does the workspace's own windows (`kora_workspace_realm_v1` v2).
    /// Only a machine-plane client may; a client inside a workspace has its
    /// surfaces there already. Returns whether the compositor offers it.
    pub fn assign_realm(&mut self, surface: &WlSurface, workspace: &str) -> bool {
        #[cfg(feature = "workspaces")]
        if let Some(manager) = &self.workspaces.realm_manager {
            if wayland_client::Proxy::version(manager) >= 2 {
                manager.assign(surface, workspace.to_string());
                if let Some(ref conn) = self.connection {
                    let _ = conn.flush();
                }
                log::info!(
                    "Assigned surface {} to workspace {workspace}",
                    surface.id().protocol_id()
                );
                return true;
            }
            log::warn!("kora_workspace_realm_manager_v1 is v1 here: no surface assignment");
            return false;
        }
        #[cfg(not(feature = "workspaces"))]
        let _ = (surface, workspace);
        false
    }

    /// Hide the popup `id` and destroy it once `fade` has passed, so the compositor fades it
    /// out instead of dropping it. Returns `false`, and does nothing, for a popup that holds
    /// the grab or was never configured, or without the visibility protocol: close those.
    pub fn fade_out_popup(&mut self, id: id::Id, fade: Duration) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
            return false;
        };
        let unit = &self.units[index];
        let fades = unit.configured
            && unit.popup_placement.as_ref().is_some_and(|p| !p.grab)
            && self.layer_surface_visibility_manager.is_some();
        if !fades {
            return false;
        }
        let surface = unit.wl_surface.clone();
        self.hide_surface(&surface);
        self.units[index].fade_close = Some(Instant::now() + fade);
        true
    }

    /// Mark the popups whose fade-out is over, and the popups of a surface being closed, for
    /// closing. Returns what to close: popups before the surface they open from.
    fn closing_order(&mut self) -> Vec<id::Id> {
        let now = Instant::now();
        for unit in &mut self.units {
            if unit.fade_close.is_some_and(|at| at <= now) {
                unit.request_flag.close = true;
            }
        }
        let closing: Vec<id::Id> = self
            .units
            .iter()
            .filter(|unit| unit.request_flag.close)
            .map(WindowStateUnit::id)
            .collect();
        let mut order: Vec<id::Id> = self
            .units
            .iter()
            .filter(|unit| {
                !unit.request_flag.close
                    && unit
                        .popup_placement
                        .as_ref()
                        .is_some_and(|p| closing.contains(&p.parent))
            })
            .map(WindowStateUnit::id)
            .collect();
        let (popups, others): (Vec<_>, Vec<_>) = closing
            .into_iter()
            .partition(|id| self.units.iter().any(|u| u.id == *id && u.is_popup()));
        order.extend(popups);
        order.extend(others);
        order
    }

    /// The next popup fade-out to end.
    fn next_fade_close(&self) -> Option<Instant> {
        self.units.iter().filter_map(|unit| unit.fade_close).min()
    }

    /// Hide a surface without destroying it (using layer_surface_visibility protocol)
    /// The surface will not be rendered and won't receive input events.
    /// Use show_surface to make it visible again.
    pub fn hide_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();

        // Track locally immediately so the timer can throttle before the
        // compositor round-trip confirms.
        self.hidden_surfaces.insert(surface_id);

        // Check if we already have a visibility controller for this surface
        if let Some(controller) = self.layer_surface_visibility_controllers.get(&surface_id) {
            controller.set_hidden();
            // Flush to ensure compositor processes immediately
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!("Hidden surface {} (existing controller)", surface_id);
            return;
        }

        // Need to create a new controller
        let transition = self.surface_transition(surface_id);
        if let Some(manager) = &self.layer_surface_visibility_manager {
            if let Some(unit) = self.units.first() {
                let visibility_data = layer_surface_visibility::LayerSurfaceVisibilityData {
                    surface: surface.clone(),
                    hidden: true,
                };
                let controller =
                    manager.get_visibility_controller(surface, &unit.qh, visibility_data);
                apply_transition_to_controller(&controller, transition);
                controller.set_hidden();
                self.layer_surface_visibility_controllers
                    .insert(surface_id, controller);
                // Flush to ensure compositor processes immediately
                if let Some(ref conn) = self.connection {
                    let _ = conn.flush();
                }
                log::info!("Hidden surface {} (new controller)", surface_id);
            }
        } else {
            log::warn!(
                "Layer surface visibility manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Show a surface that was previously hidden
    pub fn show_surface(&mut self, surface: &WlSurface) {
        let surface_id = surface.id().protocol_id();

        // Track locally immediately so the timer wakes up without waiting
        // for the compositor round-trip.
        self.hidden_surfaces.remove(&surface_id);

        // Request a refresh on the affected unit so the very next timer tick
        // renders a fresh frame, instead of waiting for the compositor's
        // visibility_changed round-trip to trigger one.
        for unit in &mut self.units {
            if unit.wl_surface.id().protocol_id() == surface_id {
                unit.request_refresh(RefreshRequest::NextFrame);
                break;
            }
        }

        // Check if we have a visibility controller for this surface
        if let Some(controller) = self.layer_surface_visibility_controllers.get(&surface_id) {
            controller.set_visible();
            // Flush to ensure compositor processes immediately
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!("Shown surface {} (existing controller)", surface_id);
            return;
        }

        // Need to create a new controller (surface was never hidden, but make it explicit)
        let transition = self.surface_transition(surface_id);
        if let Some(manager) = &self.layer_surface_visibility_manager {
            if let Some(unit) = self.units.first() {
                let visibility_data = layer_surface_visibility::LayerSurfaceVisibilityData {
                    surface: surface.clone(),
                    hidden: false,
                };
                let controller =
                    manager.get_visibility_controller(surface, &unit.qh, visibility_data);
                apply_transition_to_controller(&controller, transition);
                controller.set_visible();
                self.layer_surface_visibility_controllers
                    .insert(surface_id, controller);
                // Flush to ensure compositor processes immediately
                if let Some(ref conn) = self.connection {
                    let _ = conn.flush();
                }
                log::info!("Shown surface {} (new controller)", surface_id);
            }
        } else {
            log::warn!(
                "Layer surface visibility manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Get or create a dismiss controller for a surface
    /// Returns the controller if available, or None if the protocol is not supported
    fn get_or_create_dismiss_controller(
        &mut self,
        surface: &WlSurface,
    ) -> Option<layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::ZcosmicLayerSurfaceDismissV1>
    {
        let surface_id = surface.id().protocol_id();

        // Check if we already have a controller for this surface
        if let Some(controller) = self.layer_surface_dismiss_controllers.get(&surface_id) {
            return Some(controller.clone());
        }

        // Need to create a new controller
        let manager = self.layer_surface_dismiss_manager.as_ref()?;
        let unit = self.units.first()?;

        let dismiss_data = layer_surface_dismiss::LayerSurfaceDismissData {
            surface: surface.clone(),
        };
        let controller = manager.get_dismiss_controller(surface, &unit.qh, dismiss_data);
        self.layer_surface_dismiss_controllers
            .insert(surface_id, controller.clone());
        log::debug!("Created dismiss controller for surface {}", surface_id);
        Some(controller)
    }

    /// Arm dismiss notifications for a surface
    /// Once armed, a dismiss_requested event will be sent when the user clicks/touches
    /// outside the surface's dismiss group.
    pub fn arm_dismiss(&mut self, surface: &WlSurface) {
        if let Some(controller) = self.get_or_create_dismiss_controller(surface) {
            controller.arm();
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!("Armed dismiss for surface {}", surface.id().protocol_id());
        } else {
            log::warn!(
                "Layer surface dismiss manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Disarm dismiss notifications for a surface
    /// The surface will no longer receive dismiss_requested events.
    pub fn disarm_dismiss(&mut self, surface: &WlSurface) {
        if let Some(controller) = self.get_or_create_dismiss_controller(surface) {
            controller.disarm();
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!(
                "Disarmed dismiss for surface {}",
                surface.id().protocol_id()
            );
        } else {
            log::warn!(
                "Layer surface dismiss manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Opt a surface's dismiss controller out of dismissing on clicks that land
    /// on OTHER layer-shell surfaces (panels, docks, popovers). Clicks on app
    /// content or the bare desktop still dismiss. Needs the compositor's dismiss
    /// protocol v2 — a no-op (with a log) on older compositors that only offer v1.
    pub fn set_dismiss_ignore_layer_clicks(&mut self, surface: &WlSurface) {
        if let Some(controller) = self.get_or_create_dismiss_controller(surface) {
            if controller.version() >= 2 {
                controller.set_ignore_layer_clicks();
                if let Some(ref conn) = self.connection {
                    let _ = conn.flush();
                }
                log::info!(
                    "Dismiss controller for surface {} ignores layer-shell clicks",
                    surface.id().protocol_id()
                );
            } else {
                log::warn!(
                    "set_ignore_layer_clicks needs dismiss protocol v2; compositor offers v1"
                );
            }
        } else {
            log::warn!(
                "Layer surface dismiss manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Add a surface to the dismiss group of another surface
    /// When the popup_surface is armed, clicks outside both surfaces will trigger dismiss.
    /// The group_surface is typically the parent panel bar.
    pub fn add_to_dismiss_group(&mut self, popup_surface: &WlSurface, group_surface: &WlSurface) {
        if let Some(controller) = self.get_or_create_dismiss_controller(popup_surface) {
            controller.add_to_group(group_surface);
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!(
                "Added surface {} to dismiss group of popup surface {}",
                group_surface.id().protocol_id(),
                popup_surface.id().protocol_id()
            );
        } else {
            log::warn!(
                "Layer surface dismiss manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Remove a surface from the dismiss group of another surface
    pub fn remove_from_dismiss_group(
        &mut self,
        popup_surface: &WlSurface,
        group_surface: &WlSurface,
    ) {
        if let Some(controller) = self.get_or_create_dismiss_controller(popup_surface) {
            controller.remove_from_group(group_surface);
            if let Some(ref conn) = self.connection {
                let _ = conn.flush();
            }
            log::info!(
                "Removed surface {} from dismiss group of popup surface {}",
                group_surface.id().protocol_id(),
                popup_surface.id().protocol_id()
            );
        } else {
            log::warn!(
                "Layer surface dismiss manager not available - compositor may not support this protocol"
            );
        }
    }

    /// Check if dismiss was requested and clear the flag
    /// Returns true if a dismiss was requested since the last check.
    pub fn take_dismiss_requested(&mut self) -> bool {
        std::mem::take(&mut self.dismiss_requested)
    }
}

pub trait ZwpTextInputV3Ext {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose);

    fn set_surrounding(&self, surrounding: &SurroundingText);
}

impl ZwpTextInputV3Ext for ZwpTextInputV3 {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose) {
        let (hint, purpose) = match purpose {
            ImePurpose::Normal => (ContentHint::None, ContentPurpose::Normal),
            ImePurpose::Password => (ContentHint::SensitiveData, ContentPurpose::Password),
            ImePurpose::Terminal => (ContentHint::None, ContentPurpose::Terminal),
        };
        self.set_content_type(hint, purpose);
    }

    fn set_surrounding(&self, surrounding: &SurroundingText) {
        self.set_surrounding_text(
            surrounding.text.clone(),
            surrounding.cursor as i32,
            surrounding.anchor as i32,
        );
    }
}

impl WindowWrapper {
    #[inline]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandWindowHandle::new({
            let ptr = self.wl_surface.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        })
        .into())
    }

    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        })
        .into())
    }
}
impl rwh_06::HasWindowHandle for WindowWrapper {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::WindowHandle::borrow_raw(raw) })
    }
}

impl rwh_06::HasDisplayHandle for WindowWrapper {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;

        // SAFETY: The window handle will never be deallocated while the window is alive,
        // and the main thread safety requirements are upheld internally by each platform.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}

/// Apply blur effect to a surface using the KDE blur protocol
fn apply_blur_to_surface<T: 'static>(
    blur_manager: &Option<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager>,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
    blur_radius: Option<f32>,
    blur_saturation: Option<f32>,
    blur_tint: Option<f32>,
    blur_border: Option<f32>,
) {
    if let Some(manager) = blur_manager {
        let blur_data = blur::BlurData {
            surface: surface.clone(),
        };
        let blur_obj = manager.create(surface, qh, blur_data);
        // Set region to null (entire surface)
        blur_obj.set_region(None);
        apply_blur_params(
            &blur_obj,
            blur_radius,
            blur_saturation,
            blur_tint,
            blur_border,
        );
        // Commit the blur effect
        blur_obj.commit();
        log::info!(
            "Applied blur effect to layer shell surface (radius={:?}, saturation={:?}, tint={:?}, border={:?})",
            blur_radius,
            blur_saturation,
            blur_tint,
            blur_border
        );
    }
}

/// Send the version-gated frosted-glass parameters (radius is v2+, saturation /
/// tint / border are v3+) on an already-created, not-yet-committed blur object.
///
/// Shared by initial creation ([`apply_blur_to_surface`]) and the blur
/// re-enable path ([`WindowState::set_blur_for_surface`]) so BOTH keep the
/// requested values. Without this on the re-enable path, an auto-sized surface
/// whose effects were deferred (the panel popovers) would come back with the
/// compositor's default tint — a white wash over the blur.
fn apply_blur_params(
    blur_obj: &blur::org_kde_kwin_blur::OrgKdeKwinBlur,
    blur_radius: Option<f32>,
    blur_saturation: Option<f32>,
    blur_tint: Option<f32>,
    blur_border: Option<f32>,
) {
    if let Some(radius) = blur_radius {
        if blur_obj.version() >= 2 {
            blur_obj.set_radius((radius * 256.0) as i32);
        } else {
            log::warn!(
                "Blur radius requested but compositor only supports blur protocol v{}, ignoring",
                blur_obj.version()
            );
        }
    }
    if let Some(saturation) = blur_saturation {
        if blur_obj.version() >= 3 {
            blur_obj.set_saturation((saturation * 256.0) as i32);
        } else {
            log::warn!(
                "Blur saturation requested but compositor only supports blur protocol v{}, ignoring",
                blur_obj.version()
            );
        }
    }
    if let Some(tint) = blur_tint {
        if blur_obj.version() >= 3 {
            blur_obj.set_tint((tint * 256.0) as i32);
        } else {
            log::warn!(
                "Blur tint requested but compositor only supports blur protocol v{}, ignoring",
                blur_obj.version()
            );
        }
    }
    if let Some(border) = blur_border {
        if blur_obj.version() >= 3 {
            blur_obj.set_border((border * 256.0) as i32);
        } else {
            log::warn!(
                "Blur border requested but compositor only supports blur protocol v{}, ignoring",
                blur_obj.version()
            );
        }
    }
}

/// Apply corner radius to a surface using the layer corner radius protocol
/// Returns the corner radius surface object so it can be stored for later updates
fn apply_corner_radius_to_surface<T: 'static>(
    corner_radius_manager: &Option<
        corner_radius::layer_corner_radius_manager_v1::LayerCornerRadiusManagerV1,
    >,
    corner_radius_values: Option<[u32; 4]>,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
) -> Option<corner_radius::layer_corner_radius_surface_v1::LayerCornerRadiusSurfaceV1> {
    if let (Some(manager), Some(radii)) = (corner_radius_manager, corner_radius_values) {
        let corner_data = corner_radius::CornerRadiusData {
            surface: surface.clone(),
        };
        let corner_obj = manager.get_corner_radius(surface, qh, corner_data);
        corner_obj.set_radius(radii[0], radii[1], radii[2], radii[3]);
        log::info!("Applied corner radius to layer shell surface: {:?}", radii);
        Some(corner_obj)
    } else {
        None
    }
}

/// How a popup asks the compositor to place it as a tooltip; all `None` when it does not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TooltipPlacement {
    offset: Option<(i32, i32)>,
    anchor: Option<u32>,
    delay_ms: Option<u32>,
}

/// Give `surface` a compositor-driven tooltip over `parent` when its placement asks for one.
/// The manager is bound from the event loop's globals, since the window state's copy is
/// taken when the loop starts.
fn attach_tooltip<T: 'static>(
    manager: &mut Option<tooltip::zcosmic_tooltip_manager_v1::ZcosmicTooltipManagerV1>,
    globals: &GlobalList,
    qh: &QueueHandle<WindowState<T>>,
    surface: &WlSurface,
    parent: &WlSurface,
    placement: TooltipPlacement,
) -> Option<tooltip::zcosmic_tooltip_v1::ZcosmicTooltipV1> {
    if placement == TooltipPlacement::default() {
        return None;
    }
    if manager.is_none() {
        *manager = globals
            .bind::<tooltip::zcosmic_tooltip_manager_v1::ZcosmicTooltipManagerV1, _, _>(
                qh,
                1..=1,
                (),
            )
            .ok();
        if manager.is_some() {
            log::info!("Bound tooltip manager");
        }
    }
    let Some(manager) = manager.as_ref() else {
        log::warn!("Tooltip manager not available — compositor may not support tooltips");
        return None;
    };
    let data = tooltip::TooltipData {
        tooltip_surface: surface.clone(),
        parent_surface: parent.clone(),
    };
    let tooltip = manager.get_tooltip(surface, parent, qh, data);
    if let Some((x, y)) = placement.offset {
        tooltip.set_offset(x, y);
    }
    if let Some(anchor) = placement
        .anchor
        .and_then(|a| tooltip::zcosmic_tooltip_v1::Anchor::try_from(a).ok())
    {
        tooltip.set_anchor(anchor);
    }
    if let Some(delay) = placement.delay_ms {
        tooltip.set_show_delay(delay);
    }
    Some(tooltip)
}

/// Apply shadow to a surface using the layer shadow protocol
fn apply_shadow_to_surface<T: 'static>(
    shadow_manager: &Option<shadow::layer_shadow_manager_v1::LayerShadowManagerV1>,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
) {
    log::debug!(
        "apply_shadow_to_surface called, manager present: {}",
        shadow_manager.is_some()
    );
    if let Some(manager) = shadow_manager {
        let shadow_data = shadow::ShadowData {
            surface: surface.clone(),
        };
        let shadow_obj = manager.get_shadow(surface, qh, shadow_data);
        shadow_obj.enable();
        log::info!(
            "Applied shadow effect to layer shell surface (surface_id: {})",
            surface.id().protocol_id()
        );
    } else {
        log::warn!("Cannot apply shadow: shadow_manager is None");
    }
}

/// Register a surface for compositor usable-area reporting. Returns the object
/// so it can be stored (and destroyed on surface teardown). No-op when the
/// compositor lacks the protocol.
fn register_usable_area_for_surface<T: 'static>(
    usable_area_manager: &Option<
        layer_usable_area::layer_usable_area_manager_v1::LayerUsableAreaManagerV1,
    >,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
) -> Option<layer_usable_area::layer_usable_area_v1::LayerUsableAreaV1> {
    let manager = usable_area_manager.as_ref()?;
    let data = layer_usable_area::LayerUsableAreaData {
        surface: surface.clone(),
    };
    Some(manager.get_usable_area(surface, qh, data))
}

/// Ask for size transitions on `surface`. No-op without the protocol.
fn register_size_transition_for_surface<T: 'static>(
    manager: &Option<
        layer_size_transition::layer_size_transition_manager_v1::LayerSizeTransitionManagerV1,
    >,
    surface: &WlSurface,
    qh: &QueueHandle<WindowState<T>>,
) -> Option<layer_size_transition::layer_size_transition_v1::LayerSizeTransitionV1> {
    let manager = manager.as_ref()?;
    let data = layer_size_transition::LayerSizeTransitionData {
        surface: surface.clone(),
    };
    Some(manager.get_size_transition(surface, qh, data))
}

/// Apply a show/hide transition preference to a freshly-created visibility
/// controller.  `set_transition` was added in version 2 of the protocol, so
/// this is a no-op (logged) when the compositor only supports version 1.
fn apply_transition_to_controller(
    controller: &layer_surface_visibility::zcosmic_layer_surface_visibility_v1::ZcosmicLayerSurfaceVisibilityV1,
    transition: Option<LayerTransition>,
) {
    use layer_surface_visibility::zcosmic_layer_surface_visibility_v1::Transition as ProtoTransition;
    let Some(transition) = transition else {
        return;
    };
    if controller.version() < 2 {
        log::debug!(
            "Compositor layer_surface_visibility < v2; ignoring transition request {:?}",
            transition
        );
        return;
    }
    let role = |proto| {
        if controller.version() >= 5 {
            proto
        } else {
            ProtoTransition::Fade
        }
    };
    let proto = match transition {
        LayerTransition::Slide => ProtoTransition::Slide,
        LayerTransition::Fade => ProtoTransition::Fade,
        LayerTransition::FluidReveal => ProtoTransition::FluidReveal,
        LayerTransition::Popover => role(ProtoTransition::Popover),
        LayerTransition::Panel => role(ProtoTransition::Panel),
        LayerTransition::ControlPanel => role(ProtoTransition::ControlPanel),
        LayerTransition::Launcher => role(ProtoTransition::Launcher),
        LayerTransition::Spotlight => role(ProtoTransition::Spotlight),
        LayerTransition::Notification => role(ProtoTransition::Notification),
        LayerTransition::ContextMenu => role(ProtoTransition::ContextMenu),
        LayerTransition::Modal => role(ProtoTransition::Modal),
    };
    controller.set_transition(proto);
    log::debug!(
        "Applied transition {:?} to visibility controller",
        transition
    );
}

impl<T> WindowState<T> {
    /// create a WindowState, you need to pass a namespace in
    pub fn new(namespace: &str) -> Self {
        assert_ne!(namespace, "");
        Self {
            namespace: namespace.to_owned(),
            ..Default::default()
        }
    }

    /// suggest to bind to specific output
    /// if there is no such output , it will bind the output which now is focused,
    /// same with when binded_output_name is None
    pub fn with_xdg_output_name(mut self, binded_output_name: String) -> Self {
        self.start_mode = StartMode::TargetScreen(binded_output_name);
        self
    }

    pub fn with_start_mode(mut self, mode: StartMode) -> Self {
        self.start_mode = mode;
        self
    }

    pub fn with_events_transparent(mut self, transparent: bool) -> Self {
        self.events_transparent = transparent;
        self
    }

    /// Request blur effect for surfaces (requires compositor support for org_kde_kwin_blur)
    pub fn with_blur(mut self, blur: bool) -> Self {
        self.blur = blur;
        self
    }

    /// Set a custom blur radius in pixels (requires org_kde_kwin_blur version 2).
    /// `None` leaves the compositor default.
    pub fn with_blur_radius(mut self, blur_radius: Option<f32>) -> Self {
        self.blur_radius = blur_radius;
        self
    }

    /// Set the backdrop saturation multiplier for the blur, matching CSS
    /// `backdrop-filter: saturate(x)` (requires org_kde_kwin_blur version 3).
    /// `None` applies no saturation change.
    pub fn with_blur_saturation(mut self, blur_saturation: Option<f32>) -> Self {
        self.blur_saturation = blur_saturation;
        self
    }

    /// Set the frosted-glass white-tint strength for the blur (requires
    /// org_kde_kwin_blur version 3). `Some(0.0)` disables the compositor tint
    /// (faithful backdrop-filter); `None` keeps the compositor default.
    pub fn with_blur_tint(mut self, blur_tint: Option<f32>) -> Self {
        self.blur_tint = blur_tint;
        self
    }

    /// Set the frosted-glass border strength for the blur (requires
    /// org_kde_kwin_blur version 3). `Some(0.0)` disables the compositor's 1px
    /// border; `None` keeps the compositor default.
    pub fn with_blur_border(mut self, blur_border: Option<f32>) -> Self {
        self.blur_border = blur_border;
        self
    }

    /// Set corner radius for surfaces (requires compositor support for layer_corner_radius_manager_v1)
    /// Radii are specified as [top_left, top_right, bottom_right, bottom_left]
    pub fn with_corner_radius(mut self, radii: [u32; 4]) -> Self {
        self.corner_radius = Some(radii);
        self
    }

    /// Request shadow effect for surfaces (requires compositor support for layer_shadow_manager_v1)
    pub fn with_shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
        self
    }

    /// Returns whether shadow is enabled for this window state.
    pub fn has_shadow(&self) -> bool {
        self.shadow
    }

    /// Set the global show/hide transition animation for surfaces (requires
    /// compositor support for `zcosmic_layer_surface_visibility` version 2).
    /// `None` lets the compositor decide based on the surface anchor.  A
    /// per-surface [`NewLayerShellSettings::transition`] overrides this.
    pub fn with_transition(mut self, transition: Option<LayerTransition>) -> Self {
        self.transition = transition;
        self
    }

    /// Effective transition for a surface: its per-surface override if set,
    /// otherwise the global transition.
    /// Use `transition` for the surface's hides and shows from now on.
    pub fn set_surface_transition(&mut self, surface: &WlSurface, transition: LayerTransition) {
        let surface_id = surface.id().protocol_id();
        self.transitions.insert(surface_id, transition);
        if let Some(controller) = self.layer_surface_visibility_controllers.get(&surface_id) {
            apply_transition_to_controller(controller, Some(transition));
        }
    }

    fn surface_transition(&self, surface_id: u32) -> Option<LayerTransition> {
        self.transitions
            .get(&surface_id)
            .copied()
            .or(self.transition)
    }

    /// Enable foreign toplevel tracking (requires compositor support for zwlr_foreign_toplevel_manager_v1)
    /// When enabled, events will be sent for all opened windows (toplevels) on the system.
    /// Useful for creating taskbars or docks that need to show running applications.
    /// This method is only available when the `foreign-toplevel` feature is enabled.
    #[cfg(feature = "foreign-toplevel")]
    pub fn with_foreign_toplevel(mut self, enabled: bool) -> Self {
        self.foreign_toplevel_enabled = enabled;
        self
    }

    /// Register surfaces to receive the device's special key.
    ///
    /// `is_default` also makes the first registered surface the fallback
    /// receiver, used whenever no registered surface is focused — a home screen
    /// wants that, a chat window does not.
    pub fn with_special_action(mut self, enabled: bool, is_default: bool) -> Self {
        self.special_action = enabled;
        self.special_action_default = is_default;
        self
    }

    /// if the shell is a single one, only display on one screen,
    /// fi true, the layer will binding to current screen
    pub fn with_active(mut self) -> Self {
        self.start_mode = StartMode::Active;
        self
    }

    pub fn with_active_or_xdg_output_name(self, binded_output_name: Option<String>) -> Self {
        match binded_output_name {
            Some(binded_output_name) => self.with_xdg_output_name(binded_output_name),
            None => self.with_active(),
        }
    }

    pub fn with_allscreens_or_xdg_output_name(self, binded_output_name: Option<String>) -> Self {
        match binded_output_name {
            Some(binded_output_name) => self.with_xdg_output_name(binded_output_name),
            None => self.with_allscreens(),
        }
    }
    pub fn with_xdg_output_name_or_not(self, binded_output_name: Option<String>) -> Self {
        let Some(binded_output_name) = binded_output_name else {
            return self;
        };
        self.with_xdg_output_name(binded_output_name)
    }

    pub fn with_allscreens_or_active(mut self, allscreen: bool) -> Self {
        if allscreen {
            self.start_mode = StartMode::AllScreens;
        } else {
            self.start_mode = StartMode::Active;
        }
        self
    }

    pub fn with_allscreens(mut self) -> Self {
        self.start_mode = StartMode::AllScreens;
        self
    }

    pub fn with_background_or_not(self, background_mode: bool) -> Self {
        if !background_mode {
            return self;
        }
        self.with_background()
    }

    pub fn with_background(mut self) -> Self {
        self.start_mode = StartMode::Background;
        self
    }

    /// keyboard_interacivity, please take look at [layer_shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
    pub fn with_keyboard_interacivity(
        mut self,
        keyboard_interacivity: zwlr_layer_surface_v1::KeyboardInteractivity,
    ) -> Self {
        self.keyboard_interactivity = keyboard_interacivity;
        self
    }

    /// set the layer_shell anchor
    pub fn with_anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// set the layer_shell layer
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    /// set the layer margin
    pub fn with_margin(mut self, (top, right, bottom, left): (i32, i32, i32, i32)) -> Self {
        self.margin = Some((top, right, bottom, left));
        self
    }

    /// if not set, it will be the size suggested by layer_shell, like anchor to four ways,
    /// and margins to 0,0,0,0 , the size will be the size of screen.
    ///
    /// if set, layer_shell will use the size you set
    pub fn with_size(mut self, size: (u32, u32)) -> Self {
        self.size = Some(size);
        self
    }

    /// set the window size, optional
    pub fn with_option_size(mut self, size: Option<(u32, u32)>) -> Self {
        self.size = size;
        self
    }

    /// exclusive_zone, please take look at [layer_shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
    pub fn with_exclusive_zone(mut self, exclusive_zone: i32) -> Self {
        self.exclusive_zone = Some(exclusive_zone);
        self
    }

    /// set layershellev to use display_handle
    pub fn with_use_display_handle(mut self, use_display_handle: bool) -> Self {
        self.use_display_handle = use_display_handle;
        self
    }

    /// set a callback to create a wayland connection
    pub fn with_connection(mut self, connection_or: Option<Connection>) -> Self {
        self.connection = connection_or;
        self
    }
}

impl<T> Default for WindowState<T> {
    fn default() -> Self {
        Self {
            outputs: Vec::new(),
            current_surface: None,
            active_surfaces: HashMap::new(),
            units: Vec::new(),
            message: Vec::new(),
            pending_axis: PendingAxisFrame::default(),
            scroll_axis_source: None,

            background_surface: None,
            display: None,

            connection: None,
            event_queue: None,
            wl_compositor: None,
            shm: None,
            wmbase: None,
            cursor_manager: None,
            viewporter: None,
            xdg_output_manager: None,
            globals: None,
            fractional_scale_manager: None,
            virtual_keyboard: None,

            seat: None,
            keyboard_state: None,
            pointer: None,
            touch: None,
            data_device_manager: None,
            data_device: None,
            dnd_offer_mimes: HashMap::new(),
            dnd_current: None,
            selection_offer: None,
            dnd_source_origin: None,
            last_button_serial: None,
            last_input_serial: None,
            dnd_icon: None,
            cached_compositor: None,
            cached_shm: None,

            namespace: "".to_owned(),
            keyboard_interactivity: zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand,
            layer: Layer::Overlay,
            anchor: Anchor::Top | Anchor::Left | Anchor::Right | Anchor::Bottom,
            size: None,
            exclusive_zone: None,
            margin: None,

            use_display_handle: false,
            repeat_delay: None,
            to_remove_tokens: Vec::new(),
            to_be_released_key: None,
            closed_ids: Vec::new(),

            last_wloutput: None,
            last_unit_index: 0,

            return_data: Vec::new(),
            finger_locations: HashMap::new(),
            enter_serial: None,
            // NOTE: if is some, means it is to be binded, but not now it
            // is not binded
            xdg_info_cache: Vec::new(),
            output_watch: Vec::new(),
            output_layout: Vec::new(),
            output_handles: Vec::new(),

            start_mode: StartMode::Active,
            init_finished: false,
            events_transparent: false,
            blur: false,
            blur_radius: None,
            blur_saturation: None,
            blur_tint: None,
            blur_border: None,
            blur_manager: None,
            blur_surfaces: HashMap::new(),
            background_effect_manager: None,
            background_effect_surfaces: HashMap::new(),
            blur_params: HashMap::new(),
            adaptive_foreground_manager: None,
            adaptive_foreground_surfaces: HashMap::new(),
            adaptive_foreground_pending: HashMap::new(),
            corner_radius: None,
            corner_radius_manager: None,
            corner_radius_surfaces: HashMap::new(),
            layer_surface_placement_manager: None,
            layer_surface_placement_surfaces: HashMap::new(),
            layer_edge_resize_manager: None,
            layer_edge_resize_surfaces: HashMap::new(),
            shadow: false,
            shadow_manager: None,
            xdg_activation: None,
            shadow_surfaces: HashMap::new(),
            keyboard_shortcuts_inhibit_manager: None,
            keyboard_shortcuts_inhibitors: HashMap::new(),
            transition: None,
            transitions: HashMap::new(),
            auto_hide_manager: None,
            auto_hide_surfaces: HashMap::new(),
            auto_hide_visible: true,
            usable_area_manager: None,
            usable_area_surfaces: HashMap::new(),
            size_transition_manager: None,
            size_transition_surfaces: HashMap::new(),
            workspace_transition_manager: None,
            tooltip_manager: None,
            tooltip_surfaces: HashMap::new(),
            capture_exclusion_manager: None,
            capture_exclusions: HashMap::new(),
            special_action: false,
            special_action_default: false,
            special_action_manager: None,
            special_action_receivers: HashMap::new(),

            layer_surface_visibility_manager: None,
            layer_surface_visibility_controllers: HashMap::new(),
            hidden_surfaces: HashSet::new(),

            layer_surface_dismiss_manager: None,
            layer_surface_dismiss_controllers: HashMap::new(),
            dismiss_requested: false,

            #[cfg(feature = "foreign-toplevel")]
            foreign_toplevel_enabled: false,
            #[cfg(feature = "foreign-toplevel")]
            ext_foreign_toplevel_list: None,
            #[cfg(feature = "cosmic-toplevel")]
            cosmic_toplevel_info: None,
            #[cfg(feature = "cosmic-toplevel")]
            toplevel_identity: toplevel_identity::State::default(),
            #[cfg(feature = "cosmic-toplevel")]
            cosmic_toplevel_manager: None,
            #[cfg(feature = "cosmic-toplevel")]
            cosmic_to_ext_handle_map: HashMap::new(),
            #[cfg(feature = "cosmic-toplevel")]
            cosmic_toplevel_handles: HashMap::new(),
            #[cfg(feature = "foreign-toplevel")]
            foreign_toplevel_manager: None,
            #[cfg(feature = "foreign-toplevel")]
            foreign_toplevel_data: HashMap::new(),
            #[cfg(feature = "foreign-toplevel")]
            foreign_toplevel_handles: HashMap::new(),
            #[cfg(feature = "foreign-toplevel")]
            ext_toplevel_handles: HashMap::new(),
            #[cfg(feature = "workspaces")]
            workspaces: ext_workspace::WorkspacesState::default(),
            #[cfg(feature = "screencopy")]
            screencopy: screencopy::ScreencopyState::new(),
            queue_handle: None,
            #[cfg(feature = "screencopy")]
            screencopy_shm: None,

            text_input_manager: None,
            text_input: None,
            text_inputs: Vec::new(),
            ime_purpose: ImePurpose::Normal,
            ime_allowed: false,
            ime_surrounding: None,
            ime_committed: false,

            xdg_decoration_manager: None,

            ping_sender: None,
            tick_sender: None,
            retry_requested: false,
            retry_timer: None,
            refresh_timer: None,
        }
    }
}

impl<T> WindowState<T> {
    /// You can save the virtual_keyboard here
    pub fn set_virtual_keyboard(&mut self, keyboard: ZwpVirtualKeyboardV1) {
        self.virtual_keyboard = Some(keyboard);
    }

    /// get the saved virtual_keyboard
    pub fn get_virtual_keyboard(&self) -> Option<&ZwpVirtualKeyboardV1> {
        self.virtual_keyboard.as_ref()
    }

    pub fn set_virtual_key_release(&mut self, key_info: VirtualKeyRelease) {
        self.to_be_released_key = Some(key_info);
    }

    /// use [id::Id] to get the mut [WindowStateUnit]
    fn get_mut_unit_with_id(&mut self, id: id::Id) -> Option<&mut WindowStateUnit<T>> {
        self.units.iter_mut().find(|unit| unit.id == id)
    }

    /// use [id::Id] to get the immutable [WindowStateUnit]
    pub fn get_unit_with_id(&self, id: id::Id) -> Option<&WindowStateUnit<T>> {
        self.units.iter().find(|unit| unit.id == id)
    }

    /// it return the iter of units. you can do loop with it
    pub fn get_unit_iter(&self) -> impl Iterator<Item = &WindowStateUnit<T>> {
        self.units.iter()
    }

    fn surface_pos(&self) -> Option<usize> {
        self.units
            .iter()
            .position(|unit| Some(&unit.wl_surface) == self.current_surface.as_ref())
    }

    /// get the current focused surface id
    pub fn current_surface_id(&self) -> Option<id::Id> {
        self.units
            .iter()
            .find(|unit| Some(&unit.wl_surface) == self.current_surface.as_ref())
            .map(|unit| unit.id())
    }

    fn get_id_from_surface(&self, surface: &WlSurface) -> Option<id::Id> {
        self.units
            .iter()
            .find(|unit| &unit.wl_surface == surface)
            .map(|unit| unit.id())
    }

    pub fn is_mouse_surface(&self, surface_id: id::Id) -> bool {
        self.active_surfaces
            .get(&None)
            .filter(|(_, id)| *id == Some(surface_id))
            .is_some()
    }

    /// update `current_surface` only if a finger is down or a mouse button is clicked or a surface
    /// is created.
    fn update_current_surface(&mut self, surface: Option<WlSurface>) {
        if surface == self.current_surface {
            return;
        }
        if let Some(surface) = surface {
            self.current_surface = Some(surface);

            // reset repeat when surface is changed
            if let Some(keyboard_state) = self.keyboard_state.as_mut() {
                keyboard_state.current_repeat = None;
            }

            let unit = self
                .units
                .iter()
                .find(|unit| Some(&unit.wl_surface) == self.current_surface.as_ref());
            if let Some(unit) = unit {
                self.message
                    .push((Some(unit.id), DispatchMessageInner::Focused(unit.id)));
                self.last_unit_index = self
                    .outputs
                    .iter()
                    .position(|(_, output)| Some(output) == unit.wl_output.as_ref())
                    .unwrap_or(0);
            }
        }
    }

    pub fn request_refresh_all(&mut self, request: RefreshRequest) {
        self.units
            .iter_mut()
            .for_each(|unit| unit.request_refresh(request));
    }

    pub fn request_refresh(&mut self, id: id::Id, request: RefreshRequest) {
        if let Some(unit) = self.get_mut_unit_with_id(id) {
            unit.request_refresh(request);
        }
    }

    /// Flush pending requests to the Wayland compositor.
    /// This ensures that all pending protocol requests are sent immediately.
    pub fn flush(&self) {
        if let Some(ref conn) = self.connection {
            let _ = conn.flush();
        }
    }

    pub fn request_close(&mut self, id: id::Id) {
        self.get_mut_unit_with_id(id)
            .map(WindowStateUnit::request_close);
    }

    pub fn get_binding_mut(&mut self, id: id::Id) -> Option<&mut T> {
        self.get_mut_unit_with_id(id)
            .and_then(WindowStateUnit::get_binding_mut)
    }
}

impl<T: 'static> Dispatch<wl_registry::WlRegistry, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_output::WlOutput::interface().name => {
                let output = proxy.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());
                state.outputs.push((name, output.clone()));
                // Unset until `build` has gathered the startup outputs, which it
                // watches itself; from then on a new output is watched here.
                if let Some(manager) = state.xdg_output_manager.as_ref() {
                    let zxdgoutput = manager.get_xdg_output(&output, qh, ());
                    state
                        .output_watch
                        .push((output.clone(), ZxdgOutputInfo::new(zxdgoutput)));
                }
                state
                    .message
                    .push((None, DispatchMessageInner::NewDisplay(output)));
            }
            wl_registry::Event::GlobalRemove { name } => {
                if state
                    .last_wloutput
                    .as_ref()
                    .is_some_and(|output| !output.is_alive())
                {
                    state.last_wloutput.take();
                }
                let removed: Vec<WlOutput> = state
                    .outputs
                    .iter()
                    .filter(|x| x.0 == name)
                    .map(|x| x.1.clone())
                    .collect();
                state.outputs.retain(|x| x.0 != name);
                if !removed.is_empty() {
                    state.output_watch.retain(|(output, info)| {
                        let gone = removed.contains(output);
                        if gone {
                            info.zxdgoutput.destroy();
                        }
                        !gone
                    });
                    state.sync_output_layout(false);
                }
                let removed_states = state
                    .units
                    .extract_if(.., |unit| !unit.wl_surface.is_alive());
                for deleled in removed_states.into_iter() {
                    state.closed_ids.push(deleled.id);
                }
            }

            _ => {}
        }
    }
}

impl<T: 'static> Dispatch<wl_seat::WlSeat, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        use xkb_keyboard::KeyboardState;
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let mut keyboard_installing = true;
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                if state.keyboard_state.is_none() {
                    state.keyboard_state = Some(KeyboardState::new(seat.get_keyboard(qh, ())));
                } else {
                    keyboard_installing = false;
                    let keyboard = state.keyboard_state.take().unwrap();
                    state.keyboard_state = Some(keyboard.update(seat, qh, ()));
                    if let Some(surface_id) = state.current_surface_id() {
                        state
                            .message
                            .push((Some(surface_id), DispatchMessageInner::Unfocus));
                    }
                }
            }
            if capabilities.contains(wl_seat::Capability::Pointer) {
                if state.pointer.is_none() {
                    state.pointer = Some(seat.get_pointer(qh, ()));
                } else {
                    let pointer = state.pointer.take().unwrap();
                    if pointer.version() >= 3 {
                        pointer.release();
                    }
                }
            }
            if capabilities.contains(wl_seat::Capability::Touch) {
                if state.touch.is_none() {
                    state.touch = Some(seat.get_touch(qh, ()));
                } else {
                    let touch = state.touch.take().unwrap();
                    if touch.version() >= 3 {
                        touch.release();
                    }
                }
            }
            if keyboard_installing {
                let text_input = state
                    .text_input_manager
                    .as_ref()
                    .map(|manager| manager.get_text_input(seat, qh, TextInputData::default()));
                state.text_input = text_input;
            } else if let Some(text_input) = state.text_input.take() {
                text_input.destroy();
            }
        }
    }
}

impl<T> Dispatch<wl_keyboard::WlKeyboard, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        _wl_keyboard: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if state.keyboard_state.is_none() {
            return;
        }

        use keyboard::*;
        use xkb_keyboard::ElementState;
        let surface_id = state.current_surface_id();

        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => match format {
                WEnum::Value(KeymapFormat::XkbV1) => {
                    let keyboard_state = state.keyboard_state.as_mut().unwrap();
                    let context = &mut keyboard_state.xkb_context;
                    context.set_keymap_from_fd(fd, size as usize)
                }
                WEnum::Value(KeymapFormat::NoKeymap) => {
                    log::warn!("non-xkb compatible keymap")
                }
                _ => unreachable!(),
            },
            wl_keyboard::Event::Enter { surface, .. } => {
                log::info!("wl_keyboard::Enter event - keyboard focus entered surface");
                // Creation may select a surface before its consumer registers it.
                // A real keyboard Enter must still report focus after that selection.
                if state.current_surface.as_ref() == Some(&surface)
                    && let Some(id) = state.get_id_from_surface(&surface)
                {
                    state
                        .message
                        .push((Some(id), DispatchMessageInner::Focused(id)));
                }
                state.update_current_surface(Some(surface));
                let keyboard_state = state.keyboard_state.as_mut().unwrap();
                if let Some(token) = keyboard_state.repeat_token.take() {
                    state.to_remove_tokens.push(token);
                }
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                // Use the actual surface from the Leave event, not current_surface_id().
                // During popup switching, current_surface may already point to a newly
                // created surface, causing the Unfocus to be misattributed.
                let leave_id = state.get_id_from_surface(&surface);
                log::info!(
                    "wl_keyboard::Leave event - emitting Unfocus for surface {:?} (current_surface={:?})",
                    leave_id,
                    surface_id
                );
                // Clear keyboard focus tracking when the surface losing focus is
                // still the current one. Without this, `current_surface` keeps
                // pointing at the surface after it loses keyboard focus, so a
                // later `Enter` on the *same* surface (e.g. a layer surface that
                // is hidden then re-shown without being destroyed) is deduped by
                // `update_current_surface` and never re-emits `Focused` — leaving
                // the client stuck "unfocused" (e.g. a text input whose caret
                // never reappears). Only clear when it matches the leaving
                // surface: during popup switching `current_surface` may already
                // have advanced to a newly created surface, which must be kept.
                if state.current_surface.as_ref() == Some(&surface) {
                    state.current_surface = None;
                }
                let keyboard_state = state.keyboard_state.as_mut().unwrap();
                keyboard_state.current_repeat = None;
                state.message.push((
                    leave_id,
                    DispatchMessageInner::ModifiersChanged(ModifiersState::empty()),
                ));
                state
                    .message
                    .push((leave_id, DispatchMessageInner::Unfocus));

                if let Some(token) = keyboard_state.repeat_token.take() {
                    state.to_remove_tokens.push(token);
                }
            }
            wl_keyboard::Event::Key {
                state: keystate,
                key,
                serial,
                ..
            } => {
                state.last_input_serial = Some(serial);
                let pressed_state = match keystate {
                    WEnum::Value(KeyState::Pressed) => ElementState::Pressed,
                    WEnum::Value(KeyState::Released) => ElementState::Released,
                    _ => {
                        return;
                    }
                };
                let keyboard_state = state.keyboard_state.as_mut().unwrap();
                let key = key + 8;
                if let Some(mut key_context) = keyboard_state.xkb_context.key_context() {
                    let event = key_context.process_key_event(key, pressed_state, false);
                    let event = DispatchMessageInner::KeyboardInput {
                        event,
                        is_synthetic: false,
                    };
                    state.message.push((surface_id, event));
                }

                match pressed_state {
                    ElementState::Pressed => {
                        let delay = match keyboard_state.repeat_info {
                            RepeatInfo::Repeat { delay, .. } => delay,
                            RepeatInfo::Disable => return,
                        };

                        if keyboard_state
                            .xkb_context
                            .keymap_mut()
                            .is_none_or(|keymap| !keymap.key_repeats(key))
                        {
                            return;
                        }

                        keyboard_state.current_repeat = Some(key);

                        if let Some(token) = keyboard_state.repeat_token.take() {
                            state.to_remove_tokens.push(token);
                        }
                        state.repeat_delay = Some(KeyboardTokenState {
                            delay,
                            key,
                            surface_id,
                            pressed_state,
                        });
                    }
                    ElementState::Released => {
                        if keyboard_state.repeat_info != RepeatInfo::Disable
                            && keyboard_state
                                .xkb_context
                                .keymap_mut()
                                .is_some_and(|keymap| keymap.key_repeats(key))
                            && Some(key) == keyboard_state.current_repeat
                        {
                            keyboard_state.current_repeat = None;

                            if let Some(token) = keyboard_state.repeat_token.take() {
                                state.to_remove_tokens.push(token);
                            }
                        }
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_locked,
                mods_latched,
                group,
                ..
            } => {
                let keyboard_state = state.keyboard_state.as_mut().unwrap();
                let xkb_context = &mut keyboard_state.xkb_context;
                let xkb_state = match xkb_context.state_mut() {
                    Some(state) => state,
                    None => return,
                };
                xkb_state.update_modifiers(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                let modifiers = xkb_state.modifiers();

                state.message.push((
                    state.current_surface_id(),
                    DispatchMessageInner::ModifiersChanged(modifiers.into()),
                ))
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                let keyboard_state = state.keyboard_state.as_mut().unwrap();
                keyboard_state.repeat_info = if rate == 0 {
                    // Stop the repeat once we get a disable event.
                    keyboard_state.current_repeat = None;

                    if let Some(token) = keyboard_state.repeat_token.take() {
                        state.to_remove_tokens.push(token);
                    }
                    RepeatInfo::Disable
                } else {
                    let gap = Duration::from_micros(1_000_000 / rate as u64);
                    let delay = Duration::from_millis(delay as u64);
                    RepeatInfo::Repeat { gap, delay }
                };
            }
            _ => {}
        }
    }
}

impl<T> Dispatch<wl_touch::WlTouch, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        _proxy: &wl_touch::WlTouch,
        event: <wl_touch::WlTouch as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down {
                serial,
                time,
                surface,
                id,
                x,
                y,
            } => {
                state.last_input_serial = Some(serial);
                state.finger_locations.insert(id, (x, y));
                let surface_id = state.get_id_from_surface(&surface);
                state
                    .active_surfaces
                    .insert(Some(id), (surface.clone(), surface_id));
                state.update_current_surface(Some(surface));
                state.message.push((
                    surface_id,
                    DispatchMessageInner::TouchDown {
                        serial,
                        time,
                        id,
                        x,
                        y,
                    },
                ))
            }
            wl_touch::Event::Cancel => {
                let mut mouse_surface = None;
                for (k, v) in state.active_surfaces.drain() {
                    if let Some(id) = k {
                        let (x, y) = state.finger_locations.remove(&id).unwrap_or_default();
                        state
                            .message
                            .push((v.1, DispatchMessageInner::TouchCancel { id, x, y }));
                    } else {
                        // keep the surface of mouse.
                        mouse_surface = Some(v);
                    }
                }
                if let Some(mouse_surface) = mouse_surface {
                    state.active_surfaces.insert(None, mouse_surface);
                }
            }
            wl_touch::Event::Up { serial, time, id } => {
                let surface_id = state
                    .active_surfaces
                    .remove(&Some(id))
                    .or_else(|| {
                        log::warn!("finger[{id}] hasn't been down.");
                        None
                    })
                    .and_then(|(_, id)| id);
                let (x, y) = state.finger_locations.remove(&id).unwrap_or_default();
                state.message.push((
                    surface_id,
                    DispatchMessageInner::TouchUp {
                        serial,
                        time,
                        id,
                        x,
                        y,
                    },
                ));
            }
            wl_touch::Event::Motion { time, id, x, y } => {
                let surface_id = state
                    .active_surfaces
                    .get(&Some(id))
                    .or_else(|| {
                        log::warn!("finger[{id}] hasn't been down.");
                        None
                    })
                    .and_then(|(_, id)| *id);
                state.finger_locations.insert(id, (x, y));
                state.message.push((
                    surface_id,
                    DispatchMessageInner::TouchMotion { time, id, x, y },
                ));
            }
            _ => {}
        }
    }
}

impl<T> Dispatch<wl_pointer::WlPointer, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        // All mouse events should be happened on the surface which is hovered by the mouse.
        let (mouse_surface, surface_id) = state
            .active_surfaces
            .get(&None)
            .map(|(surface, id)| (Some(surface), *id))
            .unwrap_or_else(|| {
                match &event {
                    wl_pointer::Event::Enter { .. } => {}
                    _ => {
                        log::warn!("mouse hasn't entered.");
                    }
                }
                (None, None)
            });
        let scale = surface_id
            .and_then(|id| state.get_unit_with_id(id))
            .map(|unit| unit.scale_float())
            .unwrap_or(1.0);
        match event {
            // Axis sub-events accumulate into the current frame and are emitted
            // together on `wl_pointer::Event::Frame` (see PendingAxisFrame).
            wl_pointer::Event::Axis { time, axis, value } => match axis {
                WEnum::Value(axis) => {
                    let pending = &mut state.pending_axis;
                    pending.active = true;
                    pending.surface_id = surface_id;
                    pending.scale = scale;
                    pending.time = time;
                    match axis {
                        wl_pointer::Axis::VerticalScroll => pending.vertical.absolute += value,
                        wl_pointer::Axis::HorizontalScroll => pending.horizontal.absolute += value,
                        _ => unreachable!(),
                    }
                }
                WEnum::Unknown(unknown) => {
                    log::warn!(target: "layershellev", "{}: invalid pointer axis: {:x}", pointer.id(), unknown);
                }
            },
            wl_pointer::Event::AxisStop { time, axis } => match axis {
                WEnum::Value(axis) => {
                    let pending = &mut state.pending_axis;
                    pending.active = true;
                    pending.surface_id = surface_id;
                    pending.scale = scale;
                    pending.time = time;
                    match axis {
                        wl_pointer::Axis::VerticalScroll => pending.vertical.stop = true,
                        wl_pointer::Axis::HorizontalScroll => pending.horizontal.stop = true,
                        _ => unreachable!(),
                    }
                }
                WEnum::Unknown(unknown) => {
                    log::warn!(target: "layershellev", "{}: invalid pointer axis: {:x}", pointer.id(), unknown);
                }
            },
            wl_pointer::Event::AxisSource { axis_source } => match axis_source {
                WEnum::Value(source) => {
                    // Persist across frames: sent once per scroll sequence, but
                    // every frame needs to know wheel vs touchpad.
                    state.scroll_axis_source = Some(source);
                }
                WEnum::Unknown(unknown) => {
                    log::warn!(target: "layershellev", "unknown pointer axis source: {unknown:x}");
                }
            },
            wl_pointer::Event::AxisDiscrete { axis, discrete } => match axis {
                WEnum::Value(axis) => {
                    let pending = &mut state.pending_axis;
                    pending.active = true;
                    pending.surface_id = surface_id;
                    pending.scale = scale;
                    match axis {
                        wl_pointer::Axis::VerticalScroll => pending.vertical.discrete += discrete,
                        wl_pointer::Axis::HorizontalScroll => {
                            pending.horizontal.discrete += discrete
                        }
                        _ => unreachable!(),
                    }
                }
                WEnum::Unknown(unknown) => {
                    log::warn!(target: "layershellev", "{}: invalid pointer axis: {:x}", pointer.id(), unknown);
                }
            },
            wl_pointer::Event::Frame => {
                let pending = std::mem::take(&mut state.pending_axis);
                if pending.active {
                    state.message.push((
                        pending.surface_id,
                        DispatchMessageInner::Axis {
                            time: pending.time,
                            scale: pending.scale,
                            horizontal: pending.horizontal,
                            vertical: pending.vertical,
                            source: state.scroll_axis_source,
                        },
                    ));
                }
            }
            wl_pointer::Event::Button {
                state: btnstate,
                serial,
                button,
                time,
            } => {
                // Remember the press serial — `wl_data_device.start_drag` needs the
                // serial of the input event that began the drag.
                state.last_button_serial = Some(serial);
                state.last_input_serial = Some(serial);
                let mouse_surface = mouse_surface.cloned();
                state.update_current_surface(mouse_surface);
                state.message.push((
                    surface_id,
                    DispatchMessageInner::MouseButton {
                        state: btnstate,
                        serial,
                        button,
                        time,
                    },
                ));
            }
            wl_pointer::Event::Leave { .. } => {
                let surface_id = state
                    .active_surfaces
                    .remove(&None)
                    .or_else(|| {
                        log::warn!("mouse hasn't entered.");
                        None
                    })
                    .and_then(|(_, id)| id);
                state
                    .message
                    .push((surface_id, DispatchMessageInner::MouseLeave));
            }
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                let surface_id = state.get_id_from_surface(&surface);
                state
                    .active_surfaces
                    .insert(None, (surface.clone(), surface_id));
                state.enter_serial = Some(serial);
                state.message.push((
                    surface_id,
                    DispatchMessageInner::MouseEnter {
                        pointer: pointer.clone(),
                        serial,
                        surface_x,
                        surface_y,
                    },
                ));
            }
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                state.message.push((
                    surface_id,
                    DispatchMessageInner::MouseMotion {
                        time,
                        surface_x,
                        surface_y,
                    },
                ));
            }
            _ => {
                // TODO: not now
            }
        }
    }
}

/// Parse a `text/uri-list` payload (raw bytes) into local filesystem paths: one
/// `file:` URI per line, ignoring blank lines and `#` comments, percent-decoding
/// the path. Non-`file:` URIs (e.g. `http://`) are skipped — we can only attach
/// local files. The bytes are first decoded to text, transcoding UTF-16 sources
/// (some X11/XWayland drags deliver wide-encoded URI lists).
fn parse_uri_list(buf: &[u8]) -> Vec<std::path::PathBuf> {
    let text = decode_uri_text(buf);
    text.lines()
        // Trim whitespace *and* stray NULs (wide-encoded leftovers / padding).
        .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\0'))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(uri_to_path)
        .collect()
}

/// Convert one `file:` URI to a path, accepting the `file:/path`,
/// `file://host/path`, and `file:///path` spellings. Returns `None` for other
/// schemes or malformed entries.
fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    // Drop a UTF-8 BOM if it rode in on the first line.
    let uri = uri.trim_start_matches('\u{feff}');
    let rest = uri.strip_prefix("file:")?;
    let path = if let Some(after_authority) = rest.strip_prefix("//") {
        // `//host/abs-path` (host usually empty → `///abs-path`).
        &after_authority[after_authority.find('/')?..]
    } else if rest.starts_with('/') {
        // `file:/abs-path` (no authority).
        rest
    } else {
        return None;
    };
    Some(std::path::PathBuf::from(percent_decode(path)))
}

/// Decode a DnD/clipboard text payload to a UTF-8 string. Most sources send
/// UTF-8, but some X11/XWayland sources deliver UTF-16 (with a BOM, or with NUL
/// bytes interleaved between ASCII) — detect and transcode those so the URI
/// lines are actually readable.
fn decode_uri_text(buf: &[u8]) -> String {
    // Explicit byte-order mark.
    if let Some((&a, &b)) = buf.first().zip(buf.get(1)) {
        match (a, b) {
            (0xFF, 0xFE) => return decode_utf16(&buf[2..], false),
            (0xFE, 0xFF) => return decode_utf16(&buf[2..], true),
            _ => {}
        }
    }
    // No BOM: if roughly half the bytes are NUL, it's UTF-16. Endianness is told
    // by which column (even/odd byte positions) the zero high-bytes sit in.
    if buf.len() >= 2 {
        let nuls = buf.iter().filter(|&&b| b == 0).count();
        if nuls * 2 >= buf.len().saturating_sub(1) {
            let zeros_even = buf.iter().step_by(2).filter(|&&b| b == 0).count();
            let zeros_odd = buf.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
            // Big-endian: the zero high byte comes first (even positions).
            return decode_utf16(buf, zeros_even > zeros_odd);
        }
    }
    String::from_utf8_lossy(buf).into_owned()
}

/// Transcode raw UTF-16 bytes (given endianness) to a UTF-8 string.
fn decode_utf16(buf: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = buf
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| {
            if big_endian {
                u16::from_be_bytes(c)
            } else {
                u16::from_le_bytes(c)
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Percent-decode a URI path component (`%20` → space, etc.). Invalid escapes
/// are left as-is.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// The data device manager has no events; binding it just lets us create the
// data device.
delegate_noop!(@<T> WindowState<T>: ignore WlDataDeviceManager);

/// The data device delivers drag-and-drop offers from other apps. We support the
/// *receive* side only (dropping files in), routing each dropped file path to
/// the surface it landed on as a [`DispatchMessageInner::FileDropped`].
impl<T: 'static> Dispatch<WlDataDevice, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        _device: &WlDataDevice,
        event: <WlDataDevice as Proxy>::Event,
        _data: &(),
        conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        use std::os::fd::AsFd;
        match event {
            // A new offer is introduced; its MIME types arrive next (on the
            // offer object) before the matching enter/selection event.
            wl_data_device::Event::DataOffer { id } => {
                log::info!(target: "kcopy_dnd", "DataOffer (new offer)");
                state.dnd_offer_mimes.insert(id.id(), Vec::new());
            }
            // A drag entered one of our surfaces. Accept the file-list MIME (and
            // negotiate a copy action on v3+) so the compositor shows a "droppable"
            // cursor and will later emit the drop.
            wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id,
            } => {
                // A drag already hovering us when a new one enters. A compositor
                // is meant to leave a surface before it enters another, so this
                // is defence against one that does not — but taking `dnd_current`
                // is exactly what ends a drag as far as a destination can tell,
                // so it says so rather than dropping the previous drag on the
                // floor the way the missing leave on a non-file drop used to.
                if let Some(prev) = state.dnd_current.take() {
                    state.dnd_offer_mimes.remove(&prev.offer.id());
                    prev.offer.destroy();
                    state
                        .message
                        .push((prev.surface_id, DispatchMessageInner::DndLeft));
                }
                if let Some(offer) = id {
                    let surface_id = state.get_id_from_surface(&surface);
                    let mimes = state
                        .dnd_offer_mimes
                        .get(&offer.id())
                        .cloned()
                        .unwrap_or_default();
                    let has_uri_list = mimes.iter().any(|m| m == URI_LIST_MIME);
                    log::info!(
                        target: "kcopy_dnd",
                        "Enter surface={surface_id:?} pos=({x},{y}) mimes={mimes:?}"
                    );
                    // Accept a MIME + negotiate copy/move so the compositor shows a
                    // "droppable" cursor. Prefer uri-list, else the first offered.
                    let accept_mime = if has_uri_list {
                        Some(URI_LIST_MIME.to_string())
                    } else {
                        mimes.first().cloned()
                    };
                    offer.accept(serial, accept_mime);
                    if offer.version() >= 3 {
                        offer.set_actions(DndAction::Copy, DndAction::Copy);
                    }
                    state.message.push((
                        surface_id,
                        DispatchMessageInner::DndEntered {
                            x,
                            y,
                            mime_types: mimes,
                        },
                    ));
                    state.dnd_current = Some(DndCurrent {
                        offer,
                        surface_id,
                        has_uri_list,
                        // Nothing negotiated yet. The compositor answers the
                        // `set_actions` above with an `action` event of its own,
                        // which the `WlDataOffer` dispatch records here.
                        action: DndAction::empty(),
                    });
                }
            }
            // The drag moved over the surface — forward the surface-local position
            // so a drop target can track it (the Drop event itself carries none).
            wl_data_device::Event::Motion { x, y, .. } => {
                if let Some(dnd) = &state.dnd_current {
                    state
                        .message
                        .push((dnd.surface_id, DispatchMessageInner::DndMotion { x, y }));
                }
            }
            // The drag left without dropping: destroy the offer and clear the
            // highlight.
            wl_data_device::Event::Leave => {
                log::info!(target: "kcopy_dnd", "Leave");
                if let Some(dnd) = state.dnd_current.take() {
                    state.dnd_offer_mimes.remove(&dnd.offer.id());
                    dnd.offer.destroy();
                    state
                        .message
                        .push((dnd.surface_id, DispatchMessageInner::DndLeft));
                }
            }
            // The drop happened: pull the URI list off the offer and emit one
            // FileDropped per file.
            wl_data_device::Event::Drop => {
                log::info!(target: "kcopy_dnd", "Drop (current={})", state.dnd_current.is_some());
                if let Some(dnd) = state.dnd_current.take() {
                    let surface_id = dnd.surface_id;
                    // Notify the drop happened (position-flow consumers track the
                    // last DndMotion); the uri-list auto-read below stays for
                    // back-compat FileDropped consumers.
                    state
                        .message
                        .push((surface_id, DispatchMessageInner::DndDrop));
                    // Self-drop: don't block-read our own uri-list (our source's
                    // Send runs on this thread). finish() completes it; the drop
                    // position comes from DndDrop + the last DndMotion.
                    let is_self_drop = state.dnd_source_origin.is_some();
                    if dnd.has_uri_list && is_self_drop {
                        dnd.finish();
                    } else if dnd.has_uri_list {
                        // External drag: read the uri-list off a socketpair and emit
                        // FileDropped. A cooperating source writes immediately; the
                        // short timeout only guards a misbehaving one.
                        if let Ok((mut reader, writer)) = std::os::unix::net::UnixStream::pair() {
                            dnd.offer.receive(URI_LIST_MIME.to_string(), writer.as_fd());
                            let _ = conn.flush();
                            drop(writer);
                            let _ = reader.set_read_timeout(Some(URI_LIST_READ_TIMEOUT));
                            use std::io::Read;
                            let mut buf = Vec::new();
                            let _ = reader.read_to_end(&mut buf);
                            dnd.finish();
                            for path in parse_uri_list(&buf) {
                                state
                                    .message
                                    .push((surface_id, DispatchMessageInner::FileDropped(path)));
                            }
                        }
                    }
                    // A drop ends the gesture, whatever the offer turned out to
                    // hold — so this is emitted for EVERY drop, not just the ones
                    // that produced files.
                    //
                    // It used to sit inside both branches above, which left a drop
                    // carrying anything but a `text/uri-list` — dragged text, a
                    // colour, an image off a web page — emitting `DndDrop` and
                    // then nothing, ever. No leave means no end: any state a
                    // destination holds for the length of a drag is stranded with
                    // nothing left to release it, which in humainos-home pinned
                    // the chat input over the user's window for the session.
                    //
                    // After the uri-list read, so the order a destination sees is
                    // still `DndDrop` → `FileDropped`×N → `DndLeft`, and the paths
                    // arrive before the event that says the gesture is over.
                    state
                        .message
                        .push((surface_id, DispatchMessageInner::DndLeft));
                    state.dnd_offer_mimes.remove(&dnd.offer.id());
                    // Destroying a dropped-but-unfinished offer is how the source
                    // learns the drop was declined: smithay answers it with
                    // `wl_data_source.cancelled` (its `wl_data_offer` `Destroy`
                    // handler, `dropped && !finished` → `source.cancel()`), so a
                    // source is not left waiting on a `dnd_finished` that is not
                    // coming.
                    //
                    // Which is also why nothing is finished on the no-uri-list
                    // path. `finish` means the transfer SUCCEEDED, and under a
                    // `move` action that is the source's cue to delete what it
                    // handed over — a lie we must not tell about data we never
                    // read.
                    //
                    // The honest caveat: that `cancelled` is smithay being
                    // forgiving rather than the spec promising anything. We
                    // accepted a mime on enter, so a strict compositor need not
                    // treat this as a declined drop at all. The real fix is to
                    // stop accepting mimes we will never read — see the `Enter`
                    // arm's `mimes.first()` fallback — not to finish here.
                    dnd.offer.destroy();
                }
            }
            // A clipboard selection offer. Text still goes through iced's own
            // clipboard, but a file list does not: it lives on the selection as
            // `text/uri-list`, which `window_clipboard` cannot read. So hold the
            // offer (releasing the one it replaces) and let a paste pull the URI
            // list off it — see `read_selection_uri_list`. `None` means the
            // selection was cleared.
            wl_data_device::Event::Selection { id } => {
                if let Some((previous, _)) = state.selection_offer.take() {
                    state.dnd_offer_mimes.remove(&previous.id());
                    previous.destroy();
                }
                if let Some(offer) = id {
                    let mimes = state
                        .dnd_offer_mimes
                        .remove(&offer.id())
                        .unwrap_or_default();
                    log::info!(target: "kcopy_dnd", "Selection offer, mimes: {mimes:?}");
                    state.selection_offer = Some((offer, mimes));
                } else {
                    log::info!(target: "kcopy_dnd", "Selection cleared");
                }
            }
            _ => {}
        }
    }

    // Opcode 0 is `wl_data_device.data_offer` — the event that introduces a new
    // `wl_data_offer`, whose user data we set here.
    event_created_child!(WindowState<T>, WlDataDevice, [
        0 => (WlDataOffer, ()),
    ]);
}

/// A data offer advertises its available MIME types (one `offer` event each)
/// before the enter/selection that uses it; we accumulate them keyed by offer.
/// It also reports the action the compositor settled on, which is what decides
/// whether the drop may be finished — see [`DndCurrent::finish`].
impl<T> Dispatch<WlDataOffer, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        offer: &WlDataOffer,
        event: <WlDataOffer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_data_offer::Event::Offer { mime_type } => {
                log::info!(target: "kcopy_dnd", "Offer mime: {mime_type}");
                if let Some(mimes) = state.dnd_offer_mimes.get_mut(&offer.id()) {
                    mimes.push(mime_type);
                }
            }
            // The compositor's answer to our `set_actions`, re-sent whenever the
            // negotiated action changes. Recorded rather than acted on: the only
            // thing it decides here is whether `finish` is legal at drop time.
            wl_data_offer::Event::Action { dnd_action } => {
                let action = match dnd_action {
                    WEnum::Value(action) => action,
                    _ => DndAction::empty(),
                };
                log::info!(target: "kcopy_dnd", "Offer action: {action:?}");
                if let Some(dnd) = state.dnd_current.as_mut()
                    && dnd.offer.id() == offer.id()
                {
                    dnd.action = action;
                }
            }
            _ => {}
        }
    }
}

/// The *source* side of an outgoing drag we started: when the destination app
/// requests the data, write the matching pre-serialized blob to the fd; route
/// drop-performed / finished / cancelled back to the originating surface so the
/// app can complete the operation.
impl<T: 'static> Dispatch<WlDataSource, DndSourceData> for WindowState<T> {
    fn event(
        state: &mut Self,
        source: &WlDataSource,
        event: <WlDataSource as Proxy>::Event,
        data: &DndSourceData,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        use std::os::fd::{AsRawFd, FromRawFd};
        let origin = state.dnd_source_origin;
        match event {
            // The destination asked for the data for `mime_type` — write our
            // pre-serialized blob to the fd. We must NOT close the fd here
            // (wayland-client owns it and closes it after this callback).
            wl_data_source::Event::Send { mime_type, fd } => {
                if let Some(idx) = data.mime_types.iter().position(|m| m == &mime_type)
                    && let Some(blob) = data.data.get(idx)
                {
                    let mut file = unsafe { std::fs::File::from_raw_fd(fd.as_raw_fd()) };
                    let _ = std::io::Write::write_all(&mut file, blob);
                    std::mem::forget(file);
                }
            }
            wl_data_source::Event::Cancelled => {
                state
                    .message
                    .push((origin, DispatchMessageInner::DndSourceCancelled));
                source.destroy();
                state.dnd_source_origin = None;
                if let Some(icon) = state.dnd_icon.take() {
                    icon.destroy();
                }
            }
            wl_data_source::Event::DndDropPerformed => {
                state
                    .message
                    .push((origin, DispatchMessageInner::DndSourceDropPerformed));
            }
            wl_data_source::Event::DndFinished => {
                state
                    .message
                    .push((origin, DispatchMessageInner::DndSourceFinished));
                source.destroy();
                state.dnd_source_origin = None;
                if let Some(icon) = state.dnd_icon.take() {
                    icon.destroy();
                }
            }
            wl_data_source::Event::Action { dnd_action } => {
                let bits = match dnd_action {
                    WEnum::Value(a) => a.bits(),
                    _ => 0,
                };
                state
                    .message
                    .push((origin, DispatchMessageInner::DndSourceAction(bits)));
            }
            _ => {}
        }
    }
}

impl<T> Dispatch<xdg_surface::XdgSurface, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: <xdg_surface::XdgSurface as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state
                .units
                .iter_mut()
                .filter(|unit| unit.shell == *surface)
                .for_each(|unit| {
                    unit.configured = true;
                    unit.request_refresh(RefreshRequest::NextFrame);
                });
        }
    }
}

impl<T> Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: <zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let unit_index = state.units.iter().position(|unit| unit.shell == *surface);
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                surface.ack_configure(serial);

                let Some(unit_index) = unit_index else {
                    return;
                };
                state.units[unit_index].size = (width, height);
                state.units[unit_index].configured = true;

                state.units[unit_index].request_refresh(RefreshRequest::NextFrame);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                if let Some(i) = unit_index {
                    state.units[i].request_close();
                }
            }
            _ => log::info!("ignore zwlr_layer_surface_v1 event: {event:?}"),
        }
    }
}

impl<T> Dispatch<xdg_toplevel::XdgToplevel, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        surface: &xdg_toplevel::XdgToplevel,
        event: <xdg_toplevel::XdgToplevel as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let unit_index = state.units.iter().position(|unit| unit.shell == *surface);
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                let Some(unit_index) = unit_index else {
                    return;
                };
                if width != 0 && height != 0 {
                    state.units[unit_index].size = (width as u32, height as u32);
                }

                state.units[unit_index].request_refresh(RefreshRequest::NextFrame);
            }
            xdg_toplevel::Event::Close => {
                let Some(unit_index) = unit_index else {
                    return;
                };
                state.units[unit_index].request_flag.close = true;
            }
            _ => {}
        }
    }
}

impl<T> Dispatch<xdg_popup::XdgPopup, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        surface: &xdg_popup::XdgPopup,
        event: <xdg_popup::XdgPopup as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            xdg_popup::Event::Configure {
                width,
                height,
                x,
                y,
            } => {
                let Some(unit_index) = state.units.iter().position(|unit| unit.shell == *surface)
                else {
                    return;
                };
                log::debug!(
                    "xdg_popup configure: width={width}, height={height}, x={x}, y={y}, unit_index={unit_index}"
                );
                state.units[unit_index].size = (width as u32, height as u32);

                // Set the viewport destination so the compositor knows the
                // logical size of the popup surface.  Without this, a buffer
                // rendered at physical resolution would be treated as that
                // many logical pixels, causing the surface to appear too large
                // and its position to be wrong on HiDPI outputs.
                state.units[unit_index].try_set_viewport_destination(width, height);

                state.units[unit_index].request_refresh(RefreshRequest::NextFrame)
            }
            xdg_popup::Event::Repositioned { token } => {
                log::debug!("xdg_popup repositioned: token={token}");
                // The compositor has repositioned the popup. The new position
                // will take effect with the next configure event.
            }
            _ => {}
        }
    }
}

impl<T> WindowState<T> {
    /// `output`'s `xdg_output` name, once the compositor has sent it.
    #[cfg(feature = "workspaces")]
    pub(crate) fn output_name(&self, output: &wl_output::WlOutput) -> Option<String> {
        self.output_watch
            .iter()
            .find(|(watched, info)| watched == output && !info.name.is_empty())
            .map(|(_, info)| info.name.clone())
    }

    /// Re-derive `output_layout` and `output_handles` from `output_watch`, and
    /// queue `OutputLayoutChanged` when the layout moved (or when `force`d).
    ///
    /// An output counts once its name and logical size are known. A new
    /// output's xdg_output reports those one event at a time, and until it has
    /// a name nothing could target it by `OutputOption::OutputName` anyway.
    fn sync_output_layout(&mut self, force: bool) {
        let known: Vec<_> = self
            .output_watch
            .iter()
            .filter(|(_, info)| {
                !info.name.is_empty() && info.logical_size.0 > 0 && info.logical_size.1 > 0
            })
            .collect();
        let layout: Vec<OutputLayoutItem> = known
            .iter()
            .map(|(_, info)| OutputLayoutItem {
                name: info.name.clone(),
                x: info.position.0,
                y: info.position.1,
                width: info.logical_size.0,
                height: info.logical_size.1,
            })
            .collect();
        self.output_handles = known
            .iter()
            .map(|(output, info)| (info.name.clone(), output.clone()))
            .collect();
        if !force && layout == self.output_layout {
            return;
        }
        self.output_layout = layout;
        // A workspace group names its outputs, which may have just been named.
        #[cfg(feature = "workspaces")]
        if self.workspaces.manager.is_some() {
            self.emit_workspaces();
        }
        // One snapshot per drain: a newer layout replaces one still queued.
        self.message
            .retain(|(_, m)| !matches!(m, DispatchMessageInner::OutputLayoutChanged(_)));
        self.message.push((
            None,
            DispatchMessageInner::OutputLayoutChanged(self.output_layout.clone()),
        ));
    }
}

impl<T> Dispatch<zxdg_output_v1::ZxdgOutputV1, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &zxdg_output_v1::ZxdgOutputV1,
        event: <zxdg_output_v1::ZxdgOutputV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // Update a cached output entry if this proxy belongs to one.
        // `StartMode::TargetScreen` populates `xdg_info_cache`; do this
        // regardless of start mode / init state so its dispatch actually records
        // each output's size/position/name. (Afterwards the cache is cleared, so
        // this falls through to the paths below.)
        if let Some((_, xdg_info)) = state
            .xdg_info_cache
            .iter_mut()
            .find(|(_, info)| info.zxdgoutput == *proxy)
        {
            match event {
                zxdg_output_v1::Event::LogicalSize { width, height } => {
                    xdg_info.logical_size = (width, height);
                }
                zxdg_output_v1::Event::LogicalPosition { x, y } => {
                    xdg_info.position = (x, y);
                }
                zxdg_output_v1::Event::Name { name } => {
                    xdg_info.name = name;
                }
                zxdg_output_v1::Event::Description { description } => {
                    xdg_info.description = description;
                }
                _ => {}
            };
            return;
        }
        // An output's own watch (see `output_watch`): fold the field in and
        // re-derive the layout, which queues `OutputLayoutChanged` if it moved.
        if let Some((_, xdg_info)) = state
            .output_watch
            .iter_mut()
            .find(|(_, info)| info.zxdgoutput == *proxy)
        {
            match event {
                zxdg_output_v1::Event::LogicalSize { width, height } => {
                    xdg_info.logical_size = (width, height);
                }
                zxdg_output_v1::Event::LogicalPosition { x, y } => {
                    xdg_info.position = (x, y);
                }
                zxdg_output_v1::Event::Name { name } => {
                    xdg_info.name = name;
                }
                zxdg_output_v1::Event::Description { description } => {
                    xdg_info.description = description;
                }
                _ => return,
            };
            state.sync_output_layout(false);
            return;
        }
        let Some(index) = state.units.iter().position(|info| {
            info.zxdgoutput
                .as_ref()
                .is_some_and(|zxdgoutput| zxdgoutput.zxdgoutput == *proxy)
        }) else {
            return;
        };
        let info = &mut state.units[index];
        let xdg_info = info.zxdgoutput.as_mut().unwrap();
        let change_type = match event {
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                xdg_info.logical_size = (width, height);
                XdgInfoChangedType::Size
            }
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                xdg_info.position = (x, y);
                XdgInfoChangedType::Position
            }
            zxdg_output_v1::Event::Name { name } => {
                xdg_info.name = name;
                XdgInfoChangedType::Name
            }
            zxdg_output_v1::Event::Description { description } => {
                xdg_info.description = description;
                XdgInfoChangedType::Description
            }
            _ => {
                return;
            }
        };
        let (logical_width, logical_height) = xdg_info.logical_size;
        let output_name = xdg_info.name.clone();
        let (output_x, output_y) = xdg_info.position;
        state.message.push((
            Some(state.units[index].id),
            DispatchMessageInner::XdgInfoChanged {
                change_type,
                logical_width,
                logical_height,
                output_name,
                output_x,
                output_y,
            },
        ));
    }
}

impl<T> Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: <wp_fractional_scale_v1::WpFractionalScaleV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            log::trace!(
                "fractional_scale PreferredScale RECEIVED: scale_raw={}, scale_float={}",
                scale,
                scale as f64 / 120.
            );
            let Some(unit) = state.units.iter_mut().find(|info| {
                info.fractional_scale
                    .as_ref()
                    .is_some_and(|fractional_scale| fractional_scale == proxy)
            }) else {
                log::warn!("PreferredScale: could not find unit for fractional_scale proxy");
                return;
            };
            log::debug!(
                "fractional_scale PreferredScale: unit_id={:?}, is_popup={}, scale_raw={}, scale_float={}, size={:?}",
                unit.id,
                unit.is_popup(),
                scale,
                scale as f64 / 120.,
                unit.get_size()
            );
            unit.scale = scale;
            unit.request_refresh(RefreshRequest::NextFrame);
            state.message.push((
                Some(unit.id),
                DispatchMessageInner::PreferredScale {
                    scale_u32: scale,
                    scale_float: scale as f64 / 120.,
                },
            ));
        }
    }
}

#[derive(Default)]
pub struct TextInputData {
    inner: std::sync::Mutex<TextInputDataInner>,
}

#[derive(Default)]
pub struct TextInputDataInner {
    /// The `WlSurface` we're performing input to.
    surface: Option<WlSurface>,

    /// The commit to submit on `done`.
    pending_commit: Option<String>,

    /// The preedit to submit on `done`.
    pending_preedit: Option<Preedit>,
}
/// The state of the preedit.
struct Preedit {
    text: String,
    cursor_begin: Option<usize>,
    cursor_end: Option<usize>,
}

impl<T> Dispatch<zwp_text_input_v3::ZwpTextInputV3, TextInputData> for WindowState<T> {
    fn event(
        state: &mut Self,
        text_input: &zwp_text_input_v3::ZwpTextInputV3,
        event: <zwp_text_input_v3::ZwpTextInputV3 as Proxy>::Event,
        data: &TextInputData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use zwp_text_input_v3::Event;
        let mut text_input_data = data.inner.lock().unwrap();

        match event {
            Event::Enter { surface } => {
                let Some(id) = state.get_id_from_surface(&surface) else {
                    return;
                };
                text_input_data.surface = Some(surface);

                if state.ime_allowed() {
                    state.enable_text_input(text_input);
                    text_input.commit();
                    state
                        .message
                        .push((Some(id), DispatchMessageInner::Ime(events::Ime::Enabled)));
                }
                state.text_input_entered(text_input);
            }
            Event::Leave { surface } => {
                text_input_data.surface = None;

                text_input.disable();
                text_input.commit();
                let Some(id) = state.get_id_from_surface(&surface) else {
                    return;
                };
                state.text_input_left(text_input);
                state
                    .message
                    .push((Some(id), DispatchMessageInner::Ime(events::Ime::Disabled)));
            }
            Event::CommitString { text } => {
                text_input_data.pending_preedit = None;
                text_input_data.pending_commit = text;
            }
            Event::DeleteSurroundingText { .. } => {}
            Event::Done { .. } => {
                let Some(id) = text_input_data
                    .surface
                    .as_ref()
                    .and_then(|surface| state.get_id_from_surface(surface))
                else {
                    return;
                };
                // Clear preedit, unless all we'll be doing next is sending a new preedit.
                if text_input_data.pending_commit.is_some()
                    || text_input_data.pending_preedit.is_none()
                {
                    state.message.push((
                        Some(id),
                        DispatchMessageInner::Ime(Ime::Preedit(String::new(), None)),
                    ));
                }

                // Send `Commit`.
                if let Some(text) = text_input_data.pending_commit.take() {
                    state.ime_committed = true;
                    state
                        .message
                        .push((Some(id), DispatchMessageInner::Ime(Ime::Commit(text))));
                }

                // Send preedit.
                if let Some(preedit) = text_input_data.pending_preedit.take() {
                    let cursor_range = preedit
                        .cursor_begin
                        .map(|b| (b, preedit.cursor_end.unwrap_or(b)));

                    state.message.push((
                        Some(id),
                        DispatchMessageInner::Ime(Ime::Preedit(preedit.text, cursor_range)),
                    ));
                }
            }
            Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let text = text.unwrap_or_default();
                let cursor_begin = usize::try_from(cursor_begin)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));
                let cursor_end = usize::try_from(cursor_end)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));

                text_input_data.pending_preedit = Some(Preedit {
                    text,
                    cursor_begin,
                    cursor_end,
                })
            }

            _ => {}
        }
    }
}

/// The `wl_display.sync` sent after a size request of the unit it names.
#[derive(Debug, Clone, Copy)]
struct SizeAnswer(id::Id);

impl<T> Dispatch<WlCallback, SizeAnswer> for WindowState<T> {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        data: &SizeAnswer,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let WlCallbackEvent::Done { .. } = event else {
            return;
        };
        let Some(unit) = state.get_mut_unit_with_id(data.0) else {
            return;
        };
        let left = unit.unanswered_sizes.get_mut();
        *left = left.saturating_sub(1);
        if *left == 0 {
            unit.request_refresh(RefreshRequest::NextFrame);
        }
    }
}

impl<T> Dispatch<WlCallback, (id::Id, PresentAvailableState)> for WindowState<T> {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        data: &(id::Id, PresentAvailableState),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let WlCallbackEvent::Done { callback_data: _ } = event
            && let Some(unit) = state.get_mut_unit_with_id(data.0)
        {
            unit.present_available_state = data.1;
            // Wake the event loop immediately so the timer callback
            // picks up the newly-available present slot without waiting
            // for the next timer tick.  This is critical for smooth
            // animations: the compositor just told us the previous
            // frame landed, so we should render the next one ASAP.
            if let Some(sender) = &state.ping_sender {
                sender.ping();
            }
        }
    }
}

delegate_noop!(@<T> WindowState<T>: ignore WlCompositor); // WlCompositor is need to create a surface

// `wl_surface.enter` tells us which output a surface is shown on. For surfaces
// created with no explicit output binding (`StartMode::Active`), this is the only
// signal that reveals the compositor's placement — so we bind that output's
// xdg_output to the unit here, which makes the existing xdg_output dispatch report
// the output's logical size (used to position centered layer surfaces per-display).
// All other wl_surface events are ignored (equivalent to the previous no-op).
impl<T: 'static> Dispatch<WlSurface, ()> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &WlSurface,
        event: <WlSurface as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        let wl_surface::Event::Enter { output } = event else {
            return;
        };
        let Some(xdg_output_manager) = state.xdg_output_manager.clone() else {
            return;
        };
        let Some(index) = state
            .units
            .iter()
            .position(|unit| unit.wl_surface == *proxy)
        else {
            return;
        };
        // Already tracking this output for this surface — nothing to do.
        if state.units[index].wl_output.as_ref() == Some(&output) {
            return;
        }
        // (Re)bind the entered output's xdg_output: the compositor will replay its
        // LogicalSize/Position/Name events for the new object, which the
        // zxdg_output_v1 dispatch folds into the unit + emits as XdgInfoChanged.
        let zxdgoutput = xdg_output_manager.get_xdg_output(&output, qhandle, ());
        state.units[index].zxdgoutput = Some(ZxdgOutputInfo::new(zxdgoutput));
        state.units[index].wl_output = Some(output);
    }
}

delegate_noop!(@<T> WindowState<T>: ignore WlOutput); // output is need to place layer_shell, although here
// it is not used
delegate_noop!(@<T> WindowState<T>: ignore WlShm); // shm is used to create buffer pool
delegate_noop!(@<T> WindowState<T>: ignore WlShmPool); // so it is pool, created by wl_shm
delegate_noop!(@<T> WindowState<T>: ignore WlBuffer); // buffer show the picture
delegate_noop!(@<T> WindowState<T>: ignore WlRegion); // region is used to modify input region
delegate_noop!(@<T> WindowState<T>: ignore ZwlrLayerShellV1); // it is similar with xdg_toplevel, also the
// ext-session-shell

delegate_noop!(@<T> WindowState<T>: ignore WpCursorShapeManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore WpCursorShapeDeviceV1);

delegate_noop!(@<T> WindowState<T>: ignore WpViewporter);
delegate_noop!(@<T> WindowState<T>: ignore WpViewport);

delegate_noop!(@<T> WindowState<T>: ignore ZwpVirtualKeyboardV1);
delegate_noop!(@<T> WindowState<T>: ignore ZwpVirtualKeyboardManagerV1);

delegate_noop!(@<T> WindowState<T>: ignore ZxdgOutputManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore WpFractionalScaleManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore XdgPositioner);
delegate_noop!(@<T> WindowState<T>: ignore XdgWmBase);

delegate_noop!(@<T> WindowState<T>: ignore ZwpTextInputManagerV3);
delegate_noop!(@<T> WindowState<T>: ignore ZwpInputPanelSurfaceV1);
delegate_noop!(@<T> WindowState<T>: ignore ZwpInputPanelV1);

delegate_noop!(@<T> WindowState<T>: ignore ZxdgDecorationManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore ZxdgToplevelDecorationV1);

// Blur protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager);
// The activation manager itself has no events.
delegate_noop!(@<T> WindowState<T>: ignore XdgActivationV1);

/// The token's user data is the surface it was minted for, so `Done` can activate
/// it without any side table.
impl<T: 'static> Dispatch<XdgActivationTokenV1, WlSurface> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &XdgActivationTokenV1,
        event: <XdgActivationTokenV1 as Proxy>::Event,
        surface: &WlSurface,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            if let Some(activation) = state.xdg_activation.as_ref() {
                activation.activate(token, surface);
            }
            // The token is spent either way.
            proxy.destroy();
        }
    }
}

// Manual Dispatch impl for blur object since it has custom user data
impl<T: 'static> Dispatch<blur::org_kde_kwin_blur::OrgKdeKwinBlur, blur::BlurData>
    for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &blur::org_kde_kwin_blur::OrgKdeKwinBlur,
        _event: <blur::org_kde_kwin_blur::OrgKdeKwinBlur as Proxy>::Event,
        _data: &blur::BlurData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for blur objects
    }
}

// Background effect protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore background_effect::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1);

impl<T: 'static>
    Dispatch<
        background_effect::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
        background_effect::BackgroundEffectData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &background_effect::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
        _event: <background_effect::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1 as Proxy>::Event,
        _data: &background_effect::BackgroundEffectData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for background effect objects
    }
}

// Corner radius protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore corner_radius::layer_corner_radius_manager_v1::LayerCornerRadiusManagerV1);

// Manual Dispatch impl for corner radius surface object since it has custom user data
impl<T: 'static>
    Dispatch<
        corner_radius::layer_corner_radius_surface_v1::LayerCornerRadiusSurfaceV1,
        corner_radius::CornerRadiusData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &corner_radius::layer_corner_radius_surface_v1::LayerCornerRadiusSurfaceV1,
        _event: <corner_radius::layer_corner_radius_surface_v1::LayerCornerRadiusSurfaceV1 as Proxy>::Event,
        _data: &corner_radius::CornerRadiusData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for corner radius objects
    }
}

// Layer surface placement protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore layer_surface_placement::layer_surface_placement_manager_v1::LayerSurfacePlacementManagerV1);

// Manual Dispatch impl for the placement object since it has custom user data
impl<T: 'static>
    Dispatch<
        layer_surface_placement::layer_surface_placement_v1::LayerSurfacePlacementV1,
        layer_surface_placement::LayerSurfacePlacementData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &layer_surface_placement::layer_surface_placement_v1::LayerSurfacePlacementV1,
        _event: <layer_surface_placement::layer_surface_placement_v1::LayerSurfacePlacementV1 as Proxy>::Event,
        _data: &layer_surface_placement::LayerSurfacePlacementData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for placement objects
    }
}

// Layer edge-resize protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore layer_edge_resize::layer_edge_resize_manager_v1::LayerEdgeResizeManagerV1);

// Manual Dispatch impl for the edge-resize object since it has custom user data
impl<T: 'static>
    Dispatch<
        layer_edge_resize::layer_edge_resize_v1::LayerEdgeResizeV1,
        layer_edge_resize::LayerEdgeResizeData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &layer_edge_resize::layer_edge_resize_v1::LayerEdgeResizeV1,
        _event: <layer_edge_resize::layer_edge_resize_v1::LayerEdgeResizeV1 as Proxy>::Event,
        _data: &layer_edge_resize::LayerEdgeResizeData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for edge-resize objects
    }
}

// Keyboard-shortcuts-inhibit protocol delegates. The manager has no events; the
// inhibitor emits active/inactive, which are informational here (cosmic-comp
// activates an inhibitor on creation), so both are ignored.
delegate_noop!(@<T> WindowState<T>: ignore ZwpKeyboardShortcutsInhibitManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore ZwpKeyboardShortcutsInhibitorV1);

// Shadow protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore shadow::layer_shadow_manager_v1::LayerShadowManagerV1);

// Manual Dispatch impl for shadow surface object since it has custom user data
impl<T: 'static> Dispatch<shadow::layer_shadow_surface_v1::LayerShadowSurfaceV1, shadow::ShadowData>
    for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &shadow::layer_shadow_surface_v1::LayerShadowSurfaceV1,
        _event: <shadow::layer_shadow_surface_v1::LayerShadowSurfaceV1 as Proxy>::Event,
        _data: &shadow::ShadowData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for shadow objects
    }
}

// Auto-hide protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore layer_auto_hide::layer_auto_hide_manager_v1::LayerAutoHideManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore layer_usable_area::layer_usable_area_manager_v1::LayerUsableAreaManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore layer_size_transition::layer_size_transition_manager_v1::LayerSizeTransitionManagerV1);
#[cfg(all(feature = "screencopy", feature = "workspaces"))]
delegate_noop!(@<T> WindowState<T>: ignore cosmic_protocols::image_capture_source::v1::client::zcosmic_workspace_image_capture_source_manager_v1::ZcosmicWorkspaceImageCaptureSourceManagerV1);
#[cfg(all(feature = "screencopy", feature = "workspaces"))]
delegate_noop!(@<T> WindowState<T>: ignore kora_image_capture_size::kora_image_capture_size_manager_v1::KoraImageCaptureSizeManagerV1);

// Tooltip protocol delegates
delegate_noop!(@<T> WindowState<T>: ignore tooltip::zcosmic_tooltip_manager_v1::ZcosmicTooltipManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore capture_exclusion::kora_capture_exclusion_manager_v1::KoraCaptureExclusionManagerV1);
delegate_noop!(@<T> WindowState<T>: ignore capture_exclusion::kora_surface_capture_exclusion_v1::KoraSurfaceCaptureExclusionV1);

// Manual Dispatch impl for tooltip object to handle reposition events
impl<T: 'static> Dispatch<tooltip::zcosmic_tooltip_v1::ZcosmicTooltipV1, tooltip::TooltipData>
    for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &tooltip::zcosmic_tooltip_v1::ZcosmicTooltipV1,
        event: <tooltip::zcosmic_tooltip_v1::ZcosmicTooltipV1 as Proxy>::Event,
        _data: &tooltip::TooltipData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use tooltip::zcosmic_tooltip_v1::Event;
        match event {
            Event::Reposition { x: _, y: _ } => {
                // Informational only — compositor handles positioning
            }
        }
    }
}

// Manual Dispatch impl for auto-hide surface object to handle visibility_changed events
impl<T: 'static>
    Dispatch<
        layer_auto_hide::layer_auto_hide_v1::LayerAutoHideV1,
        layer_auto_hide::LayerAutoHideData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &layer_auto_hide::layer_auto_hide_v1::LayerAutoHideV1,
        event: <layer_auto_hide::layer_auto_hide_v1::LayerAutoHideV1 as Proxy>::Event,
        data: &layer_auto_hide::LayerAutoHideData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use layer_auto_hide::layer_auto_hide_v1::Event;
        match event {
            Event::VisibilityChanged { visible } => {
                let is_visible = visible != 0;
                log::debug!("Auto-hide visibility changed: visible={}", is_visible);
                // Resolve the originating surface to its window id so that each
                // surface in multi-surface (`AllScreens`) mode receives its own
                // visibility event. Without this the message carries `None` and
                // the consumer misattributes it to the first window, so only one
                // monitor's panel ever toggles its input region.
                let window_id = state.get_id_from_surface(&data.surface);
                state.auto_hide_visible = is_visible;
                state.message.push((
                    window_id,
                    DispatchMessageInner::AutoHideVisibilityChanged(is_visible),
                ));
            }
        }
    }
}

// The manager receives the transition events directly; there is no per-surface
// object, so they are attributed to no window.
impl<T: 'static>
    Dispatch<
        workspace_transition::workspace_transition_manager_v1::WorkspaceTransitionManagerV1,
        (),
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &workspace_transition::workspace_transition_manager_v1::WorkspaceTransitionManagerV1,
        event: <workspace_transition::workspace_transition_manager_v1::WorkspaceTransitionManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use workspace_transition::WorkspaceTransition;
        use workspace_transition::workspace_transition_manager_v1::Event;
        let transition = match event {
            Event::Started {
                from,
                to,
                duration_ms,
            } => WorkspaceTransition::Started {
                from,
                to,
                duration_ms,
            },
            Event::Finished { to } => WorkspaceTransition::Finished { to },
        };
        state
            .message
            .push((None, DispatchMessageInner::WorkspaceTransition(transition)));
    }
}

impl<T: 'static>
    Dispatch<
        layer_size_transition::layer_size_transition_v1::LayerSizeTransitionV1,
        layer_size_transition::LayerSizeTransitionData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &layer_size_transition::layer_size_transition_v1::LayerSizeTransitionV1,
        event: <layer_size_transition::layer_size_transition_v1::LayerSizeTransitionV1 as Proxy>::Event,
        data: &layer_size_transition::LayerSizeTransitionData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use layer_size_transition::SizeTransition;
        use layer_size_transition::layer_size_transition_v1::Event;
        let transition = match event {
            Event::Started {
                from_width,
                from_height,
                to_width,
                to_height,
                duration_ms,
            } => SizeTransition::Started {
                from: (from_width, from_height),
                to: (to_width, to_height),
                duration_ms,
            },
            Event::Finished { width, height } => SizeTransition::Finished {
                size: (width, height),
            },
        };
        let window_id = state.get_id_from_surface(&data.surface);
        state
            .message
            .push((window_id, DispatchMessageInner::SizeTransition(transition)));
    }
}

// Manual Dispatch impl for the usable-area surface object to handle usable_area
// events (the non-exclusive area of the surface's output).
impl<T: 'static>
    Dispatch<
        layer_usable_area::layer_usable_area_v1::LayerUsableAreaV1,
        layer_usable_area::LayerUsableAreaData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &layer_usable_area::layer_usable_area_v1::LayerUsableAreaV1,
        event: <layer_usable_area::layer_usable_area_v1::LayerUsableAreaV1 as Proxy>::Event,
        data: &layer_usable_area::LayerUsableAreaData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use layer_usable_area::layer_usable_area_v1::Event;
        match event {
            Event::UsableArea {
                x,
                y,
                width,
                height,
            } => {
                // Attribute to the originating surface's window id so multi-
                // surface (`AllScreens`) consumers receive the right output's
                // usable area.
                let window_id = state.get_id_from_surface(&data.surface);
                state.message.push((
                    window_id,
                    DispatchMessageInner::UsableAreaChanged {
                        x,
                        y,
                        width,
                        height,
                    },
                ));
            }
        }
    }
}

// Adaptive foreground protocol delegates.
// The manager is event-less; the surface object carries the readings.
delegate_noop!(@<T> WindowState<T>: ignore adaptive_foreground::adaptive_foreground_manager_v1::AdaptiveForegroundManagerV1);

impl<T: 'static>
    Dispatch<
        adaptive_foreground::adaptive_foreground_surface_v1::AdaptiveForegroundSurfaceV1,
        adaptive_foreground::AdaptiveForegroundData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &adaptive_foreground::adaptive_foreground_surface_v1::AdaptiveForegroundSurfaceV1,
        event: <adaptive_foreground::adaptive_foreground_surface_v1::AdaptiveForegroundSurfaceV1 as Proxy>::Event,
        data: &adaptive_foreground::AdaptiveForegroundData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use adaptive_foreground::adaptive_foreground_surface_v1::Event;

        let surface_id = data.surface.id().protocol_id();
        match event {
            Event::Luminance { index, luminance } => {
                // Held until `done`, so a surface with several zones restyles
                // once rather than once per zone. A repeat of the same index
                // within a batch replaces the earlier value rather than
                // appending, which would leave the client applying whichever it
                // read last by accident.
                let readings = state
                    .adaptive_foreground_pending
                    .entry(surface_id)
                    .or_default();
                let luminance = adaptive_foreground::decode_luminance(luminance);
                match readings.iter_mut().find(|(i, _)| *i == index) {
                    Some(slot) => slot.1 = luminance,
                    None => readings.push((index, luminance)),
                }
            }
            Event::Done => {
                let Some(readings) = state.adaptive_foreground_pending.remove(&surface_id) else {
                    return;
                };
                if readings.is_empty() {
                    return;
                }
                log::debug!(
                    "adaptive_foreground: {} reading(s) for surface {}",
                    readings.len(),
                    surface_id
                );
                let window_id = state.get_id_from_surface(&data.surface);
                state.message.push((
                    window_id,
                    DispatchMessageInner::AdaptiveForeground { readings },
                ));
            }
        }
    }
}

// Special action protocol delegates
impl<T: 'static>
    Dispatch<
        special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1,
        special_action::SpecialActionManagerData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1,
        _event: <special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1 as Proxy>::Event,
        _data: &special_action::SpecialActionManagerData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // The manager has no events.
    }
}

impl<T: 'static>
    Dispatch<
        special_action::zcosmic_special_action_v1::ZcosmicSpecialActionV1,
        special_action::SpecialActionData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &special_action::zcosmic_special_action_v1::ZcosmicSpecialActionV1,
        event: <special_action::zcosmic_special_action_v1::ZcosmicSpecialActionV1 as Proxy>::Event,
        _data: &special_action::SpecialActionData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use special_action::SpecialActionEvent;
        use special_action::zcosmic_special_action_v1::Event;

        let resolved = match event {
            Event::Activate => SpecialActionEvent::Activate,
            Event::HoldStart => SpecialActionEvent::HoldStart,
            Event::HoldEnd => SpecialActionEvent::HoldEnd,
            Event::Cancel => SpecialActionEvent::Cancel,
        };
        log::debug!("Special action event: {:?}", resolved);
        state
            .message
            .push((None, DispatchMessageInner::SpecialAction(resolved)));
    }
}

// Layer surface visibility protocol delegates
impl<T: 'static>
    Dispatch<
        layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1,
        layer_surface_visibility::LayerSurfaceVisibilityManagerData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1,
        _event: <layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1 as Proxy>::Event,
        _data: &layer_surface_visibility::LayerSurfaceVisibilityManagerData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for manager
    }
}

impl<T: 'static>
    Dispatch<
        layer_surface_visibility::zcosmic_layer_surface_visibility_v1::ZcosmicLayerSurfaceVisibilityV1,
        layer_surface_visibility::LayerSurfaceVisibilityData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &layer_surface_visibility::zcosmic_layer_surface_visibility_v1::ZcosmicLayerSurfaceVisibilityV1,
        event: <layer_surface_visibility::zcosmic_layer_surface_visibility_v1::ZcosmicLayerSurfaceVisibilityV1 as Proxy>::Event,
        data: &layer_surface_visibility::LayerSurfaceVisibilityData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use layer_surface_visibility::zcosmic_layer_surface_visibility_v1::Event;
        let Event::VisibilityChanged { visible } = event;
        let visible = visible != 0;
        let surface_id = data.surface.id().protocol_id();
        log::debug!(
            "Layer surface visibility changed: surface={}, visible={}",
            surface_id,
            visible
        );
        // Authoritative update from compositor – overwrite optimistic local state.
        if visible {
            state.hidden_surfaces.remove(&surface_id);
        } else {
            state.hidden_surfaces.insert(surface_id);
        }
        // Name the surface that actually changed, as auto-hide does above. A
        // process can own several surfaces and hide them independently -- the
        // desktop, its input, the strip that stands in for it -- and a message
        // that carries `None` is misattributed to the first window, so every
        // surface hears about a visibility change that belongs to one of them.
        let window_id = state.get_id_from_surface(&data.surface);
        state.message.push((
            window_id,
            DispatchMessageInner::SurfaceVisibilityChanged(visible),
        ));
    }
}

// Layer surface dismiss protocol delegates
impl<T: 'static>
    Dispatch<
        layer_surface_dismiss::zcosmic_layer_surface_dismiss_manager_v1::ZcosmicLayerSurfaceDismissManagerV1,
        layer_surface_dismiss::LayerSurfaceDismissManagerData,
    > for WindowState<T>
{
    fn event(
        _state: &mut Self,
        _proxy: &layer_surface_dismiss::zcosmic_layer_surface_dismiss_manager_v1::ZcosmicLayerSurfaceDismissManagerV1,
        _event: <layer_surface_dismiss::zcosmic_layer_surface_dismiss_manager_v1::ZcosmicLayerSurfaceDismissManagerV1 as Proxy>::Event,
        _data: &layer_surface_dismiss::LayerSurfaceDismissManagerData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No events for manager
    }
}

impl<T: 'static>
    Dispatch<
        layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::ZcosmicLayerSurfaceDismissV1,
        layer_surface_dismiss::LayerSurfaceDismissData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        _proxy: &layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::ZcosmicLayerSurfaceDismissV1,
        event: <layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::ZcosmicLayerSurfaceDismissV1 as Proxy>::Event,
        data: &layer_surface_dismiss::LayerSurfaceDismissData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use layer_surface_dismiss::zcosmic_layer_surface_dismiss_v1::Event;
        let Event::DismissRequested = event;
        log::debug!(
            "Dismiss requested for surface {:?}",
            data.surface.id().protocol_id()
        );
        state.dismiss_requested = true;
        state
            .message
            .push((None, DispatchMessageInner::DismissRequested));
    }
}

// Layer surface dismiss protocol implementation
#[allow(private_interfaces)]
impl<T: 'static> layer_surface_dismiss::LayerSurfaceDismissHandler for WindowState<T> {
    fn dismiss_requested(&mut self, _surface: &WlSurface) {
        log::debug!("Dismiss requested for surface");
        self.dismiss_requested = true;
        self.message
            .push((None, DispatchMessageInner::DismissRequested));
    }
}

// Foreign toplevel protocol implementation
#[cfg(feature = "foreign-toplevel")]
#[allow(private_interfaces)]
impl<T: 'static> foreign_toplevel::ForeignToplevelHandler for WindowState<T> {
    fn foreign_toplevel_event(&mut self, event: foreign_toplevel::ForeignToplevelEvent) {
        log::trace!("Queuing foreign toplevel event: {:?}", event);
        self.message
            .push((None, DispatchMessageInner::ForeignToplevel(event)));
    }

    fn get_toplevel_data(&mut self, id: u32) -> &mut foreign_toplevel::ToplevelHandleData {
        self.foreign_toplevel_data
            .entry(id)
            .or_insert_with(foreign_toplevel::ToplevelHandleData::default)
    }

    fn remove_toplevel_data(&mut self, id: u32) {
        self.foreign_toplevel_data.remove(&id);
    }

    fn store_toplevel_handle(
        &mut self,
        id: u32,
        handle: wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    ) {
        self.foreign_toplevel_handles.insert(id, handle);
    }

    fn remove_toplevel_handle(&mut self, id: u32) {
        self.foreign_toplevel_handles.remove(&id);
    }

    fn get_toplevel_handle(&self, id: u32) -> Option<&wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1>{
        self.foreign_toplevel_handles.get(&id)
    }

    fn store_ext_toplevel_handle(
        &mut self,
        id: u32,
        handle: wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    ) {
        self.ext_toplevel_handles.insert(id, handle);
    }

    fn remove_ext_toplevel_handle(&mut self, id: u32) {
        self.ext_toplevel_handles.remove(&id);
        #[cfg(feature = "cosmic-toplevel")]
        {
            self.toplevel_identity.remove(id);
            if let Some(handle) = self.cosmic_toplevel_handles.remove(&id) {
                handle.destroy();
            }
            self.cosmic_to_ext_handle_map
                .retain(|_, ext_id| *ext_id != id);
        }
        #[cfg(feature = "screencopy")]
        self.screencopy.forget_toplevel(id);
    }

    fn get_ext_toplevel_handle(&self, id: u32) -> Option<&wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1>{
        self.ext_toplevel_handles.get(&id)
    }

    #[cfg(feature = "cosmic-toplevel")]
    fn store_cosmic_toplevel_handle(
        &mut self,
        id: u32,
        handle: cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        log::debug!("Storing COSMIC toplevel handle for id {}", id);
        self.cosmic_toplevel_handles.insert(id, handle);
    }

    #[cfg(feature = "cosmic-toplevel")]
    fn remove_cosmic_toplevel_handle(&mut self, id: u32) {
        self.cosmic_toplevel_handles.remove(&id);
    }

    #[cfg(feature = "cosmic-toplevel")]
    fn get_cosmic_toplevel_handle(&self, id: u32) -> Option<&cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>{
        self.cosmic_toplevel_handles.get(&id)
    }

    #[cfg(feature = "cosmic-toplevel")]
    fn get_cosmic_toplevel_manager(&self) -> Option<&cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1>{
        self.cosmic_toplevel_manager.as_ref()
    }

    #[cfg(feature = "cosmic-toplevel")]
    fn cosmic_toplevel_info(&self) -> Option<&cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1>{
        self.cosmic_toplevel_info.as_ref()
    }
}

// Screencopy handler implementation
#[cfg(feature = "screencopy")]
impl<T: 'static> screencopy::ScreencopyHandler for WindowState<T> {
    fn screencopy_event(&mut self, event: screencopy::ScreencopyEvent) {
        log::trace!("Queuing screencopy event");
        self.message
            .push((None, DispatchMessageInner::Screencopy(event)));
    }

    fn screencopy_state(&self) -> &screencopy::ScreencopyState {
        &self.screencopy
    }

    fn screencopy_state_mut(&mut self) -> &mut screencopy::ScreencopyState {
        &mut self.screencopy
    }

    fn get_shm(&self) -> Option<&wayland_client::protocol::wl_shm::WlShm> {
        self.shm.as_ref().or(self.screencopy_shm.as_ref())
    }

    fn get_ext_toplevel_handle(
        &self,
        id: u32,
    ) -> Option<
        &wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    >{
        self.ext_toplevel_handles.get(&id)
    }

    #[cfg(feature = "workspaces")]
    fn get_ext_workspace_handle(&self, id: u32) -> Option<&ExtWorkspaceHandleV1> {
        self.workspaces.handle(id)
    }
}

// COSMIC toplevel handler implementation
#[cfg(feature = "cosmic-toplevel")]
#[allow(private_interfaces)]
impl<T: 'static> foreign_toplevel::CosmicToplevelHandler for WindowState<T> {
    fn cosmic_toplevel_info(&self) -> Option<&cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1>{
        self.cosmic_toplevel_info.as_ref()
    }

    fn set_cosmic_handle_mapping(&mut self, cosmic_id: u32, ext_id: u32) {
        self.cosmic_to_ext_handle_map.insert(cosmic_id, ext_id);
    }

    fn get_ext_handle_id(&self, cosmic_id: u32) -> Option<u32> {
        self.cosmic_to_ext_handle_map.get(&cosmic_id).copied()
    }
}

// Foreign toplevel manager dispatch
#[cfg(feature = "foreign-toplevel")]
impl<T: 'static>
    Dispatch<
        wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        foreign_toplevel::ForeignToplevelManagerData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        event: wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::Event,
        data: &foreign_toplevel::ForeignToplevelManagerData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
            foreign_toplevel::ForeignToplevelManagerData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        <() as Dispatch<
            wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
            foreign_toplevel::ForeignToplevelManagerData,
            Self,
        >>::event_created_child(opcode, qhandle)
    }
}

// Foreign toplevel handle dispatch
#[cfg(feature = "foreign-toplevel")]
impl<T: 'static>
    Dispatch<
        wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        foreign_toplevel::ToplevelHandleUserData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        event: wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::Event,
        data: &foreign_toplevel::ToplevelHandleUserData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
            foreign_toplevel::ToplevelHandleUserData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

// ext_foreign_toplevel_list_v1 dispatch
#[cfg(feature = "foreign-toplevel")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        foreign_toplevel::ExtForeignToplevelListData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        event: wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::Event,
        data: &foreign_toplevel::ExtForeignToplevelListData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        #[cfg(feature = "cosmic-toplevel")]
        let new_toplevel = match &event {
            wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => Some(toplevel.clone()),
            _ => None,
        };
        <() as Dispatch<
            wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            foreign_toplevel::ExtForeignToplevelListData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle);
        #[cfg(feature = "cosmic-toplevel")]
        if let Some(toplevel) = new_toplevel {
            state.toplevel_identity.request(toplevel, qhandle);
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        <() as Dispatch<
            wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            foreign_toplevel::ExtForeignToplevelListData,
            Self,
        >>::event_created_child(opcode, qhandle)
    }
}

// ext_foreign_toplevel_handle_v1 dispatch
#[cfg(feature = "foreign-toplevel")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        foreign_toplevel::ExtToplevelHandleData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::Event,
        data: &foreign_toplevel::ExtToplevelHandleData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        use foreign_toplevel::ForeignToplevelHandler;
        #[cfg(feature = "cosmic-toplevel")]
        use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::Event;

        let id = proxy.id().protocol_id();
        if state.get_ext_toplevel_handle(id) != Some(proxy) {
            return;
        }
        #[cfg(feature = "cosmic-toplevel")]
        let identifier_changed = matches!(&event, Event::Identifier { .. });

        // Forward to blanket impl
        <() as Dispatch<
            wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
            foreign_toplevel::ExtToplevelHandleData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle);
        #[cfg(feature = "cosmic-toplevel")]
        if identifier_changed {
            state.refresh_toplevel_identity(id, false);
        }
    }
}

// zcosmic_toplevel_info_v1 dispatch
#[cfg(feature = "cosmic-toplevel")]
impl<T: 'static>
    Dispatch<
        cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        foreign_toplevel::CosmicToplevelInfoData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        event: cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::Event,
        data: &foreign_toplevel::CosmicToplevelInfoData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
            foreign_toplevel::CosmicToplevelInfoData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

// zcosmic_toplevel_handle_v1 dispatch
#[cfg(feature = "cosmic-toplevel")]
impl<T: 'static>
    Dispatch<
        cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        foreign_toplevel::CosmicToplevelHandleData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::Event,
        data: &foreign_toplevel::CosmicToplevelHandleData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if state.cosmic_toplevel_handles.get(&data.ext_handle_id) != Some(proxy) {
            return;
        }
        let closed = matches!(event, cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::Event::Closed);
        <() as Dispatch<
            cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
            foreign_toplevel::CosmicToplevelHandleData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle);
        if closed {
            state.cosmic_toplevel_handles.remove(&data.ext_handle_id);
            state.cosmic_to_ext_handle_map.remove(&proxy.id().protocol_id());
        }
    }
}

// zcosmic_toplevel_manager_v1 dispatch (for control operations)
#[cfg(feature = "cosmic-toplevel")]
impl<T: 'static>
    Dispatch<
        cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
        foreign_toplevel::CosmicToplevelManagerData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
        event: cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::Event,
        data: &foreign_toplevel::CosmicToplevelManagerData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
            foreign_toplevel::CosmicToplevelManagerData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

// Screencopy protocol dispatches
#[cfg(feature = "screencopy")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
        screencopy::CaptureManagerData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
        event: <wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1 as Proxy>::Event,
        data: &screencopy::CaptureManagerData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
            screencopy::CaptureManagerData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
        screencopy::CaptureSourceManagerData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
        event: <wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1 as Proxy>::Event,
        data: &screencopy::CaptureSourceManagerData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
            screencopy::CaptureSourceManagerData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        screencopy::ImageCaptureSourceData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        event: <wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1 as Proxy>::Event,
        data: &screencopy::ImageCaptureSourceData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
            screencopy::ImageCaptureSourceData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
        screencopy::CaptureSessionData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
        event: wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::Event,
        data: &screencopy::CaptureSessionData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
            screencopy::CaptureSessionData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static>
    Dispatch<
        wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
        screencopy::CaptureFrameData,
    > for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
        event: wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::Event,
        data: &screencopy::CaptureFrameData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
            screencopy::CaptureFrameData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static> Dispatch<wayland_client::protocol::wl_buffer::WlBuffer, screencopy::BufferData>
    for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_client::protocol::wl_buffer::WlBuffer,
        event: <wayland_client::protocol::wl_buffer::WlBuffer as Proxy>::Event,
        data: &screencopy::BufferData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_client::protocol::wl_buffer::WlBuffer,
            screencopy::BufferData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

#[cfg(feature = "screencopy")]
impl<T: 'static> Dispatch<wayland_client::protocol::wl_shm_pool::WlShmPool, screencopy::ShmPoolData>
    for WindowState<T>
{
    fn event(
        state: &mut Self,
        proxy: &wayland_client::protocol::wl_shm_pool::WlShmPool,
        event: <wayland_client::protocol::wl_shm_pool::WlShmPool as Proxy>::Event,
        data: &screencopy::ShmPoolData,
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        <() as Dispatch<
            wayland_client::protocol::wl_shm_pool::WlShmPool,
            screencopy::ShmPoolData,
            Self,
        >>::event(state, proxy, event, data, conn, qhandle)
    }
}

impl<T: 'static> WindowState<T> {
    /// The logical layout of every output (global coords), kept current as
    /// outputs are added and removed. Used to position/move a layer surface
    /// across monitors.
    pub fn output_layout(&self) -> &[OutputLayoutItem] {
        &self.output_layout
    }

    /// build a new WindowState
    pub fn build(mut self) -> Result<Self, LayerEventError> {
        let connection = if let Some(connection) = self.connection.take() {
            connection
        } else {
            Connection::connect_to_env()?
        };
        let (globals, _) = registry_queue_init::<BaseState>(&connection)?;

        self.display = Some(connection.display());
        let mut event_queue = connection.new_event_queue::<WindowState<T>>();
        let qh = event_queue.handle();

        let wmcompositer = globals.bind::<WlCompositor, _, _>(&qh, 1..=5, ())?;

        let shm = globals.bind::<WlShm, _, _>(&qh, 1..=1, ())?;
        self.shm = Some(shm);
        // Bind wl_seat up to v7 (like winit/SCTK). v5+ delivers axis_source,
        // axis_stop, axis_discrete and `frame` grouping — without which a mouse
        // wheel only produces the legacy continuous `axis` event and is
        // misclassified as a touchpad-style PixelDelta instead of a LineDelta.
        let seat = globals.bind::<WlSeat, _, _>(&qh, 1..=7, ())?;
        self.seat = Some(seat);

        // Drag-and-drop (receive only): bind the data device manager and get a
        // data device for the seat, so the compositor delivers DnD offers from
        // other apps to our surfaces. Version 3 gives us the action-negotiation
        // (`set_actions`) + `finish` that some compositors require before they
        // emit the drop. Optional: absence just means no DnD support.
        match globals.bind::<WlDataDeviceManager, _, _>(&qh, 1..=3, ()) {
            Ok(manager) => {
                log::info!(target: "kcopy_dnd", "bound wl_data_device_manager v{}", manager.version());
                if let Some(seat) = self.seat.as_ref() {
                    let dd = manager.get_data_device(seat, &qh, ());
                    log::info!(target: "kcopy_dnd", "created wl_data_device v{}", dd.version());
                    self.data_device = Some(dd);
                } else {
                    log::warn!(target: "kcopy_dnd", "no seat — data device not created");
                }
                self.data_device_manager = Some(manager);
            }
            Err(e) => log::warn!(target: "kcopy_dnd", "no wl_data_device_manager: {e}"),
        }

        let wmbase = globals.bind::<XdgWmBase, _, _>(&qh, 2..=6, ())?;
        self.wmbase = Some(wmbase);

        let cursor_manager = globals
            .bind::<WpCursorShapeManagerV1, _, _>(&qh, 1..=1, ())
            .ok();
        let viewporter = globals.bind::<WpViewporter, _, _>(&qh, 1..=1, ()).ok();

        let _ = connection.display().get_registry(&qh, ()); // so if you want WlOutput, you need to
        // register this

        let xdg_output_manager = globals.bind::<ZxdgOutputManagerV1, _, _>(&qh, 1..=3, ())?; // bind
        // xdg_output_manager

        let decoration_manager = globals
            .bind::<ZxdgDecorationManagerV1, _, _>(&qh, 1..=1, ())
            .ok();

        self.xdg_decoration_manager = decoration_manager;

        let fractional_scale_manager = globals
            .bind::<WpFractionalScaleManagerV1, _, _>(&qh, 1..=1, ())
            .ok();
        let text_input_manager = globals
            .bind::<ZwpTextInputManagerV3, _, _>(&qh, 1..=1, ())
            .ok();

        self.text_input_manager = text_input_manager;

        // Ask for xdg-activation so a layer surface can request keyboard focus.
        // Optional: on a compositor without it, `request_focus_for_surface` is a
        // no-op and the surface keeps waiting for a click.
        self.xdg_activation = globals.bind::<XdgActivationV1, _, _>(&qh, 1..=1, ()).ok();
        if self.xdg_activation.is_some() {
            log::info!("Successfully bound xdg_activation_v1 for layer-surface focus requests");
        }

        // Always try to bind blur manager for dynamic blur support
        // (allows requesting blur on any surface, like popups, even if main window doesn't have blur)
        self.blur_manager = globals
            .bind::<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, _, _>(&qh, 1..=6, ())
            .ok();
        if self.blur_manager.is_some() {
            log::info!("Successfully bound org_kde_kwin_blur_manager protocol for blur support");
        }

        // The standard protocol too, so a compositor implementing either one gets
        // the same region. Bound here because the loop takes `globals` when it starts.
        self.background_effect_manager = globals
            .bind::<background_effect::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if let Some(manager) = &self.background_effect_manager {
            log::info!(
                "Bound background effect manager (version {})",
                manager.version()
            );
        }

        // Bind adaptive foreground, for surfaces that draw onto the wallpaper and
        // have to stay legible against whatever it happens to be.
        self.adaptive_foreground_manager = globals
            .bind::<adaptive_foreground::adaptive_foreground_manager_v1::AdaptiveForegroundManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.adaptive_foreground_manager.is_some() {
            log::info!(
                "Successfully bound adaptive_foreground_manager_v1 protocol for backdrop luminance"
            );
        }

        // Always try to bind corner radius manager for dynamic corner radius support
        // (allows setting corner radius at runtime even if not set initially)
        self.corner_radius_manager = globals
            .bind::<corner_radius::layer_corner_radius_manager_v1::LayerCornerRadiusManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.corner_radius_manager.is_some() {
            log::info!(
                "Successfully bound layer_corner_radius_manager_v1 protocol for corner radius support"
            );
        }

        // Always try to bind the compositor-side placement manager so a surface
        // can ask to be positioned within the usable area at any time.
        self.layer_surface_placement_manager = globals
            .bind::<layer_surface_placement::layer_surface_placement_manager_v1::LayerSurfacePlacementManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.layer_surface_placement_manager.is_some() {
            log::info!(
                "Successfully bound layer_surface_placement_manager_v1 protocol for compositor-side placement"
            );
        }

        // Always try to bind the compositor-side edge-resize manager so a surface
        // can opt into the compositor-drawn resize sash at any time.
        self.layer_edge_resize_manager = globals
            .bind::<layer_edge_resize::layer_edge_resize_manager_v1::LayerEdgeResizeManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.layer_edge_resize_manager.is_some() {
            log::info!(
                "Successfully bound layer_edge_resize_manager_v1 protocol for compositor-drawn edge resize"
            );
        }

        // Always try to bind shadow manager for dynamic shadow support
        // (allows requesting shadow on any surface, like popups, even if main window doesn't have shadow)
        self.shadow_manager = globals
            .bind::<shadow::layer_shadow_manager_v1::LayerShadowManagerV1, _, _>(&qh, 1..=1, ())
            .ok();
        if self.shadow_manager.is_some() {
            log::info!("Successfully bound layer_shadow_manager_v1 protocol for shadow support");
        }

        self.bind_keyboard_shortcuts_inhibit_manager(&globals, &qh);
        self.bind_capture_exclusion_manager(&globals, &qh);

        // Always try to bind layer auto-hide manager for compositor-driven auto-hide support
        self.auto_hide_manager = globals
            .bind::<layer_auto_hide::layer_auto_hide_manager_v1::LayerAutoHideManagerV1, _, _>(
                &qh,
                1..=2,
                (),
            )
            .ok();
        if self.auto_hide_manager.is_some() {
            log::info!(
                "Successfully bound layer_auto_hide_manager_v1 protocol for auto-hide support"
            );
        }

        // Always try to bind the usable-area manager so surfaces learn their
        // output's non-exclusive area (output size minus panels/docks).
        self.usable_area_manager = globals
            .bind::<layer_usable_area::layer_usable_area_manager_v1::LayerUsableAreaManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.usable_area_manager.is_some() {
            log::info!(
                "Successfully bound layer_usable_area_manager_v1 protocol for usable-area reporting"
            );
        }

        self.size_transition_manager = globals
            .bind::<layer_size_transition::layer_size_transition_manager_v1::LayerSizeTransitionManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.size_transition_manager.is_some() {
            log::info!("Successfully bound layer_size_transition_manager_v1");
        }

        // Every desktop of every Kora workspace, and which workspace each is.
        #[cfg(feature = "workspaces")]
        {
            self.workspaces.manager = globals
                .bind::<wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::ExtWorkspaceManagerV1, _, _>(
                    &qh,
                    1..=1,
                    ext_workspace::WorkspaceManagerData,
                )
                .ok();
            // Up to 2: v2 lets a machine-plane client put a layer surface in
            // a workspace (`assign_realm`); an older compositor still gives
            // us the realm events at 1.
            self.workspaces.realm_manager = globals
                .bind::<ext_workspace::kora_workspace_realm_manager_v1::KoraWorkspaceRealmManagerV1, _, _>(
                    &qh,
                    1..=2,
                    ext_workspace::RealmData,
                )
                .ok();
            if self.workspaces.manager.is_some() {
                log::info!("Successfully bound ext_workspace_manager_v1");
            }
            if self.workspaces.realm_manager.is_some() {
                log::info!("Successfully bound kora_workspace_realm_manager_v1");
            }
        }

        // Binding is the opt-in: a bound client receives every transition.
        self.workspace_transition_manager = globals
            .bind::<workspace_transition::workspace_transition_manager_v1::WorkspaceTransitionManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok();
        if self.workspace_transition_manager.is_some() {
            log::info!("Successfully bound workspace_transition_manager_v1");
        }

        // Always try to bind layer surface visibility manager for hide/show support
        // (allows hiding/showing surfaces without destroying them)
        self.layer_surface_visibility_manager = globals
            .bind::<layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1, _, _>(
                &qh,
                1..=5,
                layer_surface_visibility::LayerSurfaceVisibilityManagerData,
            )
            .ok();
        if self.layer_surface_visibility_manager.is_some() {
            log::info!(
                "Successfully bound zcosmic_layer_surface_visibility_manager_v1 protocol for hide/show support"
            );
        }

        // Always try to bind layer surface dismiss manager for dismiss-on-outside-click support
        self.layer_surface_dismiss_manager = globals
            .bind::<layer_surface_dismiss::zcosmic_layer_surface_dismiss_manager_v1::ZcosmicLayerSurfaceDismissManagerV1, _, _>(
                &qh,
                // v2 adds `set_ignore_layer_clicks`; fall back to v1 compositors.
                1..=2,
                layer_surface_dismiss::LayerSurfaceDismissManagerData,
            )
            .ok();
        if self.layer_surface_dismiss_manager.is_some() {
            log::info!(
                "Successfully bound zcosmic_layer_surface_dismiss_manager_v1 protocol for dismiss support"
            );
        }

        // Bind the special action manager if any surface wants the key.
        if self.special_action {
            self.special_action_manager = globals
                .bind::<special_action::zcosmic_special_action_manager_v1::ZcosmicSpecialActionManagerV1, _, _>(
                    &qh,
                    1..=1,
                    special_action::SpecialActionManagerData,
                )
                .ok();
            if self.special_action_manager.is_none() {
                log::warn!(
                    "Special action requested but compositor does not support zcosmic_special_action_v1"
                );
            }
        }

        // Bind foreign toplevel protocols if enabled
        // We need zwlr_foreign_toplevel_manager for control operations (activate, close, etc.)
        // ext_foreign_toplevel_list + cosmic_toplevel_info provide better info but no control
        #[cfg(feature = "foreign-toplevel")]
        if self.foreign_toplevel_enabled {
            // Try ext_foreign_toplevel_list_v1 first (standard protocol for info)
            self.ext_foreign_toplevel_list = globals
                .bind::<wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, _, _>(
                    &qh,
                    1..=1,
                    foreign_toplevel::ExtForeignToplevelListData::default(),
                )
                .ok();

            if self.ext_foreign_toplevel_list.is_some() {
                log::info!(
                    "Successfully bound ext_foreign_toplevel_list_v1 protocol for foreign toplevel tracking"
                );
            }

            // Try to bind COSMIC protocols for state info and control
            #[cfg(feature = "cosmic-toplevel")]
            {
                self.toplevel_identity.manager = globals.bind(&qh, 1..=1, ()).ok();

                // COSMIC toplevel info (for state info like minimized/maximized)
                self.cosmic_toplevel_info = globals
                    .bind::<cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, _, _>(
                        &qh,
                        2..=3, // 2 has get_cosmic_toplevel, 3 ext_workspace_enter
                        foreign_toplevel::CosmicToplevelInfoData::default(),
                    )
                    .ok();
                if self.cosmic_toplevel_info.is_some() {
                    log::info!(
                        "Successfully bound zcosmic_toplevel_info_v1 protocol for toplevel state info"
                    );
                } else {
                    log::debug!(
                        "zcosmic_toplevel_info_v1 not available - state info will be limited"
                    );
                }

                // COSMIC toplevel manager (for control - activate, close, etc.)
                // Version 5+ needed for force_close support
                self.cosmic_toplevel_manager = globals
                    .bind::<cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1, _, _>(
                        &qh,
                        1..=5,
                        foreign_toplevel::CosmicToplevelManagerData::default(),
                    )
                    .ok();
                if self.cosmic_toplevel_manager.is_some() {
                    log::info!(
                        "Successfully bound zcosmic_toplevel_manager_v1 protocol for toplevel control"
                    );
                } else {
                    log::debug!(
                        "zcosmic_toplevel_manager_v1 not available - trying wlr fallback for control"
                    );
                }
            }

            // Fall back to wlr_foreign_toplevel_manager_v1 if COSMIC manager not available
            #[cfg(feature = "cosmic-toplevel")]
            let has_cosmic_manager = self.cosmic_toplevel_manager.is_some();
            #[cfg(not(feature = "cosmic-toplevel"))]
            let has_cosmic_manager = false;

            if !has_cosmic_manager {
                self.foreign_toplevel_manager = globals
                    .bind::<wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, _, _>(
                        &qh,
                        1..=3,
                        foreign_toplevel::ForeignToplevelManagerData::default(),
                    )
                    .ok();

                if self.foreign_toplevel_manager.is_some() {
                    log::info!(
                        "Successfully bound zwlr_foreign_toplevel_manager_v1 protocol for foreign toplevel management"
                    );
                }
            }

            // Check if we have at least one way to track toplevels
            let has_info =
                self.ext_foreign_toplevel_list.is_some() || self.foreign_toplevel_manager.is_some();
            #[cfg(feature = "cosmic-toplevel")]
            let has_control =
                self.cosmic_toplevel_manager.is_some() || self.foreign_toplevel_manager.is_some();
            #[cfg(not(feature = "cosmic-toplevel"))]
            let has_control = self.foreign_toplevel_manager.is_some();

            if !has_info {
                log::warn!(
                    "Foreign toplevel tracking requested but compositor does not support any toplevel info protocols"
                );
            }
            if !has_control {
                log::warn!(
                    "Foreign toplevel control (activate, close, etc.) not available - no supported protocol found"
                );
            }
        }

        // Bind screencopy protocols if enabled
        #[cfg(feature = "screencopy")]
        if self.foreign_toplevel_enabled {
            self.screencopy.capture_manager = globals
                .bind::<wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1, _, _>(
                    &qh,
                    1..=1,
                    screencopy::CaptureManagerData::default(),
                )
                .ok();
            if self.screencopy.capture_manager.is_some() {
                log::info!("Successfully bound ext_image_copy_capture_manager_v1 for screencopy");
            }

            self.screencopy.source_manager = globals
                .bind::<wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(
                    &qh,
                    1..=1,
                    screencopy::CaptureSourceManagerData::default(),
                )
                .ok();
            if self.screencopy.source_manager.is_some() {
                log::info!(
                    "Successfully bound ext_foreign_toplevel_image_capture_source_manager_v1 for screencopy"
                );
            }
            #[cfg(feature = "workspaces")]
            {
                self.screencopy.workspace_source_manager = globals
                    .bind::<cosmic_protocols::image_capture_source::v1::client::zcosmic_workspace_image_capture_source_manager_v1::ZcosmicWorkspaceImageCaptureSourceManagerV1, _, _>(
                        &qh,
                        1..=1,
                        (),
                    )
                    .ok();
                if self.screencopy.workspace_source_manager.is_some() {
                    log::info!(
                        "Successfully bound zcosmic_workspace_image_capture_source_manager_v1"
                    );
                }
                // Previews drawn small by the compositor; without it they come
                // full size and are shrunk here.
                self.screencopy.size_manager = globals
                    .bind::<kora_image_capture_size::kora_image_capture_size_manager_v1::KoraImageCaptureSizeManagerV1, _, _>(
                        &qh,
                        1..=1,
                        (),
                    )
                    .ok();
                if self.screencopy.size_manager.is_some() {
                    log::info!("Successfully bound kora_image_capture_size_manager_v1");
                }
            }

            if self.screencopy.is_available() {
                log::info!("Screencopy support is available");
            } else {
                log::debug!(
                    "Screencopy not fully available (missing one or both protocol globals)"
                );
            }
        }

        event_queue.blocking_dispatch(&mut self)?; // then make a dispatch

        // Watch every output's xdg_output (name + global logical position +
        // size), so consumers can place a surface across monitors. The watch
        // lives as long as the output, and the registry handler adds and drops
        // entries as monitors come and go, so the layout never freezes at this
        // startup snapshot.
        for (_, output_display) in &self.outputs {
            let zxdgoutput = xdg_output_manager.get_xdg_output(output_display, &qh, ());
            self.output_watch
                .push((output_display.clone(), ZxdgOutputInfo::new(zxdgoutput)));
        }
        if !self.output_watch.is_empty() {
            event_queue.roundtrip(&mut self)?;
            self.sync_output_layout(true);
        }
        // The startup outputs are watched; any output that appears from here on
        // is watched by the registry handler, which needs the manager for it.
        self.xdg_output_manager = Some(xdg_output_manager.clone());

        // do the step before, you get empty list

        // so it is the same way, to get surface detach to protocol, first get the shell, like wmbase
        // or layer_shell or session-shell, then get `surface` from the wl_surface you get before, and
        // set it
        // finally thing to remember is to commit the surface, make the shell to init.
        //let (init_w, init_h) = self.size;
        // this example is ok for both xdg_surface and layer_shell
        if self.is_background() {
            let background_surface = wmcompositer.create_surface(&qh, ());
            if self.events_transparent {
                let region = wmcompositer.create_region(&qh, ());
                background_surface.set_input_region(Some(&region));
                region.destroy();
            }
            self.background_surface = Some(background_surface);
        } else if !self.is_allscreens() {
            let mut output = None;

            let (binded_output, binded_xdginfo) = match self.start_mode.clone() {
                StartMode::TargetScreen(name) => {
                    for (_, output_display) in &self.outputs {
                        let zxdgoutput = xdg_output_manager.get_xdg_output(output_display, &qh, ());
                        self.xdg_info_cache
                            .push((output_display.clone(), ZxdgOutputInfo::new(zxdgoutput)));
                    }
                    event_queue.blocking_dispatch(&mut self)?; // then make a dispatch
                    if let Some(cache) = self
                        .xdg_info_cache
                        .iter()
                        .find(|(_, info)| info.name == *name)
                        .cloned()
                    {
                        output = Some(cache.clone());
                    }
                    self.xdg_info_cache.clear();
                    let binded_output = output.as_ref().map(|(output, _)| output).cloned();
                    let binded_xdginfo = output.as_ref().map(|(_, xdginfo)| xdginfo).cloned();
                    (binded_output, binded_xdginfo)
                }
                StartMode::TargetOutput(output) => (Some(output), None),
                _ => (None, None),
            };

            let wl_surface = wmcompositer.create_surface(&qh, ()); // and create a surface. if two or more,
            let layer_shell = globals
                .bind::<ZwlrLayerShellV1, _, _>(&qh, 3..=5, ())
                .unwrap();
            let layer = layer_shell.get_layer_surface(
                &wl_surface,
                binded_output.as_ref(),
                self.layer,
                self.namespace.clone(),
                &qh,
                (),
            );
            layer.set_anchor(self.anchor);
            layer.set_keyboard_interactivity(self.keyboard_interactivity);
            if let Some((init_w, init_h)) = self.size {
                layer.set_size(init_w, init_h);
            }

            if let Some(zone) = self.exclusive_zone {
                layer.set_exclusive_zone(zone);
            }

            if let Some((top, right, bottom, left)) = self.margin {
                layer.set_margin(top, right, bottom, left);
            }

            if self.events_transparent {
                let region = wmcompositer.create_region(&qh, ());
                wl_surface.set_input_region(Some(&region));
                region.destroy();
            }

            // Apply blur effect if enabled
            if self.blur {
                apply_blur_to_surface(
                    &self.blur_manager,
                    &wl_surface,
                    &qh,
                    self.blur_radius,
                    self.blur_saturation,
                    self.blur_tint,
                    self.blur_border,
                );
            }

            // Apply corner radius if set
            let surface_id = wl_surface.id().protocol_id();
            if self.corner_radius.is_some()
                && let Some(corner_obj) = apply_corner_radius_to_surface(
                    &self.corner_radius_manager,
                    self.corner_radius,
                    &wl_surface,
                    &qh,
                )
            {
                self.corner_radius_surfaces.insert(surface_id, corner_obj);
            }

            // Apply shadow if enabled
            if self.shadow {
                apply_shadow_to_surface(&self.shadow_manager, &wl_surface, &qh);
            }

            // Register the surface to receive the device's special key.
            if self.special_action
                && let Some(receiver) = register_special_action_for_surface(
                    &self.special_action_manager,
                    &wl_surface,
                    &qh,
                    self.special_action_default && self.special_action_receivers.is_empty(),
                )
            {
                self.special_action_receivers.insert(surface_id, receiver);
            }

            // Register surface for compositor usable-area reporting.
            if let Some(obj) =
                register_usable_area_for_surface(&self.usable_area_manager, &wl_surface, &qh)
            {
                self.usable_area_surfaces.insert(surface_id, obj);
            }

            wl_surface.commit();

            let mut fractional_scale = None;
            if let Some(ref fractional_scale_manager) = fractional_scale_manager {
                fractional_scale =
                    Some(fractional_scale_manager.get_fractional_scale(&wl_surface, &qh, ()));
            }
            let viewport = viewporter
                .as_ref()
                .map(|viewport| viewport.get_viewport(&wl_surface, &qh, ()));
            // so during the init Configure of the shell, a buffer, atleast a buffer is needed.
            // and if you need to reconfigure it, you need to commit the wl_surface again
            // so because this is just an example, so we just commit it once
            // like if you want to reset anchor or KeyboardInteractivity or resize, commit is needed
            self.push_window(
                WindowStateUnitBuilder::new(
                    id::Id::unique(),
                    qh.clone(),
                    connection.display(),
                    wl_surface,
                    Shell::LayerShell(layer),
                )
                .viewport(viewport)
                .zxdgoutput(binded_xdginfo)
                .fractional_scale(fractional_scale)
                .wl_output(binded_output.clone())
                // Mark as created so remove_shell() tears this surface down when
                // the compositor sends `Closed` (e.g. its output was disabled).
                // Without this the single-window `Active` surface lingers as a
                // zombie (wl_surface stays alive), so `has_live_surface()` never
                // goes false and the surface can be neither cleaned up nor moved
                // to a surviving output. `becreated` is only read by remove_shell;
                // its other use (`is_created` in refresh events) is discarded by
                // iced_layershell, so this has no other behavioral effect.
                .becreated(true)
                .build(),
            );
        } else {
            let displays = self.outputs.clone();
            for (_, output_display) in displays.iter() {
                let wl_surface = wmcompositer.create_surface(&qh, ()); // and create a surface. if two or more,
                let layer_shell = globals
                    .bind::<ZwlrLayerShellV1, _, _>(&qh, 3..=5, ())
                    .unwrap();
                let layer = layer_shell.get_layer_surface(
                    &wl_surface,
                    Some(output_display),
                    self.layer,
                    self.namespace.clone(),
                    &qh,
                    (),
                );
                layer.set_anchor(self.anchor);
                layer.set_keyboard_interactivity(self.keyboard_interactivity);
                if let Some((init_w, init_h)) = self.size {
                    layer.set_size(init_w, init_h);
                }

                if let Some(zone) = self.exclusive_zone {
                    layer.set_exclusive_zone(zone);
                }

                if let Some((top, right, bottom, left)) = self.margin {
                    layer.set_margin(top, right, bottom, left);
                }

                if self.events_transparent {
                    let region = wmcompositer.create_region(&qh, ());
                    wl_surface.set_input_region(Some(&region));
                    region.destroy();
                }

                // Apply blur / corner radius / shadow / layer visibility
                // (shared with the runtime NewDisplay path).
                self.apply_surface_effects(&wl_surface, &qh);

                wl_surface.commit();

                let zxdgoutput = xdg_output_manager.get_xdg_output(output_display, &qh, ());
                let mut fractional_scale = None;
                if let Some(ref fractional_scale_manager) = fractional_scale_manager {
                    fractional_scale =
                        Some(fractional_scale_manager.get_fractional_scale(&wl_surface, &qh, ()));
                }
                let viewport = viewporter
                    .as_ref()
                    .map(|viewport| viewport.get_viewport(&wl_surface, &qh, ()));
                // so during the init Configure of the shell, a buffer, atleast a buffer is needed.
                // and if you need to reconfigure it, you need to commit the wl_surface again
                // so because this is just an example, so we just commit it once
                // like if you want to reset anchor or KeyboardInteractivity or resize, commit is needed

                self.push_window(
                    WindowStateUnitBuilder::new(
                        id::Id::unique(),
                        qh.clone(),
                        connection.display(),
                        wl_surface,
                        Shell::LayerShell(layer),
                    )
                    .viewport(viewport)
                    .zxdgoutput(Some(ZxdgOutputInfo::new(zxdgoutput)))
                    .fractional_scale(fractional_scale)
                    .wl_output(Some(output_display.clone()))
                    // Mark as created so remove_shell() tears the panel down when
                    // the compositor sends `Closed` (monitor disabled).
                    .becreated(true)
                    .build(),
                );
            }
            // Every startup `NewDisplay` is answered by a surface created above,
            // so drop them — but keep the startup layout, or
            // `output_layout_subscription` would stay silent until a hotplug.
            self.message
                .retain(|(_, m)| matches!(m, DispatchMessageInner::OutputLayoutChanged(_)));
        }
        self.init_finished = true;
        self.viewporter = viewporter;
        self.event_queue = Some(event_queue);
        self.globals = Some(globals);
        self.wl_compositor = Some(wmcompositer);
        self.fractional_scale_manager = fractional_scale_manager;
        self.cursor_manager = cursor_manager;
        self.xdg_output_manager = Some(xdg_output_manager);
        self.connection = Some(connection);

        Ok(self)
    }
    /// main event loop, every time dispatch, it will store the messages, and do callback. it will
    /// pass a LayerShellEvent, with self as mut, the last `Option<usize>` describe which unit the event
    /// happened on, like tell you this time you do a click, what surface it is on. you can use the
    /// index to get the unit, with [WindowState::get_unit_with_id] if the even is not spical on one surface,
    /// it will return [None].
    /// Different with running, it receiver a receiver
    pub fn running_with_proxy<F, Message>(
        self,
        message_receiver: Channel<Message>,
        event_handler: F,
    ) -> Result<(), LayerEventError>
    where
        Message: std::marker::Send + 'static,
        F: FnMut(LayerShellEvent<T, Message>, &mut WindowState<T>, Option<id::Id>) -> ReturnData<T>
            + 'static,
    {
        self.running_with_proxy_option(Some(message_receiver), event_handler)
    }
    /// main event loop, every time dispatch, it will store the messages, and do callback. it will
    /// pass a LayerShellEvent, with self as mut, the last `Option<usize>` describe which unit the event
    /// happened on, like tell you this time you do a click, what surface it is on. you can use the
    /// index to get the unit, with [WindowState::get_unit_with_id] if the even is not spical on one surface,
    /// it will return [None].
    ///
    pub fn running<F>(self, event_handler: F) -> Result<(), LayerEventError>
    where
        F: FnMut(LayerShellEvent<T, ()>, &mut WindowState<T>, Option<id::Id>) -> ReturnData<T>
            + 'static,
    {
        self.running_with_proxy_option(None, event_handler)
    }

    fn running_with_proxy_option<F, Message>(
        mut self,
        message_receiver: Option<Channel<Message>>,
        mut event_handler: F,
    ) -> Result<(), LayerEventError>
    where
        Message: std::marker::Send + 'static,
        F: FnMut(LayerShellEvent<T, Message>, &mut WindowState<T>, Option<id::Id>) -> ReturnData<T>
            + 'static,
    {
        let globals = self.globals.take().unwrap();
        let mut event_queue_origin = self.event_queue.take().unwrap();
        let qh = event_queue_origin.handle();
        self.queue_handle = Some(qh.clone());
        let wmcompositer = self.wl_compositor.take().unwrap();
        let shm = self.shm.take().unwrap();
        self.cached_compositor = Some(wmcompositer.clone());
        self.cached_shm = Some(shm.clone());
        #[cfg(feature = "screencopy")]
        {
            self.screencopy_shm = Some(shm.clone());
        }
        let fractional_scale_manager = self.fractional_scale_manager.take();
        let cursor_manager: Option<WpCursorShapeManagerV1> = self.cursor_manager.take();
        // Clone (don't take): the event-loop closures below use this local to bind
        // xdg_outputs for new surfaces, but the `Dispatch<WlSurface>` enter handler
        // also needs it on `self` to bind the entered output's xdg_output (which is
        // what reports the output's logical size). Taking it here would leave
        // `self.xdg_output_manager` None and silently disable that path.
        let xdg_output_manager = self.xdg_output_manager.clone().unwrap();
        let connection = self.connection.take().unwrap();
        let mut init_event = None;
        let wmbase = self.wmbase.take().unwrap();
        let viewporter = self.viewporter.take();
        let zxdg_decoration_manager = self.xdg_decoration_manager.take();

        let cursor_update_context = CursorUpdateContext {
            cursor_manager,
            qh: qh.clone(),
            connection: connection.clone(),
            shm: shm.clone(),
            wmcompositer: wmcompositer.clone(),
        };

        while !matches!(init_event, Some(ReturnData::None)) {
            match init_event {
                None => {
                    init_event = Some(event_handler(LayerShellEvent::InitRequest, &mut self, None));
                }
                Some(ReturnData::RequestBind) => {
                    init_event = Some(event_handler(
                        LayerShellEvent::BindProvide(&globals, &qh),
                        &mut self,
                        None,
                    ));
                }
                Some(ReturnData::RequestCompositor) => {
                    init_event = Some(event_handler(
                        LayerShellEvent::CompositorProvide(&wmcompositer, &qh),
                        &mut self,
                        None,
                    ));
                }
                _ => panic!("Not provide server here"),
            }
        }

        struct EventWrapper<Raw, F> {
            raw: Raw,
            fun: F,
            loop_handle: LoopHandle<'static, Self>,
        }

        let mut event_loop: EventLoop<_> =
            EventLoop::try_new().expect("Failed to initialize the event loop");

        let event_queue = connection.new_event_queue::<EventWrapper<Self, F>>();
        WaylandSource::new(connection.clone(), event_queue)
            .insert(event_loop.handle())
            .expect("Failed to init wayland source");
        let mut state = EventWrapper {
            raw: self,
            fun: event_handler,
            loop_handle: event_loop.handle(),
        };

        let signal = event_loop.get_signal();

        // Create a ping source for immediate wake-up after channel messages.
        // When a message arrives via the channel, the channel callback pings
        // so the event loop re-dispatches immediately and the ping callback
        // runs NormalDispatch + refresh.
        let (ping_sender, ping_source) =
            calloop::ping::make_ping().expect("Failed to create ping source");
        state.raw.ping_sender = Some(ping_sender);
        // The pass that handles queued messages and presents due frames. Nothing
        // runs it on a timer: the loop's idle callback pings it after Wayland
        // events, queued messages or a frame coming due.
        let (tick_sender, tick_source) =
            calloop::ping::make_ping().expect("Failed to create tick source");
        state.raw.tick_sender = Some(tick_sender);

        // Clone resources needed by the ping callback for the SHM buffer path.
        // For GPU-rendering apps (use_display_handle=true) these are unused.
        let shm_for_ping = shm.clone();
        let qh_for_ping = qh.clone();

        event_loop
            .handle()
            .insert_source(ping_source, move |(), _, r_window_state| {
                let window_state = &mut r_window_state.raw;
                let event_handler = &mut r_window_state.fun;

                // Process pending iced actions (subscriptions, command output).
                window_state.handle_event(
                    &mut *event_handler,
                    LayerShellEvent::NormalDispatch,
                    None,
                );

                // Refresh/present cycle — renders frames immediately.
                let all_hidden = window_state.all_surfaces_hidden();
                for idx in 0..window_state.units.len() {
                    let unit = &mut window_state.units[idx];
                    let (width, height) = unit.size;
                    if width == 0 || height == 0 {
                        continue;
                    }
                    if all_hidden && unit.initial_refresh_sent {
                        continue;
                    }
                    if unit.take_present_slot() {
                        let unit_id = unit.id;
                        let is_created = unit.becreated;
                        let scale_float = unit.scale_float();
                        let wl_surface = unit.wl_surface.clone();
                        if unit.buffer.is_none() && !window_state.use_display_handle {
                            let Ok(mut file) = tempfile::tempfile() else {
                                log::error!("Cannot create new file from tempfile");
                                return;
                            };
                            let ReturnData::WlBuffer(buffer) = (event_handler)(
                                LayerShellEvent::RequestBuffer(
                                    &mut file,
                                    &shm_for_ping,
                                    &qh_for_ping,
                                    width,
                                    height,
                                ),
                                window_state,
                                Some(unit_id),
                            ) else {
                                panic!("You cannot return this one");
                            };
                            wl_surface.attach(Some(&buffer), 0, 0);
                            wl_surface.commit();
                            window_state.units[idx].buffer = Some(buffer);
                        }
                        window_state.refresh_unit(
                            &mut *event_handler,
                            unit_id,
                            (width, height),
                            is_created,
                            scale_float,
                        );
                        window_state.units[idx].reset_present_slot();
                    }
                }
            })
            .expect("Failed to insert ping source");

        // Insert message channel as event source (calloop-style)
        if let Some(channel) = message_receiver {
            event_loop
                .handle()
                .insert_source(channel, |event, _, r_window_state| {
                    let channel::Event::Msg(event) = event else {
                        return;
                    };
                    let window_state = &mut r_window_state.raw;
                    let event_handler = &mut r_window_state.fun;
                    window_state.handle_event(
                        &mut *event_handler,
                        LayerShellEvent::UserEvent(event),
                        None,
                    );
                    // Ping the event loop so NormalDispatch + refresh runs
                    // immediately in this iteration, instead of waiting for
                    // the next timer tick.
                    if let Some(sender) = &window_state.ping_sender {
                        sender.ping();
                    }
                })
                .expect("Failed to insert message channel source");
        }

        event_loop
            .handle()
            .insert_source(
                tick_source,
                move |(), _, r_window_state| {
                    let window_state = &mut r_window_state.raw;
                    let event_handler = &mut r_window_state.fun;

                    let has_pending = window_state.units.iter().any(|u| u.should_refresh());
                    if has_pending {
                        log::debug!("[evloop] timer callback (pending_refresh=true)");
                    }
                    let mut messages = Vec::new();
                    std::mem::swap(&mut messages, &mut window_state.message);
                    for msg in messages.iter() {
                        match msg {
                            (
                                index_info,
                                msg_inner @ DispatchMessageInner::XdgInfoChanged { change_type, .. },
                            ) => {
                                // Raw layershellev consumers get the typed callback
                                // (it only carries the change kind, no dimensions)...
                                window_state.handle_event(
                                    &mut *event_handler,
                                    LayerShellEvent::XdgInfoChanged(*change_type),
                                    *index_info,
                                );
                                // ...but iced_layershell only handles the general
                                // `RequestMessages(DispatchMessage)` path, so the full
                                // message (carrying the output's logical size) must be
                                // forwarded there too — otherwise this arm swallows the
                                // event and the size never reaches the iced app as
                                // `WindowEvent::OutputLogicalSize`.
                                let msg: DispatchMessage = msg_inner.clone().into();
                                window_state.handle_event(
                                    &mut *event_handler,
                                    LayerShellEvent::RequestMessages(&msg),
                                    *index_info,
                                );
                            }
                            (_, DispatchMessageInner::NewDisplay(output_display)) => {
                                // AllScreens always gets one surface per output.
                                //
                                // Single-surface `Active` apps (e.g. the
                                // notifications daemon) normally ignore NewDisplay
                                // — but if their only surface was destroyed because
                                // its output was disabled, nothing would ever
                                // re-create it and ShowWindow becomes a silent no-op
                                // (window_manager is empty). So when an Active app
                                // has NO live surface left, treat this newly-appeared
                                // output as the place to re-create it. The live-unit
                                // guard makes this a no-op at boot and whenever the
                                // surface still exists, so no duplicates are created.
                                let recreate_lost_active =
                                    window_state.is_active() && !window_state.has_live_surface();
                                if !window_state.is_allscreens() && !recreate_lost_active {
                                    continue;
                                }
                                // Active surfaces must remain output-agnostic after
                                // recovery so the compositor can move them on show.
                                let bound_output = (!recreate_lost_active).then_some(output_display);
                                let wl_surface = wmcompositer.create_surface(&qh, ()); // and create a surface. if two or more,
                                let layer_shell = globals
                                    .bind::<ZwlrLayerShellV1, _, _>(&qh, 3..=5, ())
                                    .unwrap();
                                let layer = layer_shell.get_layer_surface(
                                    &wl_surface,
                                    bound_output,
                                    window_state.layer,
                                    window_state.namespace.clone(),
                                    &qh,
                                    (),
                                );
                                layer.set_anchor(window_state.anchor);
                                layer
                                    .set_keyboard_interactivity(window_state.keyboard_interactivity);
                                if let Some((init_w, init_h)) = window_state.size {
                                    layer.set_size(init_w, init_h);
                                }

                                if let Some(zone) = window_state.exclusive_zone {
                                    layer.set_exclusive_zone(zone);
                                }

                                if let Some((top, right, bottom, left)) = window_state.margin {
                                    layer.set_margin(top, right, bottom, left);
                                }

                                if window_state.events_transparent {
                                    let region = wmcompositer.create_region(&qh, ());
                                    wl_surface.set_input_region(Some(&region));
                                    region.destroy();
                                }

                                // Apply blur / corner radius / shadow / layer
                                // visibility. Without this a monitor enabled
                                // after startup gets a panel with none of
                                // these effects (e.g. no blur).
                                window_state.apply_surface_effects(&wl_surface, &qh);

                                wl_surface.commit();

                                let zxdgoutput = bound_output.map(|output| {
                                    ZxdgOutputInfo::new(
                                        xdg_output_manager.get_xdg_output(output, &qh, ()),
                                    )
                                });
                                let mut fractional_scale = None;
                                if let Some(ref fractional_scale_manager) = fractional_scale_manager
                                {
                                    fractional_scale =
                                        Some(fractional_scale_manager.get_fractional_scale(
                                            &wl_surface,
                                            &qh,
                                            (),
                                        ));
                                }
                                let viewport = viewporter
                                    .as_ref()
                                    .map(|viewport| viewport.get_viewport(&wl_surface, &qh, ()));
                                // so during the init Configure of the shell, a buffer, atleast a buffer is needed.
                                // and if you need to reconfigure it, you need to commit the wl_surface again
                                // so because this is just an example, so we just commit it once
                                // like if you want to reset anchor or KeyboardInteractivity or resize, commit is needed

                                window_state.push_window(
                                    WindowStateUnitBuilder::new(
                                        id::Id::unique(),
                                        qh.clone(),
                                        connection.display(),
                                        wl_surface,
                                        Shell::LayerShell(layer),
                                    )
                                    .viewport(viewport)
                                    .zxdgoutput(zxdgoutput)
                                    .fractional_scale(fractional_scale)
                                    .wl_output(bound_output.cloned())
                                    // Mark as created so remove_shell() tears the
                                    // panel down when the compositor sends `Closed`
                                    // (monitor disabled); otherwise re-enabling the
                                    // monitor leaves the old surface alive and the
                                    // panel ends up duplicated.
                                    .becreated(true)
                                    .build(),
                                );
                            }
                            _ => {
                                let (index_message, msg) = msg;

                                let msg: DispatchMessage = msg.clone().into();
                                window_state.handle_event(
                                    &mut *event_handler,
                                    LayerShellEvent::RequestMessages(&msg),
                                    *index_message,
                                );
                            }
                        }
                    }

                    // User events are now processed by the ping source for immediate response.
                    // The timer only handles internal messages and periodic dispatch.
                    window_state.handle_event(
                        &mut *event_handler,
                        LayerShellEvent::NormalDispatch,
                        None,
                    );
                    loop {
                        let mut return_data = vec![];
                        std::mem::swap(&mut window_state.return_data, &mut return_data);

                        for data in return_data {
                            match data {
                                ReturnData::RequestExit => {
                                    signal.stop();
                                    return;
                                }
                                ReturnData::RequestSetCursorShape((shape_name, pointer)) => {
                                    let Some(serial) = window_state.enter_serial else {
                                        continue;
                                    };
                                    set_cursor_shape(
                                        &cursor_update_context,
                                        shape_name,
                                        pointer,
                                        serial,
                                    );
                                }
                                ReturnData::NewLayerShell((
                                    NewLayerShellSettings {
                                        size,
                                        layer,
                                        anchor,
                                        exclusive_zone,
                                        margin,
                                        keyboard_interactivity,
                                        output_option: output_type,
                                        events_transparent,
                                        namespace,
                                        blur,
                                        blur_radius,
                                        blur_saturation,
                                        blur_tint,
                                        blur_border,
                                        shadow,
                                        corner_radius,
                                        transition,
                                        auto_size: _, // Auto-size is handled at the iced level
                                        first_size: _,
                                        start_hidden,
                                        start_hidden_without_animating,
                                    },
                                    id,
                                    info,
                                )) => {
                                    let output = match output_type {
                                        OutputOption::Output(output) => Some(output),
                                        // Bind to a monitor by xdg name (used for
                                        // moving a surface across outputs).
                                        OutputOption::OutputName(ref name) => window_state
                                            .output_handles
                                            .iter()
                                            .find(|(n, _)| n == name)
                                            .map(|(_, o)| o.clone()),
                                        _ => {
                                            let pos = window_state.surface_pos();

                                            let mut output =
                                                pos.and_then(|p| window_state.units[p].wl_output.as_ref());

                                            if window_state.last_wloutput.is_none()
                                                && window_state.outputs.len() > window_state.last_unit_index
                                            {
                                                window_state.last_wloutput = Some(
                                                    window_state.outputs[window_state.last_unit_index]
                                                        .1
                                                        .clone(),
                                                );
                                            }

                                            if matches!(output_type, events::OutputOption::LastOutput) {
                                                output = window_state.last_wloutput.as_ref();
                                            }

                                            output.cloned()
                                        }
                                    };


                                    let wl_surface = wmcompositer.create_surface(&qh, ()); // and create a surface. if two or more,
                                    let layer_shell = globals
                                        .bind::<ZwlrLayerShellV1, _, _>(&qh, 3..=5, ())
                                        .unwrap();
                                    let layer = layer_shell.get_layer_surface(
                                        &wl_surface,
                                        output.as_ref(),
                                        layer,
                                        namespace.unwrap_or_else(|| window_state.namespace.clone()),
                                        &qh,
                                        (),
                                    );
                                    layer.set_anchor(anchor);
                                    layer.set_keyboard_interactivity(keyboard_interactivity);
                                    if let Some((init_w, init_h)) = size {
                                        layer.set_size(init_w, init_h);
                                    }

                                    if let Some(zone) = exclusive_zone {
                                        layer.set_exclusive_zone(zone);
                                    }

                                    if let Some((top, right, bottom, left)) = margin {
                                        layer.set_margin(top, right, bottom, left);
                                    }

                                    if events_transparent {
                                        let region = wmcompositer.create_region(&qh, ());
                                        wl_surface.set_input_region(Some(&region));
                                        region.destroy();
                                    }

                                    // Apply blur if requested
                                    if blur {
                                        // Lazily bind blur manager if not already bound
                                        if window_state.blur_manager.is_none() {
                                            window_state.blur_manager = globals
                                                .bind::<blur::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, _, _>(
                                                    &qh,
                                                    1..=6,
                                                    (),
                                                )
                                                .ok();
                                            if window_state.blur_manager.is_some() {
                                                log::info!("Lazily bound blur manager for NewLayerShell surface");
                                            } else {
                                                log::warn!("Blur requested but compositor does not support org_kde_kwin_blur_manager protocol");
                                            }
                                        }
                                        apply_blur_to_surface(&window_state.blur_manager, &wl_surface, &qh, blur_radius, blur_saturation, blur_tint, blur_border);
                                    }

                                    let surface_id = wl_surface.id().protocol_id();
                                    // Remember this surface's frosted-glass params even when blur
                                    // is deferred (auto-size sets `blur = false` at creation): the
                                    // later BlurChange(true) re-enable reads them back so the tint
                                    // isn't lost to the compositor default.
                                    window_state.blur_params.insert(
                                        surface_id,
                                        (blur_radius, blur_saturation, blur_tint, blur_border),
                                    );

                                    // Apply corner radius if set (per-surface setting takes precedence, then fallback to window_state)
                                    let effective_corner_radius = corner_radius.or(window_state.corner_radius);
                                    log::debug!("NewLayerShell: corner_radius={:?}, effective={:?}", corner_radius, effective_corner_radius);
                                    if effective_corner_radius.is_some()
                                        && let Some(corner_obj) = apply_corner_radius_to_surface(&window_state.corner_radius_manager, effective_corner_radius, &wl_surface, &qh) {
                                            window_state.corner_radius_surfaces.insert(surface_id, corner_obj);
                                        }

                                    // Apply shadow if enabled (per-surface setting takes precedence, then fallback to window_state)
                                    log::debug!("NewLayerShell: shadow={}, window_state.shadow={}, shadow_manager present={}", 
                                        shadow, window_state.shadow, window_state.shadow_manager.is_some());
                                    if shadow || window_state.shadow {
                                        apply_shadow_to_surface(&window_state.shadow_manager, &wl_surface, &qh);
                                    } else {
                                        log::debug!("NewLayerShell: shadow not requested for this surface");
                                    }

                                    // Register the surface to receive the device's special key.
                                    if window_state.special_action
                                        && let Some(receiver) = register_special_action_for_surface(
                                            &window_state.special_action_manager,
                                            &wl_surface,
                                            &qh,
                                            window_state.special_action_default && window_state.special_action_receivers.is_empty(),
                                        )
                                    {
                                        window_state.special_action_receivers.insert(surface_id, receiver);
                                    }


                                    // Register surface for compositor usable-area reporting.
                                    if let Some(obj) = register_usable_area_for_surface(
                                        &window_state.usable_area_manager,
                                        &wl_surface,
                                        &qh,
                                    ) {
                                        window_state
                                            .usable_area_surfaces
                                            .insert(surface_id, obj);
                                    }

                                    // Record any per-surface transition override so subsequent
                                    // hide/show (which only know the surface id) can honor it.
                                    if let Some(t) = transition {
                                        window_state
                                            .transitions
                                            .insert(wl_surface.id().protocol_id(), t);
                                    }

                                    // If start_hidden is requested, create a visibility
                                    // controller and set hidden BEFORE the first commit so
                                    // the compositor already knows the surface is hidden
                                    // when it processes the initial buffer.
                                    //
                                    // We create the controller directly here instead of
                                    // going through hide_surface() because at daemon boot
                                    // self.units may be empty (no QueueHandle available).
                                    if start_hidden {
                                        let surface_id = wl_surface.id().protocol_id();
                                        window_state.hidden_surfaces.insert(surface_id);
                                        let transition = window_state.surface_transition(surface_id);
                                        if let Some(manager) = &window_state.layer_surface_visibility_manager {
                                            let visibility_data = layer_surface_visibility::LayerSurfaceVisibilityData {
                                                surface: wl_surface.clone(),
                                                hidden: true,
                                            };
                                            let controller = manager.get_visibility_controller(&wl_surface, &qh, visibility_data);
                                            apply_transition_to_controller(&controller, transition);
                                            // A surface that maps hidden and STAYS hidden has
                                            // nothing on screen to animate away, and the
                                            // compositor cannot tell that from an ordinary hide.
                                            // Saying so costs it the hide animation, which is
                                            // why it is opt-in: a surface shown a moment later
                                            // wants that animation still in flight.
                                            if start_hidden_without_animating && controller.version() >= 4 {
                                                controller.set_hidden_immediately();
                                            } else {
                                                if start_hidden_without_animating {
                                                    log::debug!(
                                                        "start_hidden_without_animating asked for but compositor layer_surface_visibility is v{}; falling back to an animated hide",
                                                        controller.version()
                                                    );
                                                }
                                                controller.set_hidden();
                                            }
                                            window_state.layer_surface_visibility_controllers.insert(surface_id, controller);
                                            if let Some(ref conn) = window_state.connection {
                                                let _ = conn.flush();
                                            }
                                            log::debug!("start_hidden: created controller and hid surface {}", surface_id);
                                        } else {
                                            log::warn!("start_hidden: visibility manager not available");
                                        }
                                    } else if let Some(t) = transition
                                        && let Some(manager) = &window_state.layer_surface_visibility_manager
                                    {
                                        // The compositor animates the first buffer in, so it needs the role first.
                                        let visibility_data = layer_surface_visibility::LayerSurfaceVisibilityData {
                                            surface: wl_surface.clone(),
                                            hidden: false,
                                        };
                                        let controller = manager.get_visibility_controller(&wl_surface, &qh, visibility_data);
                                        apply_transition_to_controller(&controller, Some(t));
                                        window_state
                                            .layer_surface_visibility_controllers
                                            .insert(wl_surface.id().protocol_id(), controller);
                                    }

                                    wl_surface.commit();

                                    let mut fractional_scale = None;
                                    if let Some(ref fractional_scale_manager) =
                                        fractional_scale_manager
                                    {
                                        fractional_scale =
                                            Some(fractional_scale_manager.get_fractional_scale(
                                                &wl_surface,
                                                &qh,
                                                (),
                                            ));
                                    }
                                    let viewport = viewporter.as_ref().map(|viewport| {
                                        viewport.get_viewport(&wl_surface, &qh, ())
                                    });
                                    // so during the init Configure of the shell, a buffer, atleast a buffer is needed.
                                    // and if you need to reconfigure it, you need to commit the wl_surface again
                                    // so because this is just an example, so we just commit it once
                                    // like if you want to reset anchor or KeyboardInteractivity or resize, commit is needed

                                    window_state.push_window(
                                        WindowStateUnitBuilder::new(
                                            id,
                                            qh.clone(),
                                            connection.display(),
                                            wl_surface,
                                            Shell::LayerShell(layer),
                                        )
                                        .viewport(viewport)
                                        .fractional_scale(fractional_scale)
                                        .wl_output(output)
                                        .binding(info)
                                        .becreated(true)
                                        .build(),
                                    );
                                }
                                ReturnData::NewPopUp((
                                    NewPopUpSettings {
                                        size: (width, height),
                                        position: (x, y),
                                        id,
                                        blur,
                                        shadow: _,
                                        corner_radius,
                                        auto_size: _, // Auto-size is handled at the iced level
                                        anchor_rect_size,
                                        anchor,
                                        gravity,
                                        constraint_adjustment,
                                        offset,
                                        reactive,
                                        grab,
                                        input_passthrough,
                                        tooltip_offset,
                                        tooltip_anchor,
                                        tooltip_delay_ms,
                                        capture_excluded,
                                    },
                                    targetid,
                                    info,
                                )) => {
                                    let Some(index) = window_state
                                        .units
                                        .iter()
                                        .position(|unit| !unit.is_popup() && unit.id == id)
                                    else {
                                        continue;
                                    };
                                    let wl_surface = wmcompositer.create_surface(&qh, ());
                                    let (ar_w, ar_h) = anchor_rect_size.unwrap_or((width as i32, height as i32));
                                    let placement = PopupPlacement {
                                        parent: id,
                                        grab,
                                        wm_base: wmbase.clone(),
                                        anchor_rect: (x, y, ar_w, ar_h),
                                        anchor,
                                        gravity,
                                        constraint_adjustment,
                                        offset,
                                        reactive,
                                    };
                                    let positioner = placement.positioner(&qh, (width, height));
                                    let wl_xdg_surface =
                                        wmbase.get_xdg_surface(&wl_surface, &qh, ());
                                    let popup =
                                        wl_xdg_surface.get_popup(None, &positioner, &qh, ());
                                    positioner.destroy();

                                    let Shell::LayerShell(shell) = &window_state.units[index].shell
                                    else {
                                        unreachable!()
                                    };
                                    shell.get_popup(&popup);

                                    if grab
                                        && let Some(seat) = window_state.seat.as_ref() {
                                            popup.grab(seat, window_state.enter_serial.unwrap_or(0));
                                        }

                                    // Apply corner radius to popup surface if set
                                    let surface_id = wl_surface.id().protocol_id();
                                    if corner_radius.is_some()
                                        && let Some(corner_obj) = apply_corner_radius_to_surface(
                                            &window_state.corner_radius_manager,
                                            corner_radius,
                                            &wl_surface,
                                            &qh,
                                        ) {
                                            window_state.corner_radius_surfaces.insert(surface_id, corner_obj);
                                        }

                                    // Shadow for popups is deferred until the first
                                    // content frame to avoid a visible flash of
                                    // shadow around an empty/transparent surface.
                                    // iced_layershell applies ShadowChange after
                                    // the popup's first present.

                                    // Let the compositor place a tooltip popup at the pointer.
                                    let tooltip = TooltipPlacement {
                                        offset: tooltip_offset,
                                        anchor: tooltip_anchor,
                                        delay_ms: tooltip_delay_ms,
                                    };
                                    if let Some(tooltip_obj) = attach_tooltip(
                                        &mut window_state.tooltip_manager,
                                        &globals,
                                        &qh,
                                        &wl_surface,
                                        &window_state.units[index].wl_surface,
                                        tooltip,
                                    ) {
                                        window_state.tooltip_surfaces.insert(surface_id, tooltip_obj);
                                    }

                                    // Before the first commit, so no capture ever holds the popup.
                                    if capture_excluded {
                                        if window_state.capture_exclusion_manager.is_none() {
                                            window_state.capture_exclusion_manager = globals
                                                .bind::<capture_exclusion::kora_capture_exclusion_manager_v1::KoraCaptureExclusionManagerV1, _, _>(
                                                    &qh,
                                                    1..=1,
                                                    (),
                                                )
                                                .ok();
                                        }
                                        if let Some(manager) = &window_state.capture_exclusion_manager {
                                            let exclusion = manager.get_surface_exclusion(&wl_surface, &qh, ());
                                            exclusion.set_excluded(1);
                                            window_state.capture_exclusions.insert(surface_id, exclusion);
                                        }
                                    }

                                    // Set empty input region so pointer events pass through
                                    if input_passthrough {
                                        let region = wmcompositer.create_region(&qh, ());
                                        wl_surface.set_input_region(Some(&region));
                                        region.destroy();
                                    }

                                    // Always set window geometry so the compositor
                                    // knows the visible content bounds for anchor
                                    // calculations/constraint adjustment.
                                    wl_xdg_surface.set_window_geometry(0, 0, width as i32, height as i32);

                                    // A popup frosts like the surface it opens from,
                                    // or with the global blur when that has none.
                                    if blur {
                                        let parent = window_state.units[index].wl_surface.id().protocol_id();
                                        let params = window_state.blur_params.get(&parent).copied().unwrap_or((
                                            window_state.blur_radius,
                                            window_state.blur_saturation,
                                            window_state.blur_tint,
                                            window_state.blur_border,
                                        ));
                                        window_state.blur_params.insert(surface_id, params);
                                        window_state.set_blur_for_surface(&wl_surface, true);
                                    }

                                    let mut fractional_scale = None;
                                    if let Some(ref fractional_scale_manager) =
                                        fractional_scale_manager
                                    {
                                        fractional_scale =
                                            Some(fractional_scale_manager.get_fractional_scale(
                                                &wl_surface,
                                                &qh,
                                                (),
                                            ));
                                    }
                                    wl_surface.commit();

                                    let viewport = viewporter.as_ref().map(|viewport| {
                                        viewport.get_viewport(&wl_surface, &qh, ())
                                    });
                                    window_state.push_window(
                                        WindowStateUnitBuilder::new(
                                            targetid,
                                            qh.clone(),
                                            connection.display(),
                                            wl_surface,
                                            Shell::PopUp((popup, wl_xdg_surface)),
                                        )
                                        .size((width, height))
                                        .viewport(viewport)
                                        .fractional_scale(fractional_scale)
                                        .binding(info)
                                        .becreated(true)
                                        .popup_placement(placement)
                                        .build(),
                                    );
                                },
                                ReturnData::RepositionPopUp(settings) => {
                                    use events::RepositionPopUpSettings;
                                    let RepositionPopUpSettings {
                                        popup_id,
                                        size: (width, height),
                                        position: (x, y),
                                        anchor_rect_size,
                                        anchor,
                                        gravity,
                                        constraint_adjustment,
                                        offset,
                                        reactive,
                                    } = settings;

                                    let Some(unit) = window_state
                                        .units
                                        .iter_mut()
                                        .find(|unit| unit.id == popup_id)
                                    else {
                                        log::warn!("RepositionPopUp: popup unit not found for id {:?}", popup_id);
                                        continue;
                                    };

                                    let Shell::PopUp((ref xdg_popup, ref xdg_surface)) = unit.shell else {
                                        log::warn!("RepositionPopUp: unit is not a popup");
                                        continue;
                                    };

                                    let Some(previous) = unit.popup_placement.clone() else {
                                        log::warn!("RepositionPopUp: popup has no placement");
                                        continue;
                                    };
                                    let (ar_w, ar_h) = anchor_rect_size.unwrap_or((width as i32, height as i32));
                                    let placement = PopupPlacement {
                                        anchor_rect: (x, y, ar_w, ar_h),
                                        anchor,
                                        gravity,
                                        constraint_adjustment,
                                        offset,
                                        reactive,
                                        ..previous
                                    };
                                    let positioner = placement.positioner(&qh, (width, height));
                                    let token = next_reposition_token();
                                    xdg_popup.reposition(&positioner, token);
                                    positioner.destroy();
                                    xdg_surface.set_window_geometry(0, 0, width as i32, height as i32);
                                    // A later resize keeps this placement.
                                    unit.popup_placement = Some(placement);

                                    log::debug!("RepositionPopUp: repositioned popup {:?} with token {}", popup_id, token);
                                },
                                ReturnData::NewXdgBase((
                                NewXdgWindowSettings { maximized, title, size, app_id, max_size },
                                    id,
                                    info,
                                )) => {
                                    let wl_surface = wmcompositer.create_surface(&qh, ());
                                    let wl_xdg_surface =
                                        wmbase.get_xdg_surface(&wl_surface, &qh, ());
                                    let toplevel =
                                        wl_xdg_surface.get_toplevel(&qh, ());

                                    toplevel.set_title(title.unwrap_or("".to_owned()));
                                    if let Some(app_id) = app_id {
                                        toplevel.set_app_id(app_id);
                                    }

                                    if let Some((width, height)) = max_size {
                                        toplevel.set_max_size(
                                            i32::try_from(width).unwrap_or(i32::MAX),
                                            i32::try_from(height).unwrap_or(i32::MAX),
                                        );
                                    }
                                    if maximized { toplevel.set_maximized(); }
                                    let decoration = if let Some(decoration_manager) = &zxdg_decoration_manager {
                                        let decoration = decoration_manager.get_toplevel_decoration(&toplevel, &qh, ());
                                        use zxdg_toplevel_decoration_v1::Mode;
                                        // Use client-side (no) decorations when maximized, server-side otherwise
                                        if maximized {
                                            decoration.set_mode(Mode::ClientSide);
                                        } else {
                                            decoration.set_mode(Mode::ServerSide);
                                        }
                                        Some(decoration)

                                    } else {
                                        None
                                    };
                                    let mut fractional_scale = None;
                                    if let Some(ref fractional_scale_manager) =
                                        fractional_scale_manager
                                    {
                                        fractional_scale =
                                            Some(fractional_scale_manager.get_fractional_scale(
                                                &wl_surface,
                                                &qh,
                                                (),
                                            ));
                                    }
                                    wl_surface.commit();

                                    let viewport = viewporter.as_ref().map(|viewport| {
                                        viewport.get_viewport(&wl_surface, &qh, ())
                                    });
                                    window_state.push_window(
                                        WindowStateUnitBuilder::new(
                                            id,
                                            qh.clone(),
                                            connection.display(),
                                            wl_surface,
                                            Shell::XdgTopLevel((toplevel, wl_xdg_surface, decoration)),
                                        )
                                        .size(size.unwrap_or((300, 300)))
                                        .viewport(viewport)
                                        .fractional_scale(fractional_scale)
                                        .binding(info)
                                        .becreated(true)
                                        .build(),
                                    );
                                },

                                ReturnData::NewInputPanel((
                                    NewInputPanelSettings {
                                        size: (width, height),
                                        keyboard,
                                        use_last_output,
                                    },
                                    id,
                                    info,
                                )) => {
                                    let pos = window_state.surface_pos();

                                    let mut output = pos.and_then(|p| window_state.units[p].wl_output.as_ref());

                                    if window_state.last_wloutput.is_none()
                                        && window_state.outputs.len() > window_state.last_unit_index
                                    {
                                        window_state.last_wloutput =
                                            Some(window_state.outputs[window_state.last_unit_index].1.clone());
                                    }

                                    if use_last_output {
                                        output = window_state.last_wloutput.as_ref();
                                    }

                                    if output.is_none() {
                                        output = window_state.outputs.first().map(|(_, o)| o);
                                    }

                                    let Some(output) = output else {
                                        log::warn!("no WlOutput, skip creating input panel");
                                        continue;
                                    };

                                    let wl_surface = wmcompositer.create_surface(&qh, ());
                                    let input_panel = globals
                                        .bind::<ZwpInputPanelV1, _, _>(&qh, 1..=1, ())
                                        .unwrap();
                                    let input_panel_surface =
                                        input_panel.get_input_panel_surface(&wl_surface, &qh, ());
                                    if keyboard {
                                        input_panel_surface.set_toplevel(
                                            output,
                                            ZwpInputPanelPosition::CenterBottom as u32,
                                        );
                                    } else {
                                        input_panel_surface.set_overlay_panel();
                                    }
                                    wl_surface.commit();

                                    let mut fractional_scale = None;
                                    if let Some(ref fractional_scale_manager) = fractional_scale_manager {
                                        fractional_scale =
                                            Some(fractional_scale_manager.get_fractional_scale(
                                                &wl_surface,
                                                &qh,
                                                (),
                                            ));
                                    }

                                    let viewport = viewporter
                                        .as_ref()
                                        .map(|viewport| viewport.get_viewport(&wl_surface, &qh, ()));
                                    window_state.push_window(
                                        WindowStateUnitBuilder::new(
                                            id,
                                            qh.clone(),
                                            connection.display(),
                                            wl_surface,
                                            Shell::InputPanel(input_panel_surface),
                                        )
                                        .size((width, height))
                                        .viewport(viewport)
                                        .fractional_scale(fractional_scale)
                                        .binding(info)
                                        .becreated(true)
                                        .build(),
                                    );
                                },
                                _ => {}
                            }
                        }
                        if window_state.return_data.is_empty() {
                            break;
                        }
                    }

                    let to_be_closed_ids = window_state.closing_order();
                    if !to_be_closed_ids.is_empty() {
                        log::info!(target: "move_debug", "processing {} to_be_closed unit(s) (request_flag.close)", to_be_closed_ids.len());
                    }
                    for id in to_be_closed_ids {
                        window_state.handle_event(
                            &mut *event_handler,
                            LayerShellEvent::RequestMessages(&DispatchMessage::Closed),
                            Some(id),
                        );
                        // event_handler may use unit, only remove it after calling event_handler.
                        window_state.remove_shell(id);
                    }

                    // NOTE: this is for those closed because wl_output is dead.
                    let closed_ids = window_state.closed_ids.clone();
                    for id in closed_ids {
                        window_state.handle_event(
                            &mut *event_handler,
                            LayerShellEvent::RequestMessages(&DispatchMessage::Closed),
                            Some(id),
                        );
                    }
                    window_state.closed_ids.clear();

                    // A single-surface `Active` app (e.g. the notifications
                    // daemon) keeps exactly one layer surface, normally on the
                    // active output. If that output was just disabled, the
                    // surface is destroyed — but unlike plugging in a brand-new
                    // display, no `NewDisplay` fires for the monitors that were
                    // already connected. So nothing would move the surface to a
                    // still-active monitor, and the app goes silently blind until
                    // a display is (re-)enabled.
                    //
                    // Detect that here and synthesize a `NewDisplay` for a
                    // surviving output; the recreate path above turns it into a
                    // fresh surface on the next tick. This runs every 50 ms, so
                    // it self-heals regardless of teardown ordering. Guards:
                    //   - `has_live_surface()` is true for merely *hidden*
                    //     surfaces, so the idle-hidden state never triggers it,
                    //     and it stops the moment the surface is back;
                    //   - the already-queued check prevents piling up duplicates.
                    if window_state.is_active() && !window_state.has_live_surface() {
                        let already_queued = window_state
                            .message
                            .iter()
                            .any(|(_, m)| matches!(m, DispatchMessageInner::NewDisplay(_)));
                        let next_output =
                            window_state.outputs.first().map(|(_, o)| o.clone());
                        if !already_queued
                            && let Some(output) = next_output
                        {
                            window_state
                                .message
                                .push((None, DispatchMessageInner::NewDisplay(output)));
                        }
                    }

                    // Re-check hidden state AFTER NormalDispatch + action
                    // processing.  show_surface() may have been called (e.g.
                    // via the calloop channel between timer ticks, or during
                    // NormalDispatch via a synchronous Task resolution), which
                    // removes the surface from hidden_surfaces.  Using a fresh
                    // check ensures we don't skip the Refresh loop with a
                    // stale all_hidden=true.
                    let all_hidden = window_state.all_surfaces_hidden();

                    // When every surface is hidden via the layer-surface-visibility
                    // protocol the compositor is not rendering us.  Skip the
                    // (expensive) per-unit refresh / present cycle and idle
                    // at a low rate so we still wake up for protocol events
                    // (e.g. show requests).
                    //
                    // Exception: newly created units that have never been
                    // through a refresh cycle must still get their initial
                    // Refresh event so the iced window_manager registers them.
                    // Without this, surfaces created with `start_hidden` would
                    // never be findable by ShowWindow.
                    for idx in 0..window_state.units.len() {
                        let unit = &mut window_state.units[idx];
                        let (width, height) = unit.size;
                        if width == 0 || height == 0 {
                            // don't refresh, if size is 0.
                            continue;
                        }
                        // Skip already-initialized units when all hidden.
                        if all_hidden && unit.initial_refresh_sent {
                            continue;
                        }
                        if unit.take_present_slot() {
                            log::debug!("[evloop] timer: presenting unit {:?}", unit.id);
                            let unit_id = unit.id;
                            let is_created = unit.becreated;
                            let scale_float = unit.scale_float();
                            let wl_surface = unit.wl_surface.clone();
                            if unit.buffer.is_none() && !window_state.use_display_handle {
                                let Ok(mut file) = tempfile::tempfile() else {
                                    log::error!("Cannot create new file from tempfile");
                                    return;
                                };
                                let ReturnData::WlBuffer(buffer) = event_handler(
                                    LayerShellEvent::RequestBuffer(&mut file, &shm, &qh, width, height),
                                    window_state,
                                    Some(unit_id)) else {
                                    panic!("You cannot return this one");
                                };
                                wl_surface.attach(Some(&buffer), 0, 0);
                                wl_surface.commit();
                                window_state.units[idx].buffer = Some(buffer);
                            }
                            window_state.refresh_unit(
                                &mut *event_handler,
                                unit_id,
                                (width, height),
                                is_created,
                                scale_float,
                            );
                            // reset if the slot is not used
                            window_state.units[idx].reset_present_slot();
                        }
                    }

                    // Messages queued during this pass (a surface to recreate)
                    // are for the next one; wait a little so a recreate that
                    // keeps failing can't spin.
                    if !window_state.message.is_empty() {
                        window_state.wake_soon();
                    }
                },
            )
            .expect("Cannot insert_source");

        /// Wake the next pass: now for Wayland events, queued messages or a due
        /// frame; later for a retry or a timed redraw. Nothing else wakes the loop.
        fn schedule_next_pass<T: 'static, F: 'static>(
            window_state: &mut WindowState<T>,
            looph: &LoopHandle<'static, EventWrapper<WindowState<T>, F>>,
            woke_by_events: bool,
        ) {
            if window_state.retry_requested && window_state.retry_timer.is_none() {
                window_state.retry_timer = looph
                    .insert_source(Timer::from_duration(RETRY_DELAY), |_, _, wrapper| {
                        wrapper.raw.retry_timer = None;
                        if let Some(tick) = &wrapper.raw.tick_sender {
                            tick.ping();
                        }
                        TimeoutAction::Drop
                    })
                    .ok();
            }
            window_state.retry_requested = false;

            if window_state.pass_due_now(woke_by_events)
                && let Some(tick) = &window_state.tick_sender
            {
                tick.ping();
            }

            let next = window_state.next_timed_refresh();
            if window_state.refresh_timer.map(|(at, _)| at) == next {
                return;
            }
            if let Some((_, token)) = window_state.refresh_timer.take() {
                looph.remove(token);
            }
            let Some(at) = next else {
                return;
            };
            window_state.refresh_timer = looph
                .insert_source(Timer::from_deadline(at), |_, _, wrapper| {
                    wrapper.raw.refresh_timer = None;
                    if let Some(tick) = &wrapper.raw.tick_sender {
                        tick.ping();
                    }
                    TimeoutAction::Drop
                })
                .ok()
                .map(|token| (at, token));
        }

        // Events read while setting up, before the loop first sleeps.
        let _ = event_queue_origin.dispatch_pending(&mut state.raw);
        if let Some(tick) = &state.raw.tick_sender {
            tick.ping();
        }
        event_loop
            .run(None, &mut state, move |r_window_state| {
                let window_state = &mut r_window_state.raw;
                // The Wayland source reads the socket when it has data; this
                // dispatches what it read for the queue every object lives on.
                let dispatched = event_queue_origin
                    .dispatch_pending(window_state)
                    .unwrap_or(0);
                let looph = &r_window_state.loop_handle;
                schedule_next_pass(window_state, looph, dispatched > 0);
                for token in window_state.to_remove_tokens.iter() {
                    looph.remove(*token);
                }
                window_state.to_remove_tokens.clear();
                if let Some(VirtualKeyRelease { delay, time, key }) =
                    window_state.to_be_released_key
                {
                    looph
                        .insert_source(Timer::from_duration(delay), move |_, _, r_window_state| {
                            let state = &mut r_window_state.raw;
                            let ky = state.get_virtual_keyboard().unwrap();

                            ky.key(time, key, KeyState::Released.into());
                            TimeoutAction::Drop
                        })
                        .ok();
                }
                if let Some(KeyboardTokenState {
                    key,
                    delay,
                    surface_id,
                    pressed_state,
                }) = window_state.repeat_delay.take()
                {
                    let timer = Timer::from_duration(delay);
                    let keyboard_state = window_state.keyboard_state.as_mut().unwrap();
                    keyboard_state.repeat_token = looph
                        .insert_source(timer, move |_, _, r_window_state| {
                            let state = &mut r_window_state.raw;
                            let event_handler = &mut r_window_state.fun;
                            let keyboard_state = match state.keyboard_state.as_mut() {
                                Some(keyboard_state) => keyboard_state,
                                None => return TimeoutAction::Drop,
                            };
                            let repeat_keycode = match keyboard_state.current_repeat {
                                Some(repeat_keycode) => repeat_keycode,
                                None => return TimeoutAction::Drop,
                            };
                            // NOTE: not the same key
                            if repeat_keycode != key {
                                return TimeoutAction::Drop;
                            }
                            if let Some(mut key_context) = keyboard_state.xkb_context.key_context()
                            {
                                let event = key_context.process_key_event(
                                    repeat_keycode,
                                    pressed_state,
                                    false,
                                );
                                let event = DispatchMessageInner::KeyboardInput {
                                    event,
                                    is_synthetic: false,
                                };
                                state.message.push((surface_id, event));
                            }
                            let repeat_info = keyboard_state.repeat_info;

                            let _ = keyboard_state;
                            state.handle_event(
                                &mut *event_handler,
                                LayerShellEvent::NormalDispatch,
                                None,
                            );
                            match repeat_info {
                                RepeatInfo::Repeat { gap, .. } => TimeoutAction::ToDuration(gap),
                                RepeatInfo::Disable => TimeoutAction::Drop,
                            }
                        })
                        .ok();
                }
            })
            .map_err(LayerEventError::EventLoopInitError)?;
        Ok(())
    }

    pub fn request_next_present(&mut self, id: id::Id) {
        self.get_mut_unit_with_id(id)
            .map(WindowStateUnit::request_next_present);
    }

    pub fn reset_present_slot(&mut self, id: id::Id) {
        self.get_mut_unit_with_id(id)
            .map(WindowStateUnit::reset_present_slot);
    }

    pub fn handle_event<F, Message>(
        &mut self,
        mut event_handler: F,
        event: LayerShellEvent<T, Message>,
        unit_id: Option<id::Id>,
    ) where
        Message: std::marker::Send + 'static,
        F: FnMut(LayerShellEvent<T, Message>, &mut WindowState<T>, Option<id::Id>) -> ReturnData<T>,
    {
        let return_data = event_handler(event, self, unit_id);
        if !matches!(return_data, ReturnData::None) {
            self.append_return_data(return_data);
        }
    }
}

fn get_cursor_buffer(
    shape: &str,
    connection: &Connection,
    shm: &WlShm,
) -> Option<CursorImageBuffer> {
    let mut cursor_theme = CursorTheme::load(connection, shm.clone(), 23).ok()?;
    let cursor = cursor_theme.get_cursor(shape);
    Some(cursor?[0].clone())
}

/// avoid too_many_arguments alert in `set_cursor_shape`
struct CursorUpdateContext<T: 'static> {
    cursor_manager: Option<WpCursorShapeManagerV1>,
    qh: QueueHandle<WindowState<T>>,
    connection: Connection,
    shm: WlShm,
    wmcompositer: WlCompositor,
}

fn set_cursor_shape<T: 'static>(
    context: &CursorUpdateContext<T>,
    shape_name: String,
    pointer: WlPointer,
    serial: u32,
) {
    if shape_name == "none" {
        pointer.set_cursor(serial, None, 0, 0);
        return;
    }
    if let Some(cursor_manager) = &context.cursor_manager {
        let Some(shape) = str_to_shape(&shape_name) else {
            log::error!("Not supported shape");
            return;
        };
        let device = cursor_manager.get_pointer(&pointer, &context.qh, ());
        device.set_shape(serial, shape);
        device.destroy();
    } else {
        let Some(cursor_buffer) = get_cursor_buffer(&shape_name, &context.connection, &context.shm)
        else {
            log::error!("Cannot find cursor {shape_name}");
            return;
        };
        let cursor_surface = context.wmcompositer.create_surface(&context.qh, ());
        cursor_surface.attach(Some(&cursor_buffer), 0, 0);
        // and create a surface. if two or more,
        let (hotspot_x, hotspot_y) = cursor_buffer.hotspot();
        pointer.set_cursor(
            serial,
            Some(&cursor_surface),
            hotspot_x as i32,
            hotspot_y as i32,
        );
        cursor_surface.commit();
    }
}

#[cfg(test)]
mod cursor_tests;

#[cfg(test)]
mod keyboard_focus_tests;

#[cfg(test)]
mod background_effect_tests;

#[cfg(test)]
mod toplevel_configure_tests;

#[cfg(test)]
mod shortcuts_inhibit_tests;

#[cfg(test)]
mod requested_surface_tests;

#[cfg(test)]
mod popup_fade_tests;

#[cfg(test)]
mod tooltip_tests;

#[cfg(test)]
mod output_info_tests;

#[cfg(test)]
mod size_answer_tests;

#[cfg(test)]
mod activation_token_tests;

#[cfg(test)]
mod tests {
    use super::{DndAction, may_finish};

    /// The ordinary incoming file drop: a v3 offer on which the compositor
    /// settled on Copy. Finishing tells the source we took the data.
    #[test]
    fn a_negotiated_copy_may_be_finished() {
        assert!(may_finish(3, DndAction::Copy));
    }

    /// The one that kills the process. We ask the compositor for `Copy` alone,
    /// and it hands back the intersection with what the source offers — so a
    /// source advertising only `Move` leaves nothing negotiated, and smithay
    /// answers a `finish` in that state with the `invalid_finish` protocol
    /// error rather than ignoring it. A source that dies mid-drag lands here
    /// too, as does one that never triggered an `action` event at all (it is
    /// only sent when the action CHANGES, and it starts empty).
    #[test]
    fn an_offer_with_no_negotiated_action_is_never_finished() {
        assert!(!may_finish(3, DndAction::empty()));
        assert!(!may_finish(3, DndAction::None));
    }

    /// `finish` does not exist below version 3, so asking for it is a request
    /// the offer has no opcode for.
    #[test]
    fn finish_does_not_exist_before_version_3() {
        assert!(!may_finish(1, DndAction::Copy));
        assert!(!may_finish(2, DndAction::Copy));
    }
}
