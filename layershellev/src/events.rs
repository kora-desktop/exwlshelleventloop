use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::Layer,
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity},
};

use wayland_client::{
    QueueHandle, WEnum,
    globals::GlobalList,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_output::{self, WlOutput},
        wl_pointer::{self, ButtonState, WlPointer},
        wl_shm::WlShm,
    },
};

#[cfg(feature = "foreign-toplevel")]
use crate::foreign_toplevel::ForeignToplevelEvent;
#[cfg(feature = "screencopy")]
use crate::screencopy::ScreencopyEvent;
use crate::{id, xkb_keyboard::KeyEvent};

use crate::keyboard::ModifiersState;

use super::WindowState;

use crate::id::Id;

use std::{fmt::Debug, fs::File};

/// tell program what event is happened
///
/// InitRequest will tell the program is inited, you can request to Bind other wayland-protocols
/// there, with return [ReturnData::RequestBind]
///
/// RequestBuffer request to get the wl-buffer, so you init a buffer_pool here. It return a
/// GlobalList and a QueueHandle. This will enough for bind a extra wayland-protocol, and also,
/// seat can be gotten directly from [WindowState]
///
/// RequestMessages store the DispatchMessage, you can know what happened during dispatch with this
/// event.
pub enum LayerShellEvent<'a, T, Message> {
    /// the first event when start a new gui, program. you can return [ReturnData::None] or
    /// [ReturnData::RequestBind], then it will continue to the next request.
    /// Here only the above two [ReturnData] are acceptable.
    InitRequest,
    /// if the info of the XdgOutput is changed, it will send the event
    XdgInfoChanged(XdgInfoChangedType),
    /// After you return [ReturnData::RequestBind] in the [LayerShellEvent::InitRequest] stage, next
    /// event is [LayerShellEvent::BindProvide], you can use the GlobalList and QueueHandle to create
    /// new wayland objects.
    BindProvide(&'a GlobalList, &'a QueueHandle<WindowState<T>>),
    /// After you return [ReturnData::RequestCompositor] in the init stage, next
    /// event is [LayerShellEvent::CompositorProvide], you can use the WlCompositor and QueueHandle to
    /// create new wayland objects.
    CompositorProvide(&'a WlCompositor, &'a QueueHandle<WindowState<T>>),
    /// create a new buffer after request. if you use display_handle, you do not need to care about
    /// it.
    RequestBuffer(
        &'a mut File,
        &'a WlShm,
        &'a QueueHandle<WindowState<T>>,
        u32,
        u32,
    ),
    /// Some thing KeyboardEvent, TouchEvent, MouseEvent and etc.
    RequestMessages(&'a DispatchMessage),
    /// Nothing happened, you can do some other things after it, like to refresh the ui, and etc.
    NormalDispatch,
    /// It return the event you passed with message_receiver, and return it back.
    UserEvent(Message),
}

/// Define the output for new layershell
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OutputOption {
    LastOutput,

    OutputName(String),

    /// NOTE: The output should be in the same connection with the layershellev, that means if you
    /// want to pass a [wl_output::WlOutput] to create a new layershell, you need to pass your
    /// connection to layershellev first
    Output(wl_output::WlOutput),
    #[default]
    None,
}

/// Show/hide transition animation a layer surface requests from the compositor
/// via the `zcosmic_layer_surface_visibility` protocol (`set_transition`).
///
/// When left unset, the compositor picks the default based on the surface's
/// anchor: a surface anchored to a single lateral edge (left or right) slides
/// in/out from that edge, while everything else fades in/out with a subtle
/// upward slide.  Setting this overrides that default — request
/// [`LayerTransition::Fade`] to keep the fade+rise on an edge-anchored surface,
/// or [`LayerTransition::Slide`] to force the full edge slide (e.g. the dock,
/// the only surface that should).
///
/// Requires compositor support for `zcosmic_layer_surface_visibility` version 2;
/// on older compositors the request is silently ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerTransition {
    /// Slide the whole surface in/out from its anchored screen edge.
    Slide,
    /// Fade in/out with a subtle upward slide (the default for non-edge-anchored
    /// surfaces) — the agentos-panel popover animation.
    Fade,
    /// The design's `fluidReveal` / `fluidDismiss`: rises further than [`Self::Fade`]
    /// and overshoots on the way in, accelerates away on the way out.
    ///
    /// Ask for this when the surface has a BACKDROP BLUR that has to travel with
    /// it. A client animating its own pixels cannot move the blur — that belongs
    /// to the surface, and stays behind as a rectangle.
    FluidReveal,
    /// From here on, what the surface is: the compositor's theme picks its motion.
    Popover,
    Panel,
    ControlPanel,
    Launcher,
    Spotlight,
    Notification,
    ContextMenu,
    Modal,
}

/// layershell settings to create a new layershell surface
#[derive(Debug, Clone, PartialEq)]
pub struct NewLayerShellSettings {
    /// the size of the layershell, optional.
    pub size: Option<(u32, u32)>,
    pub layer: Layer,
    pub anchor: Anchor,
    pub exclusive_zone: Option<i32>,
    pub margin: Option<(i32, i32, i32, i32)>,
    pub keyboard_interactivity: KeyboardInteractivity,
    /// follow the last output of the activated surface, used to create some thing like mako, who
    /// will show on the same window, only when the notifications is cleared, it will change the
    /// wl_output.
    pub output_option: OutputOption,
    pub events_transparent: bool,
    pub namespace: Option<String>,
    /// Request blur effect for this surface (requires compositor support)
    pub blur: bool,
    /// Custom blur radius in pixels. When `blur` is true and this is `Some(r)`,
    /// the compositor applies a blur with radius `r` instead of the default.
    /// Requires compositor support for org_kde_kwin_blur version 2.
    pub blur_radius: Option<f32>,
    /// Backdrop saturation multiplier for the blur (CSS `saturate(x)` semantics).
    /// When `blur` is true and this is `Some(s)`, the compositor saturates the
    /// blurred backdrop by `s` (1.0 = unchanged).
    /// Requires compositor support for org_kde_kwin_blur version 3.
    pub blur_saturation: Option<f32>,
    /// Frosted-glass white-tint strength for the blur. `Some(0.0)` disables the
    /// compositor's white overlay (faithful `backdrop-filter`); `None` keeps the
    /// compositor default. Requires org_kde_kwin_blur version 3.
    pub blur_tint: Option<f32>,
    /// Frosted-glass border strength (alpha) for the blur. `Some(0.0)` disables
    /// the compositor's 1px white border; `None` keeps the compositor default.
    /// Requires org_kde_kwin_blur version 3.
    pub blur_border: Option<f32>,
    /// Request shadow effect for this surface (requires compositor support for layer_shadow_manager_v1)
    pub shadow: bool,
    /// Corner radius in pixels [top_left, top_right, bottom_right, bottom_left]
    /// (requires compositor support for layer_corner_radius_manager_v1)
    pub corner_radius: Option<[u32; 4]>,
    /// Show/hide transition animation for this surface (via the
    /// `layer_surface_visibility` protocol).  Takes precedence over the global
    /// transition set on the [`WindowState`]; `None` falls back to that global
    /// (and ultimately to the compositor's anchor-based heuristic).  Requires
    /// `zcosmic_layer_surface_visibility` version 2.
    pub transition: Option<LayerTransition>,
    /// Auto-size the surface to fit content after first layout.
    /// When true, the initial size is used as a maximum, and the surface will be resized
    /// to match the actual content size after the first render.
    pub auto_size: bool,
    /// The size an auto-sized surface would measure, when the app already knows it. The
    /// surface then maps at this size at once, with its shadow, blur and corners, instead
    /// of at 1x1 and transparent until it has been measured. Later changes are measured.
    pub first_size: Option<(u32, u32)>,
    /// Start the surface hidden (using layer_surface_visibility protocol).
    /// The compositor will not render or give focus to the surface until
    /// `ShowWindow` is sent.  Useful for daemon-mode GPU warm-up where the
    /// first frame should never be visible.
    pub start_hidden: bool,
    /// Whether that initial hide should skip the hide ANIMATION.
    ///
    /// Only meaningful alongside `start_hidden`, and only honoured by a
    /// compositor implementing `layer_surface_visibility` v4.
    ///
    /// A surface mapped hidden has nothing on screen to animate away, but the
    /// compositor sees the same request either way and plays the transition —
    /// so the surface appears for the length of it purely in order to disappear.
    /// Set this for a surface that maps hidden and STAYS hidden.
    ///
    /// Leave it false for one that maps hidden and is shown a moment later: the
    /// hide animation still in flight is what the show animates out of, and
    /// skipping it loses that. A hover preview created per hover wants the
    /// default; a chat input waiting to be summoned wants this.
    pub start_hidden_without_animating: bool,
    /// With `OutputOption::None`, make the surface with no output, not on the output of the
    /// surface last focused, and have the compositor show it under the pointer each time,
    /// not where keyboard focus last went. For surfaces that appear on their own, such as
    /// notifications. Placing it needs `zcosmic_layer_surface_visibility` version 6.
    pub follow_pointer: bool,
    /// With `OutputOption::None`, make the surface with no output and have the compositor
    /// show it on the output of the window with keyboard focus each time. For surfaces about
    /// what the person is typing into, such as dictation feedback. Placing it needs
    /// `zcosmic_layer_surface_visibility` version 7.
    pub follow_focus: bool,
    /// Leave the surface out of screen captures and recordings from its first frame, while
    /// it stays on screen. Needs compositor support for `kora_capture_exclusion_manager_v1`.
    pub capture_excluded: bool,
}

/// be used to create a new popup
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NewPopUpSettings {
    /// the size of the popup
    pub size: (u32, u32),
    /// the position of the popup, relative to the he layersurface
    pub position: (i32, i32),
    /// It means where the popup is, on which surface. It is the id of that layershell
    pub id: id::Id,
    /// Blur what is behind the popup (requires compositor support for org_kde_kwin_blur).
    /// It takes the blur radius, saturation, tint and border of the surface it opens from,
    /// or the global ones when that surface has none.
    pub blur: bool,
    /// Request shadow effect for this popup surface (requires compositor support for layer_shadow_manager_v1)
    pub shadow: bool,
    /// Corner radius in pixels [top_left, top_right, bottom_right, bottom_left]
    /// (requires compositor support for layer_corner_radius_manager_v1)
    pub corner_radius: Option<[u32; 4]>,
    /// Auto-size the popup to fit content after first layout.
    /// When true, the initial size is used as a maximum, and the surface will be resized
    /// to match the actual content size after the first render.
    pub auto_size: bool,
    /// Anchor rectangle width/height (defaults to size if not set).
    /// The anchor_rect position is taken from `position`.
    pub anchor_rect_size: Option<(i32, i32)>,
    /// xdg_positioner anchor edge (protocol enum value, 0 = none)
    pub anchor: u32,
    /// xdg_positioner gravity direction (protocol enum value, 0 = none)
    pub gravity: u32,
    /// xdg_positioner constraint adjustment flags
    pub constraint_adjustment: u32,
    /// Offset from the calculated anchor point
    pub offset: Option<(i32, i32)>,
    /// Whether the popup should reposition when parent moves
    pub reactive: bool,
    /// Whether to grab keyboard/pointer focus
    pub grab: bool,
    /// When true, the popup surface gets an empty input region so pointer
    /// events pass through to the surface below (e.g. for tooltips).
    pub input_passthrough: bool,
    /// When set, the compositor will position this popup at the pointer cursor
    /// plus the given (x, y) offset, eliminating client-side reposition round-trips.
    /// Requires compositor support for zcosmic_tooltip_manager_v1.
    pub tooltip_offset: Option<(i32, i32)>,
    /// Tooltip anchor corner (protocol enum value):
    /// 0 = top_left (default), 1 = top_right, 2 = bottom_left, 3 = bottom_right.
    /// Determines which corner of the tooltip aligns to the computed position.
    pub tooltip_anchor: Option<u32>,
    /// Show delay in milliseconds. When set to a non-zero value, the tooltip
    /// appears only after the pointer has hovered over the parent surface for
    /// this duration. 0 or None = immediate (follows pointer).
    pub tooltip_delay_ms: Option<u32>,
    /// Leave the popup out of screen captures and recordings while it stays on screen.
    /// Needs compositor support for kora_capture_exclusion_manager_v1.
    pub capture_excluded: bool,
}

/// Settings to reposition an existing popup via xdg_popup.reposition (v3).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RepositionPopUpSettings {
    /// The layershellev ID of the popup to reposition
    pub popup_id: id::Id,
    /// New popup size
    pub size: (u32, u32),
    /// New anchor rectangle position (x, y) relative to the parent surface
    pub position: (i32, i32),
    /// New anchor rectangle width/height
    pub anchor_rect_size: Option<(i32, i32)>,
    /// xdg_positioner anchor edge (protocol enum value, 0 = none)
    pub anchor: u32,
    /// xdg_positioner gravity direction (protocol enum value, 0 = none)
    pub gravity: u32,
    /// xdg_positioner constraint adjustment flags
    pub constraint_adjustment: u32,
    /// Offset from the calculated anchor point
    pub offset: Option<(i32, i32)>,
    /// Whether the popup should reposition when parent moves
    pub reactive: bool,
}

/// be used to create a new popup
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct NewXdgWindowSettings {
    /// Start maximized
    pub maximized: bool,
    /// the size of the popup
    pub title: Option<String>,
    pub size: Option<(u32, u32)>,
    /// xdg-shell app_id — used by compositors for the SSD titlebar icon, taskbar
    /// grouping, and `.desktop` matching. `None` leaves it unset.
    pub app_id: Option<String>,
    /// The largest size the window asks for (`xdg_toplevel.set_max_size`), in logical px.
    /// Some compositors size a new window from it: cosmic-comp shrinks one larger than
    /// two thirds of the screen unless its maximum allows it.
    pub max_size: Option<(u32, u32)>,
}

/// input panel settings to create a new input panel surface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewInputPanelSettings {
    pub size: (u32, u32),
    /// set the surface type as a keyboard
    pub keyboard: bool,
    /// follow the last output of the activated surface, used to create some thing like mako, who
    /// will show on the same window, only when the notifications is cleared, it will change the
    /// wl_output.
    pub use_last_output: bool,
}

impl Default for NewLayerShellSettings {
    fn default() -> Self {
        NewLayerShellSettings {
            anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Top,
            exclusive_zone: None,
            size: None,
            margin: Some((0, 0, 0, 0)),
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            output_option: OutputOption::None,
            events_transparent: false,
            namespace: None,
            blur: false,
            blur_radius: None,
            blur_saturation: None,
            blur_tint: None,
            blur_border: None,
            shadow: false,
            corner_radius: None,
            transition: None,
            auto_size: false,
            first_size: None,
            start_hidden: false,
            start_hidden_without_animating: false,
            follow_pointer: false,
            follow_focus: false,
            capture_excluded: false,
        }
    }
}

// Manual Eq impl: f32 doesn't derive Eq, but blur_radius is a hint value
// and doesn't affect identity semantics.
impl Eq for NewLayerShellSettings {}

/// the return data
/// Note: when event is RequestBuffer, you must return WlBuffer
/// Note: when receive InitRequest, you can request to bind extra wayland-protocols. this time you
/// can bind virtual-keyboard. you can take startcolorkeyboard as reference, or the simple.rs. Also,
/// it should can bind with text-input, but I am not fully understand about this, maybe someone
/// familiar with it can do
///
/// When send RequestExit, it will tell the event to finish.
///
/// When send RequestSetCursorShape, you can set current pointer shape. please take
/// [cursor-shape](https://wayland.app/protocols/cursor-shape-v1#wp_cursor_shape_device_v1:enum:shape) as reference.
///
/// None means nothing will happened, no request, and no return data
#[derive(Debug, PartialEq, Eq)]
pub enum ReturnData<INFO> {
    WlBuffer(WlBuffer),
    RequestBind,
    RequestExit,
    RequestCompositor,
    RedrawAllRequest,
    RedrawIndexRequest(Id),
    /// A cursor-shape name, or `"none"` to hide the pointer until a shape is requested.
    RequestSetCursorShape((String, WlPointer)),
    NewLayerShell((NewLayerShellSettings, id::Id, Option<INFO>)),
    NewPopUp((NewPopUpSettings, id::Id, Option<INFO>)),
    RepositionPopUp(RepositionPopUpSettings),
    NewXdgBase((NewXdgWindowSettings, id::Id, Option<INFO>)),
    NewInputPanel((NewInputPanelSettings, id::Id, Option<INFO>)),
    None,
}

/// this tell the what kind of information passed by [LayerShellEvent::XdgInfoChanged]
#[derive(Debug, Clone, Copy)]
pub enum XdgInfoChangedType {
    Position,
    Size,
    Name,
    Description,
}

/// The logical geometry of one output, in the compositor's global logical
/// coordinate space (xdg_output). Used by consumers that need the full monitor
/// layout — e.g. to drag a layer surface across monitors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLayoutItem {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Describes a scroll along one axis
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct AxisScroll {
    /// The scroll measured in pixels.
    pub absolute: f64,

    /// The scroll measured in steps.
    ///
    /// Note: this might always be zero if the scrolling is due to a touchpad or other continuous
    /// source.
    pub discrete: i32,

    /// The scroll was stopped.
    ///
    /// Generally this is encountered when hardware indicates the end of some continuous scrolling.
    pub stop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ime {
    /// Notifies when the IME was enabled.
    ///
    /// After getting this event you could receive [`Preedit`][Self::Preedit] and
    /// [`Commit`][Self::Commit] events. You should also start performing IME related requests
    Enabled,

    /// Notifies when a new composing text should be set at the cursor position.
    ///
    /// The value represents a pair of the preedit string and the cursor begin position and end
    /// position. When it's `None`, the cursor should be hidden. When `String` is an empty string
    /// this indicates that preedit was cleared.
    ///
    /// The cursor position is byte-wise indexed.
    Preedit(String, Option<(usize, usize)>),

    /// Notifies when text should be inserted into the editor widget.
    ///
    /// Right before this event winit will send empty [`Self::Preedit`] event.
    Commit(String),

    /// Notifies when the IME was disabled.
    ///
    /// After receiving this event you won't get any more [`Preedit`][Self::Preedit] or
    /// [`Commit`][Self::Commit] events until the next [`Enabled`][Self::Enabled] event. You should
    /// also stop issuing IME related requests and clear
    /// pending preedit text.
    Disabled,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) enum DispatchMessageInner {
    NewDisplay(WlOutput),
    MouseButton {
        state: WEnum<ButtonState>,
        serial: u32,
        button: u32,
        time: u32,
    },
    MouseLeave,
    MouseEnter {
        pointer: WlPointer,
        serial: u32,
        surface_x: f64,
        surface_y: f64,
    },
    MouseMotion {
        time: u32,
        surface_x: f64,
        surface_y: f64,
    },
    Axis {
        time: u32,
        scale: f64,
        horizontal: AxisScroll,
        vertical: AxisScroll,
        source: Option<wl_pointer::AxisSource>,
    },
    TouchDown {
        serial: u32,
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    TouchUp {
        serial: u32,
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    TouchMotion {
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    TouchCancel {
        id: i32,
        x: f64,
        y: f64,
    },

    ModifiersChanged(ModifiersState),
    Focused(Id),
    Unfocus,
    KeyboardInput {
        event: KeyEvent,

        /// If `true`, the event was generated synthetically by winit
        /// in one of the following circumstances:
        ///
        /// * Synthetic key press events are generated for all keys pressed when a window gains
        ///   focus. Likewise, synthetic key release events are generated for all keys pressed when
        ///   a window goes out of focus. ***Currently, this is only functional on X11 and
        ///   Windows***
        ///
        /// Otherwise, this value is always `false`.
        is_synthetic: bool,
    },
    PreferredScale {
        scale_u32: u32,
        scale_float: f64,
    },
    XdgInfoChanged {
        change_type: XdgInfoChangedType,
        /// The output's current logical size (logical px), carried so consumers
        /// can position per-output surfaces without re-querying the unit.
        logical_width: i32,
        logical_height: i32,
        /// The name + global logical position of the surface's output, so
        /// consumers know which monitor it's on and where (for cross-output drag).
        output_name: String,
        output_x: i32,
        output_y: i32,
    },
    /// The full logical layout of every output changed (at startup and on
    /// hotplug). Carries every monitor's name + global logical geometry.
    OutputLayoutChanged(Vec<OutputLayoutItem>),
    /// The usable (non-exclusive) area of the surface's output changed, reported
    /// by the compositor via the `layer_usable_area_v1` protocol (output logical
    /// geometry minus panels/docks), in output-logical coordinates.
    /// A workspace switch began or finished animating.
    WorkspaceTransition(crate::workspace_transition::WorkspaceTransition),
    /// The compositor announced an animated change to the surface's size.
    SizeTransition(crate::layer_size_transition::SizeTransition),
    UsableAreaChanged {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    Ime(Ime),
    /// Backdrop luminance readings for a surface's zones, as `(index, luminance)`.
    AdaptiveForeground {
        readings: Vec<(u32, f32)>,
    },
    /// The device's special key was used while this surface was the receiver
    SpecialAction(crate::special_action::SpecialActionEvent),
    /// Auto-hide visibility changed from compositor (true = visible, false = hidden)
    AutoHideVisibilityChanged(bool),
    /// Layer-surface visibility changed via hide/show protocol (true = visible, false = hidden)
    SurfaceVisibilityChanged(bool),
    /// Foreign toplevel event
    #[cfg(feature = "foreign-toplevel")]
    ForeignToplevel(ForeignToplevelEvent),
    /// Screencopy event (captured frame or failure)
    #[cfg(feature = "screencopy")]
    Screencopy(ScreencopyEvent),
    /// The desktops of every workspace changed.
    #[cfg(feature = "workspaces")]
    Workspaces(crate::ext_workspace::WorkspaceEvent),
    /// Dismiss requested - user clicked/touched outside an armed dismiss group
    DismissRequested,
    /// A drag-and-drop offer entered the surface — carries the surface-local
    /// position and the offered MIME types.
    DndEntered {
        x: f64,
        y: f64,
        mime_types: Vec<String>,
    },
    /// The drag cursor moved within the surface (surface-local position).
    DndMotion {
        x: f64,
        y: f64,
    },
    /// The drag-and-drop offer left the surface without dropping.
    DndLeft,
    /// The drag was dropped on the surface (the client reads data via the offer
    /// and calls `finish`).
    DndDrop,
    /// Data requested from the current offer arrived for a MIME type.
    DndDataReceived {
        mime_type: String,
        data: Vec<u8>,
    },
    /// The compositor selected a DnD action for the current offer.
    DndSelectedAction(u32),
    /// Source side: the drag we started was cancelled.
    DndSourceCancelled,
    /// Source side: the destination performed the drop.
    DndSourceDropPerformed,
    /// Source side: the destination finished — the source may clean up / delete.
    DndSourceFinished,
    /// Source side: the negotiated action for the drag changed.
    DndSourceAction(u32),
    /// A file was dropped onto the surface (one message per dropped file).
    FileDropped(std::path::PathBuf),
}

/// This tell the DispatchMessage by dispatch
#[derive(Debug)]
pub enum DispatchMessage {
    /// forward the event of wayland-mouse
    MouseButton {
        state: WEnum<ButtonState>,
        serial: u32,
        button: u32,
        time: u32,
    },
    /// Mouse leave the surface
    MouseLeave,
    /// forward the event of wayland-mouse
    MouseEnter {
        pointer: WlPointer,
        serial: u32,
        surface_x: f64,
        surface_y: f64,
    },
    /// forward the event of wayland-mouse
    MouseMotion {
        time: u32,
        surface_x: f64,
        surface_y: f64,
    },
    /// About the scroll
    Axis {
        time: u32,
        scale: f64,
        horizontal: AxisScroll,
        vertical: AxisScroll,
        source: Option<wl_pointer::AxisSource>,
    },
    /// forward the event of wayland-touch
    TouchDown {
        serial: u32,
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    /// forward the event of wayland-touch
    TouchUp {
        serial: u32,
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    /// forward the event of wayland-touch
    TouchMotion {
        time: u32,
        id: i32,
        x: f64,
        y: f64,
    },
    /// TouchEvent is cancelled
    TouchCancel {
        id: i32,
        x: f64,
        y: f64,
    },
    Focused(Id),
    Unfocus,
    /// Keyboard ModifiersChanged.
    ModifiersChanged(ModifiersState),
    /// Keyboard Event about input.
    KeyboardInput {
        event: KeyEvent,

        /// If `true`, the event was generated synthetically by winit
        /// in one of the following circumstances:
        ///
        /// * Synthetic key press events are generated for all keys pressed when a window gains
        ///   focus. Likewise, synthetic key release events are generated for all keys pressed when
        ///   a window goes out of focus. ***Currently, this is only functional on X11 and
        ///   Windows***
        ///
        /// Otherwise, this value is always `false`.
        is_synthetic: bool,
    },
    /// this will request to do refresh the whole screen, because the layershell tell that a new
    /// configure happened
    RequestRefresh {
        width: u32,
        height: u32,
        scale_float: f64,
        is_created: bool,
    },
    /// fractal scale handle
    PreferredScale {
        scale_u32: u32,
        scale_float: f64,
    },
    Ime(Ime),
    Closed,
    /// Backdrop luminance readings for a surface's marked zones.
    ///
    /// Each entry is `(zone index, luminance)` where the index is the order the
    /// surface added the zone to its region. Only changed zones are reported.
    AdaptiveForeground {
        readings: Vec<(u32, f32)>,
    },
    /// The device's special key was used while this surface was the receiver
    SpecialAction(crate::special_action::SpecialActionEvent),
    /// Auto-hide visibility changed from compositor
    /// visible: true = surface is fully visible, false = surface is fully hidden
    AutoHideVisibilityChanged {
        visible: bool,
    },
    /// Layer-surface visibility changed via hide/show protocol
    /// visible: true = surface visible, false = surface hidden
    SurfaceVisibilityChanged {
        visible: bool,
    },
    /// Foreign toplevel event (new window, window changed, window closed)
    #[cfg(feature = "foreign-toplevel")]
    ForeignToplevel(ForeignToplevelEvent),
    /// Screencopy event (captured frame ready or capture failed)
    #[cfg(feature = "screencopy")]
    Screencopy(ScreencopyEvent),
    /// The desktops of every workspace changed.
    #[cfg(feature = "workspaces")]
    Workspaces(crate::ext_workspace::WorkspaceEvent),
    /// Dismiss requested - user clicked/touched outside an armed dismiss group
    DismissRequested,
    /// A drag-and-drop offer entered the surface — surface-local position + the
    /// offered MIME types.
    DndEntered {
        x: f64,
        y: f64,
        mime_types: Vec<String>,
    },
    /// The drag cursor moved within the surface (surface-local position).
    DndMotion {
        x: f64,
        y: f64,
    },
    /// The drag-and-drop offer left the surface without dropping.
    DndLeft,
    /// The drag was dropped on the surface.
    DndDrop,
    /// Data requested from the current offer arrived for a MIME type.
    DndDataReceived {
        mime_type: String,
        data: Vec<u8>,
    },
    /// The compositor selected a DnD action for the current offer.
    DndSelectedAction(u32),
    /// Source side: the drag we started was cancelled.
    DndSourceCancelled,
    /// Source side: the destination performed the drop.
    DndSourceDropPerformed,
    /// Source side: the destination finished — the source may clean up / delete.
    DndSourceFinished,
    /// Source side: the negotiated action for the drag changed.
    DndSourceAction(u32),
    /// A file was dropped onto the surface (one message per dropped file).
    FileDropped(std::path::PathBuf),
    /// The xdg_output info of the output the surface is shown on changed.
    /// Carries that output's current logical size (logical px) — used to
    /// position centered/anchored layer surfaces correctly per-display,
    /// especially for `StartMode::Active` surfaces created with no output
    /// binding (whose output is only known once the compositor maps them).
    XdgInfoChanged {
        width: i32,
        height: i32,
        /// The surface's output name + global logical position (for positioning
        /// a surface across monitors).
        output_name: String,
        output_x: i32,
        output_y: i32,
    },
    /// The usable (non-exclusive) area of the surface's output changed: the
    /// output logical geometry minus every exclusive zone (panels/docks), in
    /// output-logical coordinates. Reported by the `layer_usable_area_v1`
    /// protocol; used to center overlays in the space free of panels.
    /// A workspace switch began or finished animating.
    WorkspaceTransition(crate::workspace_transition::WorkspaceTransition),
    /// The compositor announced an animated change to the surface's size.
    SizeTransition(crate::layer_size_transition::SizeTransition),
    UsableAreaChanged {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// The full logical layout of every output (startup + hotplug).
    OutputLayoutChanged(Vec<OutputLayoutItem>),
}

impl From<DispatchMessageInner> for DispatchMessage {
    fn from(val: DispatchMessageInner) -> Self {
        match val {
            DispatchMessageInner::NewDisplay(_) => unimplemented!(),
            DispatchMessageInner::MouseButton {
                state,
                serial,
                button,
                time,
            } => DispatchMessage::MouseButton {
                state,
                serial,
                button,
                time,
            },
            DispatchMessageInner::MouseLeave => DispatchMessage::MouseLeave,
            DispatchMessageInner::MouseEnter {
                pointer,
                serial,
                surface_x,
                surface_y,
            } => DispatchMessage::MouseEnter {
                pointer,
                serial,
                surface_x,
                surface_y,
            },
            DispatchMessageInner::MouseMotion {
                time,
                surface_x,
                surface_y,
            } => DispatchMessage::MouseMotion {
                time,
                surface_x,
                surface_y,
            },
            DispatchMessageInner::TouchDown {
                serial,
                time,
                id,
                x,
                y,
            } => DispatchMessage::TouchDown {
                serial,
                time,
                id,
                x,
                y,
            },
            DispatchMessageInner::TouchUp {
                serial,
                time,
                id,
                x,
                y,
            } => DispatchMessage::TouchUp {
                serial,
                time,
                id,
                x,
                y,
            },
            DispatchMessageInner::TouchMotion { time, id, x, y } => {
                DispatchMessage::TouchMotion { time, id, x, y }
            }
            DispatchMessageInner::TouchCancel { id, x, y } => {
                DispatchMessage::TouchCancel { id, x, y }
            }
            DispatchMessageInner::Axis {
                time,
                scale,
                horizontal,
                vertical,
                source,
            } => DispatchMessage::Axis {
                time,
                scale,
                horizontal,
                vertical,
                source,
            },
            DispatchMessageInner::Focused(id) => DispatchMessage::Focused(id),
            DispatchMessageInner::Unfocus => DispatchMessage::Unfocus,
            DispatchMessageInner::ModifiersChanged(modifier) => {
                DispatchMessage::ModifiersChanged(modifier)
            }
            DispatchMessageInner::KeyboardInput {
                event,
                is_synthetic,
            } => DispatchMessage::KeyboardInput {
                event,
                is_synthetic,
            },
            DispatchMessageInner::PreferredScale {
                scale_u32,
                scale_float,
            } => DispatchMessage::PreferredScale {
                scale_u32,
                scale_float,
            },
            DispatchMessageInner::Ime(ime) => DispatchMessage::Ime(ime),
            DispatchMessageInner::XdgInfoChanged {
                logical_width,
                logical_height,
                output_name,
                output_x,
                output_y,
                ..
            } => DispatchMessage::XdgInfoChanged {
                width: logical_width,
                height: logical_height,
                output_name,
                output_x,
                output_y,
            },
            DispatchMessageInner::OutputLayoutChanged(layout) => {
                DispatchMessage::OutputLayoutChanged(layout)
            }
            DispatchMessageInner::WorkspaceTransition(t) => DispatchMessage::WorkspaceTransition(t),
            DispatchMessageInner::SizeTransition(t) => DispatchMessage::SizeTransition(t),
            DispatchMessageInner::UsableAreaChanged {
                x,
                y,
                width,
                height,
            } => DispatchMessage::UsableAreaChanged {
                x,
                y,
                width,
                height,
            },
            DispatchMessageInner::AdaptiveForeground { readings } => {
                DispatchMessage::AdaptiveForeground { readings }
            }
            DispatchMessageInner::SpecialAction(event) => DispatchMessage::SpecialAction(event),
            DispatchMessageInner::AutoHideVisibilityChanged(visible) => {
                DispatchMessage::AutoHideVisibilityChanged { visible }
            }
            DispatchMessageInner::SurfaceVisibilityChanged(visible) => {
                DispatchMessage::SurfaceVisibilityChanged { visible }
            }
            #[cfg(feature = "foreign-toplevel")]
            DispatchMessageInner::ForeignToplevel(event) => DispatchMessage::ForeignToplevel(event),
            #[cfg(feature = "screencopy")]
            DispatchMessageInner::Screencopy(event) => DispatchMessage::Screencopy(event),
            #[cfg(feature = "workspaces")]
            DispatchMessageInner::Workspaces(event) => DispatchMessage::Workspaces(event),
            DispatchMessageInner::DismissRequested => DispatchMessage::DismissRequested,
            DispatchMessageInner::DndEntered { x, y, mime_types } => {
                DispatchMessage::DndEntered { x, y, mime_types }
            }
            DispatchMessageInner::DndMotion { x, y } => DispatchMessage::DndMotion { x, y },
            DispatchMessageInner::DndLeft => DispatchMessage::DndLeft,
            DispatchMessageInner::DndDrop => DispatchMessage::DndDrop,
            DispatchMessageInner::DndDataReceived { mime_type, data } => {
                DispatchMessage::DndDataReceived { mime_type, data }
            }
            DispatchMessageInner::DndSelectedAction(a) => DispatchMessage::DndSelectedAction(a),
            DispatchMessageInner::DndSourceCancelled => DispatchMessage::DndSourceCancelled,
            DispatchMessageInner::DndSourceDropPerformed => DispatchMessage::DndSourceDropPerformed,
            DispatchMessageInner::DndSourceFinished => DispatchMessage::DndSourceFinished,
            DispatchMessageInner::DndSourceAction(a) => DispatchMessage::DndSourceAction(a),
            DispatchMessageInner::FileDropped(path) => DispatchMessage::FileDropped(path),
        }
    }
}
