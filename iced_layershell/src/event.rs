use iced_core::mouse;
use iced_runtime::Action;
use layershellev::DispatchMessage;
#[cfg(feature = "foreign-toplevel")]
use layershellev::foreign_toplevel::ForeignToplevelEvent;
use layershellev::keyboard::ModifiersState;
use layershellev::reexport::wayland_client::{ButtonState, KeyState, WEnum, WlCompositor};
#[cfg(feature = "screencopy")]
pub use layershellev::screencopy::{CapturedFrame, ScreencopyEvent};
use layershellev::xkb_keyboard::KeyEvent as LayerShellKeyEvent;
#[cfg(any(feature = "foreign-toplevel", feature = "screencopy"))]
use std::sync::OnceLock;
#[cfg(any(feature = "foreign-toplevel", feature = "screencopy"))]
use std::sync::mpsc;

use iced_core::keyboard::Modifiers as IcedModifiers;

// Global channel for foreign toplevel events
#[cfg(feature = "foreign-toplevel")]
static FOREIGN_TOPLEVEL_CHANNEL: OnceLock<(
    std::sync::Mutex<mpsc::Sender<ForeignToplevelEvent>>,
    std::sync::Mutex<mpsc::Receiver<ForeignToplevelEvent>>,
)> = OnceLock::new();

#[cfg(feature = "foreign-toplevel")]
fn get_foreign_toplevel_channel() -> &'static (
    std::sync::Mutex<mpsc::Sender<ForeignToplevelEvent>>,
    std::sync::Mutex<mpsc::Receiver<ForeignToplevelEvent>>,
) {
    FOREIGN_TOPLEVEL_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send a foreign toplevel event (called by the event loop)
#[cfg(feature = "foreign-toplevel")]
pub(crate) fn send_foreign_toplevel_event(event: ForeignToplevelEvent) {
    let (tx, _) = get_foreign_toplevel_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for foreign toplevel events
///
/// Use this to receive events about toplevel windows (created, changed, closed)
/// when `foreign_toplevel: true` is set in the layer shell settings.
///
/// This function is only available when the `foreign-toplevel` feature is enabled.
///
/// # Example
/// ```no_run
/// # use iced::Subscription;
/// # use iced_layershell::reexport::ForeignToplevelEvent;
/// # enum Message { ForeignToplevel(ForeignToplevelEvent) }
/// # struct App;
/// # impl App {
/// fn subscription(&self) -> Subscription<Message> {
///     iced_layershell::event::foreign_toplevel_subscription()
///         .map(Message::ForeignToplevel)
/// }
/// # }
/// ```
#[cfg(feature = "foreign-toplevel")]
pub fn foreign_toplevel_subscription() -> iced_futures::Subscription<ForeignToplevelEvent> {
    #[derive(Hash)]
    struct ForeignToplevelSubscription;

    iced_futures::Subscription::run_with(ForeignToplevelSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<ForeignToplevelEvent>| async move {
                use iced_futures::futures::SinkExt;

                // Bridge std::sync::mpsc → async channel via a dedicated thread.
                // This replaces the old 16ms polling loop with blocking recv(),
                // giving zero CPU usage when idle and instant delivery when events arrive.
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<ForeignToplevelEvent>(100);

                std::thread::Builder::new()
                    .name("toplevel-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_foreign_toplevel_channel();
                        let rx = rx.lock().expect("foreign toplevel rx lock");
                        loop {
                            match rx.recv() {
                                Ok(event) => {
                                    if async_tx.clone().try_send(event).is_err() {
                                        log::warn!(
                                            "Foreign toplevel bridge: async channel full, dropping event"
                                        );
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    })
                    .expect("spawn toplevel bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

// Global channel for screencopy events
#[cfg(feature = "screencopy")]
static SCREENCOPY_CHANNEL: OnceLock<(
    std::sync::Mutex<mpsc::Sender<ScreencopyEvent>>,
    std::sync::Mutex<mpsc::Receiver<ScreencopyEvent>>,
)> = OnceLock::new();

#[cfg(feature = "screencopy")]
fn get_screencopy_channel() -> &'static (
    std::sync::Mutex<mpsc::Sender<ScreencopyEvent>>,
    std::sync::Mutex<mpsc::Receiver<ScreencopyEvent>>,
) {
    SCREENCOPY_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send a screencopy event (called by the event loop)
#[cfg(feature = "screencopy")]
pub(crate) fn send_screencopy_event(event: ScreencopyEvent) {
    let (tx, _) = get_screencopy_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for screencopy events
///
/// Use this to receive captured frame data from toplevel windows.
/// Requires `screencopy: true` and `foreign_toplevel: true` in the layer shell settings.
#[cfg(feature = "screencopy")]
pub fn screencopy_subscription() -> iced_futures::Subscription<ScreencopyEvent> {
    #[derive(Hash)]
    struct ScreencopySubscription;

    iced_futures::Subscription::run_with(ScreencopySubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<ScreencopyEvent>| async move {
                use iced_futures::futures::SinkExt;

                // Bridge std::sync::mpsc → async channel via a dedicated thread.
                // This replaces the old 16ms polling loop with blocking recv(),
                // giving zero-latency delivery of screencopy frames.
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<ScreencopyEvent>(100);

                std::thread::Builder::new()
                    .name("screencopy-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_screencopy_channel();
                        let rx = rx.lock().expect("screencopy rx lock");
                        loop {
                            match rx.recv() {
                                Ok(event) => {
                                    // Use try_send to avoid blocking the bridge thread.
                                    // If channel is full, drop the frame (better than stalling).
                                    if async_tx.clone().try_send(event).is_err() {
                                        log::warn!(
                                            "Screencopy bridge: async channel full, dropping frame"
                                        );
                                    }
                                }
                                Err(_) => break, // Sender disconnected
                            }
                        }
                    })
                    .expect("spawn screencopy bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

// A std::sync::mpsc channel with both ends wrapped in mutexes, stored in a
// global OnceLock so the event loop (sender) and a subscription (receiver) can
// reach it. Used by the output-info and usable-area channels below.
type SharedChannel<T> = (
    std::sync::Mutex<std::sync::mpsc::Sender<T>>,
    std::sync::Mutex<std::sync::mpsc::Receiver<T>>,
);

// Global channel for output-info events (the logical size of the output a layer
// surface is shown on). Mirrors the foreign-toplevel / screencopy channels, but
// is always available (not feature-gated).
static OUTPUT_INFO_CHANNEL: std::sync::OnceLock<SharedChannel<OutputInfoEvent>> =
    std::sync::OnceLock::new();

fn get_output_info_channel() -> &'static SharedChannel<OutputInfoEvent> {
    OUTPUT_INFO_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send an output-info event (called by the event loop).
pub(crate) fn send_output_info_event(event: OutputInfoEvent) {
    let (tx, _) = get_output_info_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for output-info events.
///
/// Yields the logical size (logical px) of the output the layer surface is
/// currently shown on, whenever the compositor reports it (at map time, and
/// again if the surface moves to another output or the output is reconfigured).
/// Use it to position/size centered or anchored surfaces relative to the actual
/// display they appear on.
///
/// # Example
/// ```no_run
/// # use iced::Subscription;
/// # use iced_layershell::event::OutputInfoEvent;
/// # enum Message { OutputInfo(OutputInfoEvent) }
/// # struct App;
/// # impl App {
/// fn subscription(&self) -> Subscription<Message> {
///     iced_layershell::event::output_info_subscription().map(Message::OutputInfo)
/// }
/// # }
/// ```
pub fn output_info_subscription() -> iced_futures::Subscription<OutputInfoEvent> {
    #[derive(Hash)]
    struct OutputInfoSubscription;

    iced_futures::Subscription::run_with(OutputInfoSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<OutputInfoEvent>| async move {
                use iced_futures::futures::SinkExt;

                // Bridge std::sync::mpsc → async channel via a dedicated thread,
                // so idle costs nothing and events deliver instantly.
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<OutputInfoEvent>(100);

                std::thread::Builder::new()
                    .name("output-info-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_output_info_channel();
                        let rx = rx.lock().expect("output info rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!(
                                    "Output info bridge: async channel full, dropping event"
                                );
                            }
                        }
                    })
                    .expect("spawn output info bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

// Global channel for output-layout events (the logical geometry of every
// output). Mirrors the output-info channel; always available.
static OUTPUT_LAYOUT_CHANNEL: std::sync::OnceLock<SharedChannel<OutputLayoutEvent>> =
    std::sync::OnceLock::new();

fn get_output_layout_channel() -> &'static SharedChannel<OutputLayoutEvent> {
    OUTPUT_LAYOUT_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send an output-layout event (called by the event loop).
pub(crate) fn send_output_layout_event(event: OutputLayoutEvent) {
    let (tx, _) = get_output_layout_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for output-layout events: the logical geometry (name + global
/// position + size) of every output, reported at startup (and on hotplug). Use
/// it to move a layer surface across monitors.
pub fn output_layout_subscription() -> iced_futures::Subscription<OutputLayoutEvent> {
    #[derive(Hash)]
    struct OutputLayoutSubscription;

    iced_futures::Subscription::run_with(OutputLayoutSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<OutputLayoutEvent>| async move {
                use iced_futures::futures::SinkExt;
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<OutputLayoutEvent>(100);
                std::thread::Builder::new()
                    .name("output-layout-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_output_layout_channel();
                        let rx = rx.lock().expect("output layout rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!("Output layout bridge: async channel full, dropping");
                            }
                        }
                    })
                    .expect("spawn output layout bridge thread");
                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

// Global channel for usable-area events (the non-exclusive area of the output a
// layer surface is shown on — output size minus panels/docks). Mirrors the
// output-info channel; always available (not feature-gated).
static USABLE_AREA_CHANNEL: std::sync::OnceLock<SharedChannel<UsableAreaEvent>> =
    std::sync::OnceLock::new();

fn get_usable_area_channel() -> &'static SharedChannel<UsableAreaEvent> {
    USABLE_AREA_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send a usable-area event (called by the event loop).
pub(crate) fn send_usable_area_event(event: UsableAreaEvent) {
    let (tx, _) = get_usable_area_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for usable-area events.
///
/// Yields the usable (non-exclusive) area of the output the layer surface is
/// shown on — the output's logical geometry minus every exclusive zone reserved
/// by panels/docks — whenever the compositor reports it (at map time, and again
/// when a panel appears/disappears/resizes/toggles auto-hide, or the surface
/// moves outputs). Use it to center content in the space free of panels rather
/// than the full output. Requires compositor support for the
/// `layer_usable_area_v1` protocol; on other compositors it never fires and
/// consumers should fall back to the full output size.
///
/// # Example
/// ```no_run
/// # use iced::Subscription;
/// # use iced_layershell::event::UsableAreaEvent;
/// # enum Message { UsableArea(UsableAreaEvent) }
/// # struct App;
/// # impl App {
/// fn subscription(&self) -> Subscription<Message> {
///     iced_layershell::event::usable_area_subscription().map(Message::UsableArea)
/// }
/// # }
/// ```
pub fn usable_area_subscription() -> iced_futures::Subscription<UsableAreaEvent> {
    #[derive(Hash)]
    struct UsableAreaSubscription;

    iced_futures::Subscription::run_with(UsableAreaSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<UsableAreaEvent>| async move {
                use iced_futures::futures::SinkExt;

                // Bridge std::sync::mpsc → async channel via a dedicated thread,
                // so idle costs nothing and events deliver instantly.
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<UsableAreaEvent>(100);

                std::thread::Builder::new()
                    .name("usable-area-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_usable_area_channel();
                        let rx = rx.lock().expect("usable area rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!(
                                    "Usable area bridge: async channel full, dropping event"
                                );
                            }
                        }
                    })
                    .expect("spawn usable area bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

// Global channel for first-frame events. Sends only once an app has asked for
// the subscription, so an app that never asks keeps nothing.
static FIRST_FRAME_CHANNEL: std::sync::OnceLock<SharedChannel<iced_core::window::Id>> =
    std::sync::OnceLock::new();
static FIRST_FRAME_WANTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn get_first_frame_channel() -> &'static SharedChannel<iced_core::window::Id> {
    FIRST_FRAME_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send a first-frame event (called by the event loop).
pub(crate) fn send_first_frame_event(id: iced_core::window::Id) {
    if !FIRST_FRAME_WANTED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let (tx, _) = get_first_frame_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(id);
    }
}

/// Subscription for first-frame events.
///
/// Yields a surface's id once its first drawn frame is committed, so the
/// compositor shows it from its next repaint. An auto-sized surface without a
/// `first_size` reports it after it has measured itself, not while it waits
/// hidden. Use it to start a motion the moment there is something to see:
/// a motion started when the surface opens can be over before its first frame
/// is drawn.
///
/// # Example
/// ```no_run
/// # use iced::Subscription;
/// # use iced::window;
/// # enum Message { FirstFrame(window::Id) }
/// # struct App;
/// # impl App {
/// fn subscription(&self) -> Subscription<Message> {
///     iced_layershell::event::first_frame_subscription().map(Message::FirstFrame)
/// }
/// # }
/// ```
pub fn first_frame_subscription() -> iced_futures::Subscription<iced_core::window::Id> {
    #[derive(Hash)]
    struct FirstFrameSubscription;

    // Set as the app asks, before it has a surface: a frame drawn before the
    // stream first runs waits in the channel rather than being dropped.
    FIRST_FRAME_WANTED.store(true, std::sync::atomic::Ordering::Relaxed);
    iced_futures::Subscription::run_with(FirstFrameSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<iced_core::window::Id>| async move {
                use iced_futures::futures::SinkExt;

                // Bridge std::sync::mpsc → async channel via a dedicated thread,
                // so idle costs nothing and events deliver instantly.
                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<iced_core::window::Id>(100);

                std::thread::Builder::new()
                    .name("first-frame-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_first_frame_channel();
                        let rx = rx.lock().expect("first frame rx lock");
                        while let Ok(id) = rx.recv() {
                            if async_tx.clone().try_send(id).is_err() {
                                log::warn!(
                                    "First frame bridge: async channel full, dropping event"
                                );
                            }
                        }
                    })
                    .expect("spawn first frame bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(id) = async_rx.next().await {
                    let _ = output.send(id).await;
                }
            },
        )
    })
}

/// A workspace switch, as reported by the compositor.
pub use layershellev::workspace_transition::WorkspaceTransition;

/// An animated change to a surface's arranged size, as reported by the compositor.
pub use layershellev::layer_size_transition::SizeTransition;

/// The desktops of every workspace, as reported by the compositor.
#[cfg(feature = "workspaces")]
pub use layershellev::ext_workspace::{
    WorkspaceEvent, WorkspaceGroupInfo, WorkspaceInfo, WorkspacesSnapshot,
};

#[cfg(feature = "workspaces")]
static WORKSPACES_CHANNEL: std::sync::OnceLock<SharedChannel<WorkspaceEvent>> =
    std::sync::OnceLock::new();

#[cfg(feature = "workspaces")]
fn get_workspaces_channel() -> &'static SharedChannel<WorkspaceEvent> {
    WORKSPACES_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

#[cfg(feature = "workspaces")]
pub(crate) fn send_workspaces_event(event: WorkspaceEvent) {
    let (tx, _) = get_workspaces_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for the desktops of every workspace.
///
/// Yields a whole snapshot after every batch of changes: the groups (one per
/// workspace and output, with the workspace registry id the compositor put on
/// it) and the desktops in them. Requires compositor support for
/// `ext_workspace_v1`; elsewhere it never fires.
#[cfg(feature = "workspaces")]
pub fn workspaces_subscription() -> iced_futures::Subscription<WorkspaceEvent> {
    #[derive(Hash)]
    struct WorkspacesSubscription;

    iced_futures::Subscription::run_with(WorkspacesSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<WorkspaceEvent>| async move {
                use iced_futures::futures::SinkExt;

                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<WorkspaceEvent>(100);

                std::thread::Builder::new()
                    .name("workspaces-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_workspaces_channel();
                        let rx = rx.lock().expect("workspaces rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!("Workspaces bridge: channel full");
                            }
                        }
                    })
                    .expect("spawn workspaces bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

/// A size transition and the window it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeTransitionEvent {
    pub window: iced_core::window::Id,
    pub transition: SizeTransition,
}

static SIZE_TRANSITION_CHANNEL: std::sync::OnceLock<SharedChannel<SizeTransitionEvent>> =
    std::sync::OnceLock::new();

fn get_size_transition_channel() -> &'static SharedChannel<SizeTransitionEvent> {
    SIZE_TRANSITION_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

pub(crate) fn send_size_transition_event(event: SizeTransitionEvent) {
    let (tx, _) = get_size_transition_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for size-transition events.
///
/// Yields `Started` when the compositor begins animating the space around one
/// of the app's surfaces, with the size it is heading for and how long that
/// takes, and `Finished` once the surface has been configured to it. Requires
/// compositor support for `layer_size_transition_manager_v1`; elsewhere it
/// never fires.
pub fn size_transition_subscription() -> iced_futures::Subscription<SizeTransitionEvent> {
    #[derive(Hash)]
    struct SizeTransitionSubscription;

    iced_futures::Subscription::run_with(SizeTransitionSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<SizeTransitionEvent>| async move {
                use iced_futures::futures::SinkExt;

                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<SizeTransitionEvent>(100);

                std::thread::Builder::new()
                    .name("size-transition-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_size_transition_channel();
                        let rx = rx.lock().expect("size transition rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!("Size transition bridge: channel full");
                            }
                        }
                    })
                    .expect("spawn size transition bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

static WORKSPACE_TRANSITION_CHANNEL: std::sync::OnceLock<SharedChannel<WorkspaceTransition>> =
    std::sync::OnceLock::new();

fn get_workspace_transition_channel() -> &'static SharedChannel<WorkspaceTransition> {
    WORKSPACE_TRANSITION_CHANNEL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
    })
}

/// Send a workspace-transition event (called by the event loop).
pub(crate) fn send_workspace_transition_event(event: WorkspaceTransition) {
    let (tx, _) = get_workspace_transition_channel();
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(event);
    }
}

/// Subscription for workspace-transition events.
///
/// Yields `Started` when a workspace switch begins animating — carrying the
/// compositor's own animation length, so a client can fade its contents over
/// the same window — and `Finished` when it lands. Requires compositor support
/// for `workspace_transition_manager_v1`; elsewhere it never fires.
///
/// # Example
/// ```no_run
/// # use iced::Subscription;
/// # use iced_layershell::event::WorkspaceTransition;
/// # enum Message { WorkspaceTransition(WorkspaceTransition) }
/// # struct App;
/// # impl App {
/// fn subscription(&self) -> Subscription<Message> {
///     iced_layershell::event::workspace_transition_subscription()
///         .map(Message::WorkspaceTransition)
/// }
/// # }
/// ```
pub fn workspace_transition_subscription() -> iced_futures::Subscription<WorkspaceTransition> {
    #[derive(Hash)]
    struct WorkspaceTransitionSubscription;

    iced_futures::Subscription::run_with(WorkspaceTransitionSubscription, |_| {
        iced_futures::stream::channel(
            100,
            |mut output: iced_futures::futures::channel::mpsc::Sender<WorkspaceTransition>| async move {
                use iced_futures::futures::SinkExt;

                let (async_tx, mut async_rx) =
                    iced_futures::futures::channel::mpsc::channel::<WorkspaceTransition>(100);

                std::thread::Builder::new()
                    .name("workspace-transition-bridge".into())
                    .spawn(move || {
                        let (_, rx) = get_workspace_transition_channel();
                        let rx = rx.lock().expect("workspace transition rx lock");
                        while let Ok(event) = rx.recv() {
                            if async_tx.clone().try_send(event).is_err() {
                                log::warn!("Workspace transition bridge: channel full");
                            }
                        }
                    })
                    .expect("spawn workspace transition bridge thread");

                use iced_futures::futures::StreamExt;
                while let Some(event) = async_rx.next().await {
                    let _ = output.send(event).await;
                }
            },
        )
    })
}

fn from_u32_to_icedmouse(code: u32) -> mouse::Button {
    match code {
        273 => mouse::Button::Right,
        274 => mouse::Button::Middle,
        _ => mouse::Button::Left,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IcedButtonState {
    Pressed(mouse::Button),
    Released(mouse::Button),
}

#[derive(Debug, Clone, Copy)]
pub enum IcedKeyState {
    Pressed,
    Released,
}

impl From<WEnum<KeyState>> for IcedKeyState {
    fn from(value: WEnum<KeyState>) -> Self {
        match value {
            WEnum::Value(KeyState::Released) => Self::Released,
            WEnum::Value(KeyState::Pressed) => Self::Pressed,
            _ => unreachable!(),
        }
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum WindowEvent {
    ScaleFactorChanged {
        scale_u32: u32,
        scale_float: f64,
    },
    CursorEnter {
        x: f64,
        y: f64,
    },
    CursorMoved {
        x: f64,
        y: f64,
    },
    CursorLeft,
    MouseInput(IcedButtonState),
    Keyboard {
        state: IcedKeyState,
        key: u32,
        modifiers: IcedModifiers,
    },
    KeyBoardInput {
        event: LayerShellKeyEvent,
        is_synthetic: bool,
    },
    Unfocus,
    Focused,
    ModifiersChanged(ModifiersState),
    Axis {
        x: f32,
        y: f32,
    },
    PixelDelta {
        x: f32,
        y: f32,
    },
    ScrollStop,
    TouchDown {
        id: i32,
        x: f64,
        y: f64,
    },
    TouchUp {
        id: i32,
        x: f64,
        y: f64,
    },
    TouchMotion {
        id: i32,
        x: f64,
        y: f64,
    },
    TouchCancel {
        id: i32,
        x: f64,
        y: f64,
    },
    Ime(layershellev::Ime),
    Refresh,
    Closed,
    ThemeChanged(iced_core::theme::Mode),
    /// Backdrop luminance readings for a surface's marked zones, as
    /// `(zone index, luminance)`. Only changed zones are reported.
    AdaptiveForeground {
        readings: Vec<(u32, f32)>,
    },
    /// The device's special key was used while this surface was the receiver
    SpecialAction(layershellev::special_action::SpecialActionEvent),

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
    /// Foreign toplevel event (window created, changed, or closed)
    #[cfg(feature = "foreign-toplevel")]
    ForeignToplevel(ForeignToplevelEvent),
    /// Screencopy event (captured frame or failure)
    #[cfg(feature = "screencopy")]
    Screencopy(ScreencopyEvent),
    /// Dismiss requested - user clicked/touched outside an armed dismiss group
    DismissRequested,
    /// A drag-and-drop offer entered the surface — surface-local position + mimes.
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
    /// A file was dropped onto the surface (one event per dropped file).
    FileDropped(std::path::PathBuf),
    /// The output the surface is shown on reported its logical size (logical px).
    /// Delivered to the app through [`output_info_subscription`] so it can position
    /// per-display layer surfaces correctly.
    OutputLogicalSize {
        width: i32,
        height: i32,
        /// Name + global logical position of the surface's output (for
        /// cross-monitor positioning).
        output_name: String,
        output_x: i32,
        output_y: i32,
    },
    /// The usable (non-exclusive) area of the output the surface is shown on
    /// changed (output logical geometry minus panels/docks). Delivered to the
    /// app through [`usable_area_subscription`].
    OutputUsableArea {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// The full logical layout of every output (startup + hotplug). Delivered to
    /// the app through [`output_layout_subscription`].
    OutputLayout(Vec<layershellev::OutputLayoutItem>),
    /// A workspace switch began or finished animating.
    WorkspaceTransition(WorkspaceTransition),
    /// The compositor announced an animated change to this surface's size.
    SizeTransition(SizeTransition),
    /// The desktops of every workspace changed.
    #[cfg(feature = "workspaces")]
    Workspaces(WorkspaceEvent),
}

/// The logical size (logical px) of the output a layer surface is shown on.
///
/// Delivered via [`output_info_subscription`]. Use it to position/size centered
/// or anchored surfaces relative to the actual display they appear on, rather
/// than a cached or primary-monitor size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputInfoEvent {
    pub width: u32,
    pub height: u32,
    /// The output's name (stable id for targeting it via `OutputOption`).
    pub name: String,
    /// The output's top-left in the compositor's global logical space.
    pub x: i32,
    pub y: i32,
    /// The surface shown on that output, when the event came from one.
    pub window: Option<iced_core::window::Id>,
}

/// The full logical layout of every output (global coords), delivered via
/// [`output_layout_subscription`]. Use it to move a surface across monitors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLayoutEvent {
    pub outputs: Vec<layershellev::OutputLayoutItem>,
}

/// The usable (non-exclusive) area of the output a layer surface is shown on,
/// in that output's logical coordinates (origin at the output's top-left). This
/// is the output's logical geometry minus every exclusive zone reserved by
/// panels/docks.
///
/// Delivered via [`usable_area_subscription`]. Center content within this
/// rectangle to sit clear of panels rather than over the full output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsableAreaEvent {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug)]
pub enum IcedLayerShellEvent<Message> {
    /// The compositor global, kept so a fresh `wl_region` can be made per
    /// request.
    ///
    /// This used to be three long-lived regions — one for input, one for blur,
    /// one for adaptive-foreground zones — shared by every surface in the
    /// application. Separating them by PURPOSE was not enough: a region is
    /// copied by the compositor when a surface commits, so one surface's
    /// rectangles were whatever the last caller had left in the shared object.
    /// A client that set a region on two surfaces got a result that depended on
    /// commit ordering, silently and with nothing logged.
    UpdateCompositor(WlCompositor),
    Window(WindowEvent),
    UserAction(Action<Message>),
    NormalDispatch,
}

impl From<&DispatchMessage> for WindowEvent {
    fn from(value: &DispatchMessage) -> Self {
        match value {
            DispatchMessage::WorkspaceTransition(t) => WindowEvent::WorkspaceTransition(t.clone()),
            DispatchMessage::SizeTransition(t) => WindowEvent::SizeTransition(*t),
            #[cfg(feature = "workspaces")]
            DispatchMessage::Workspaces(e) => WindowEvent::Workspaces(e.clone()),
            DispatchMessage::RequestRefresh { .. } => WindowEvent::Refresh,
            DispatchMessage::Closed => WindowEvent::Closed,
            DispatchMessage::MouseEnter {
                surface_x: x,
                surface_y: y,
                ..
            } => WindowEvent::CursorEnter { x: *x, y: *y },
            DispatchMessage::MouseMotion {
                surface_x: x,
                surface_y: y,
                ..
            } => WindowEvent::CursorMoved { x: *x, y: *y },
            DispatchMessage::MouseLeave => WindowEvent::CursorLeft,
            DispatchMessage::MouseButton { state, button, .. } => {
                let btn = from_u32_to_icedmouse(*button);
                match state {
                    WEnum::Value(ButtonState::Pressed) => {
                        WindowEvent::MouseInput(IcedButtonState::Pressed(btn))
                    }
                    WEnum::Value(ButtonState::Released) => {
                        WindowEvent::MouseInput(IcedButtonState::Released(btn))
                    }
                    _ => unreachable!(),
                }
            }
            DispatchMessage::TouchUp { id, x, y, .. } => WindowEvent::TouchUp {
                id: *id,
                x: *x,
                y: *y,
            },
            DispatchMessage::TouchDown { id, x, y, .. } => WindowEvent::TouchDown {
                id: *id,
                x: *x,
                y: *y,
            },
            DispatchMessage::TouchMotion { id, x, y, .. } => WindowEvent::TouchMotion {
                id: *id,
                x: *x,
                y: *y,
            },
            DispatchMessage::TouchCancel { id, x, y, .. } => WindowEvent::TouchCancel {
                id: *id,
                x: *x,
                y: *y,
            },
            DispatchMessage::PreferredScale {
                scale_u32,
                scale_float,
            } => WindowEvent::ScaleFactorChanged {
                scale_u32: *scale_u32,
                scale_float: *scale_float,
            },

            DispatchMessage::KeyboardInput {
                event,
                is_synthetic,
            } => WindowEvent::KeyBoardInput {
                event: event.clone(),
                is_synthetic: *is_synthetic,
            },
            DispatchMessage::Unfocus => WindowEvent::Unfocus,
            DispatchMessage::Focused(_) => WindowEvent::Focused,
            DispatchMessage::ModifiersChanged(modifiers) => {
                WindowEvent::ModifiersChanged(*modifiers)
            }
            DispatchMessage::Axis {
                horizontal,
                vertical,
                source,
                ..
            } => {
                use layershellev::reexport::wayland_client::wl_pointer::AxisSource;
                // Classify by the axis *source*, not by per-frame `discrete`
                // presence: a wheel is a discrete device (→ logical lines, which
                // apps animate smoothly), a touchpad is continuous (→ logical
                // pixel deltas). The compositor can drop `axis_discrete` on some
                // wheel notches (e.g. under a reduced scroll-speed setting), so
                // keying on it per-frame makes wheel scrolling flip between lines
                // and pixels and hiccup.
                let is_touchpad = matches!(
                    source,
                    Some(AxisSource::Finger) | Some(AxisSource::Continuous)
                );
                if horizontal.stop && vertical.stop {
                    WindowEvent::ScrollStop
                } else if is_touchpad {
                    // Surface (logical) pixels, as the cursor position is.
                    WindowEvent::PixelDelta {
                        x: -horizontal.absolute as f32,
                        y: -vertical.absolute as f32,
                    }
                } else {
                    // Wheel (or unknown source): logical "lines", scale-independent
                    // like winit. Derive the amount from the compositor's
                    // *continuous* magnitude rather than the discrete count, so the
                    // OS scroll-speed setting — which the compositor bakes into that
                    // value — flows through automatically. Normalize against the
                    // per-notch value at 100% speed so one default notch == 1.0 line
                    // (matching winit's discrete count and the browser-like default),
                    // and the setting scales it (e.g. 75% → 0.75 line). Using the
                    // continuous value also keeps every frame of a gesture consistent
                    // (the per-frame `discrete` is unreliable under a reduced setting).
                    const NOTCH_PIXELS: f64 = 15.0;
                    let line = |discrete: i32, absolute: f64| -> f32 {
                        if absolute != 0.0 {
                            (-absolute / NOTCH_PIXELS) as f32
                        } else if discrete != 0 {
                            // Rare: a discrete step with no continuous sample.
                            -discrete as f32
                        } else {
                            0.0
                        }
                    };
                    WindowEvent::Axis {
                        x: line(horizontal.discrete, horizontal.absolute),
                        y: line(vertical.discrete, vertical.absolute),
                    }
                }
            }
            DispatchMessage::Ime(ime) => WindowEvent::Ime(ime.clone()),
            DispatchMessage::AdaptiveForeground { readings } => WindowEvent::AdaptiveForeground {
                readings: readings.clone(),
            },
            DispatchMessage::SpecialAction(event) => WindowEvent::SpecialAction(*event),
            DispatchMessage::AutoHideVisibilityChanged { visible } => {
                WindowEvent::AutoHideVisibilityChanged { visible: *visible }
            }
            DispatchMessage::SurfaceVisibilityChanged { visible } => {
                WindowEvent::SurfaceVisibilityChanged { visible: *visible }
            }
            #[cfg(feature = "foreign-toplevel")]
            DispatchMessage::ForeignToplevel(event) => WindowEvent::ForeignToplevel(event.clone()),
            #[cfg(feature = "screencopy")]
            DispatchMessage::Screencopy(event) => WindowEvent::Screencopy(event.clone()),
            DispatchMessage::DismissRequested => WindowEvent::DismissRequested,
            DispatchMessage::DndEntered { x, y, mime_types } => WindowEvent::DndEntered {
                x: *x,
                y: *y,
                mime_types: mime_types.clone(),
            },
            DispatchMessage::DndMotion { x, y } => WindowEvent::DndMotion { x: *x, y: *y },
            DispatchMessage::DndLeft => WindowEvent::DndLeft,
            DispatchMessage::DndDrop => WindowEvent::DndDrop,
            DispatchMessage::DndDataReceived { mime_type, data } => WindowEvent::DndDataReceived {
                mime_type: mime_type.clone(),
                data: data.clone(),
            },
            DispatchMessage::DndSelectedAction(a) => WindowEvent::DndSelectedAction(*a),
            DispatchMessage::DndSourceCancelled => WindowEvent::DndSourceCancelled,
            DispatchMessage::DndSourceDropPerformed => WindowEvent::DndSourceDropPerformed,
            DispatchMessage::DndSourceFinished => WindowEvent::DndSourceFinished,
            DispatchMessage::DndSourceAction(a) => WindowEvent::DndSourceAction(*a),
            DispatchMessage::FileDropped(path) => WindowEvent::FileDropped(path.clone()),
            DispatchMessage::XdgInfoChanged {
                width,
                height,
                output_name,
                output_x,
                output_y,
            } => WindowEvent::OutputLogicalSize {
                width: *width,
                height: *height,
                output_name: output_name.clone(),
                output_x: *output_x,
                output_y: *output_y,
            },
            DispatchMessage::OutputLayoutChanged(layout) => {
                WindowEvent::OutputLayout(layout.clone())
            }
            DispatchMessage::UsableAreaChanged {
                x,
                y,
                width,
                height,
            } => WindowEvent::OutputUsableArea {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            },
        }
    }
}
