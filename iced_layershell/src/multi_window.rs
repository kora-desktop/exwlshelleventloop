use crate::{
    DefaultStyle,
    actions::{IcedNewPopupSettings, LayershellCustomActionWithId, MenuDirection},
    ime_preedit::ImeState,
    multi_window::window_manager::WindowManager,
    settings::VirtualKeyboardSettings,
    user_interface::UserInterfaces,
};
use crate::{
    actions::LayershellCustomAction, clipboard::LayerShellClipboard, conversion, error::Error,
};
use crate::{
    event::{IcedLayerShellEvent, WindowEvent as LayerShellWindowEvent},
    proxy::IcedProxy,
    settings::Settings,
};
use futures::{FutureExt, StreamExt, future::LocalBoxFuture};
#[cfg(not(all(feature = "linux-theme-detection", target_os = "linux")))]
use iced_core::theme::Mode;
use iced_core::{
    Event as IcedEvent, adaptive_foreground as iced_adaptive_foreground,
    auto_hide as iced_auto_hide, dismiss as iced_dismiss, special_action as iced_special_action,
    surface_visibility as iced_surface_visibility, theme,
    window::{Event as IcedWindowEvent, Id as IcedId, RedrawRequest},
};
use iced_core::{Size, mouse, mouse::Cursor, time::Instant};
use iced_futures::{Executor, Runtime};
use iced_graphics::{Compositor, Shell, compositor};
use iced_program::Instance;
use iced_program::Program as IcedProgram;
use iced_runtime::Action;
use iced_runtime::user_interface;
use layershellev::{
    LayerShellEvent, NewPopUpSettings, RefreshRequest, ReturnData, WindowState, WindowWrapper,
    id::Id as LayerShellId,
    reexport::{
        wayland_client::{WlCompositor, WlRegion},
        zwp_virtual_keyboard_v1,
    },
};
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    mem,
    os::fd::AsFd,
    sync::Arc,
    task::Poll,
    time::Duration,
};
use window_manager::Window;

mod event_queue;
mod state;
mod window_manager;

type MultiRuntime<E, Message> = Runtime<E, IcedProxy<Action<Message>>, Action<Message>>;

/// A popup fitted to its content as it was shown: its id, largest size and size.
type FittedPopup = (IcedId, (u32, u32), (u32, u32));

/// How long a hidden popup lives on while the compositor fades it out: longer than
/// cosmic-comp's 100 ms fade of a hidden layer popup.
const POPUP_FADE_OUT: Duration = Duration::from_millis(150);

// a dispatch loop, another is listen loop
/// Runs `program` on layer-shell surfaces. `automation` is the app's settings
/// for its automation door (used when the `automation` feature builds one in
/// and an administrator switches it on).
#[cfg_attr(
    not(all(feature = "automation", target_os = "linux")),
    allow(unused_variables)
)]
pub fn run<P>(
    program: P,
    namespace: &str,
    settings: Settings,
    compositor_settings: iced_graphics::Settings,
    automation: iced_core::automation::Config,
) -> Result<(), Error>
where
    P: IcedProgram + 'static,
    P::Theme: DefaultStyle,
    P::Message: 'static + TryInto<LayershellCustomActionWithId, Error = P::Message>,
{
    use futures::task;
    use layershellev::calloop::channel::channel;
    let (message_sender, message_receiver) = channel::<Action<P::Message>>();

    let boot_span = iced_debug::boot();
    let proxy = IcedProxy::new(message_sender);

    #[cfg(feature = "debug")]
    {
        let proxy = proxy.clone();

        iced_debug::on_hotpatch(move || {
            proxy.send_action(Action::Reload);
        });
    }

    // Kept until the loop ends, when dropping it removes the door's socket.
    #[cfg(all(feature = "automation", target_os = "linux"))]
    let _door = {
        let proxy = proxy.clone();

        iced_automation::start_with(automation, Some(namespace), move || {
            proxy.send_action(Action::Tick);
        })
    };

    let proxy_back = proxy.clone();
    let mut runtime: MultiRuntime<P::Executor, P::Message> = {
        let executor = P::Executor::new().map_err(Error::ExecutorCreationFailed)?;

        Runtime::new(executor, proxy)
    };

    let (application, task) = runtime.enter(|| Instance::new(program));

    if let Some(stream) = iced_runtime::task::into_stream(task) {
        runtime.run(stream);
    }

    runtime.track(iced_futures::subscription::into_recipes(
        runtime.enter(|| application.subscription().map(Action::Output)),
    ));

    let ev: WindowState<iced_core::window::Id> = layershellev::WindowState::new(namespace)
        .with_start_mode(settings.layer_settings.start_mode)
        .with_use_display_handle(true)
        .with_events_transparent(settings.layer_settings.events_transparent)
        .with_blur(settings.layer_settings.blur)
        .with_blur_radius(settings.layer_settings.blur_radius)
        .with_blur_saturation(settings.layer_settings.blur_saturation)
        .with_blur_tint(settings.layer_settings.blur_tint)
        .with_blur_border(settings.layer_settings.blur_border)
        .with_shadow(settings.layer_settings.shadow)
        .with_transition(settings.layer_settings.transition)
        .with_special_action(
            settings.layer_settings.special_action,
            settings.layer_settings.special_action_default,
        );

    #[cfg(feature = "foreign-toplevel")]
    let ev = ev.with_foreign_toplevel(settings.layer_settings.foreign_toplevel);

    let ev = ev
        .with_option_size(settings.layer_settings.size)
        .with_layer(settings.layer_settings.layer)
        .with_anchor(settings.layer_settings.anchor)
        .with_exclusive_zone(settings.layer_settings.exclusive_zone)
        .with_margin(settings.layer_settings.margin)
        .with_keyboard_interacivity(settings.layer_settings.keyboard_interactivity)
        .with_connection(settings.with_connection);

    // Apply corner radius if set
    let ev = if let Some(radii) = settings.layer_settings.corner_radius {
        ev.with_corner_radius(radii)
    } else {
        ev
    };

    let ev = ev.build().expect("Cannot create layershell");

    #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
    let system_theme = {
        let to_mode = |color_scheme| match color_scheme {
            mundy::ColorScheme::NoPreference => theme::Mode::None,
            mundy::ColorScheme::Light => theme::Mode::Light,
            mundy::ColorScheme::Dark => theme::Mode::Dark,
        };

        runtime.run(
            mundy::Preferences::stream(mundy::Interest::ColorScheme)
                .map(move |preferences| {
                    Action::System(iced_runtime::system::Action::NotifyTheme(to_mode(
                        preferences.color_scheme,
                    )))
                })
                .boxed(),
        );

        runtime
            .enter(|| {
                mundy::Preferences::once_blocking(
                    mundy::Interest::ColorScheme,
                    core::time::Duration::from_millis(200),
                )
            })
            .map(|preferences| to_mode(preferences.color_scheme))
            .unwrap_or_default()
    };

    #[cfg(not(all(feature = "linux-theme-detection", target_os = "linux")))]
    let system_theme = Mode::default();

    let context = Context::<
        P,
        <P as iced_program::Program>::Executor,
        <P::Renderer as iced_graphics::compositor::Default>::Compositor,
    >::new(
        application,
        compositor_settings,
        runtime,
        settings.fonts,
        system_theme,
        proxy_back,
    );
    let mut context_state = ContextState::Context(context);
    boot_span.finish();

    let mut waiting_layer_shell_events = VecDeque::new();
    let mut task_context = task::Context::from_waker(task::noop_waker_ref());

    let _ = ev.running_with_proxy(message_receiver, move |event, ev, layer_shell_id| {
        let mut def_returndata = ReturnData::None;
        match event {
            LayerShellEvent::InitRequest => {
                def_returndata = ReturnData::RequestBind;
            }
            LayerShellEvent::BindProvide(globals, qh) => {
                let wl_compositor = globals
                    .bind::<WlCompositor, _, _>(qh, 1..=1, ())
                    .expect("could not bind wl_compositor");
                waiting_layer_shell_events.push_back((
                    None,
                    IcedLayerShellEvent::UpdateCompositor(wl_compositor.clone()),
                ));

                if let Some(virtual_keyboard_setting) = settings.virtual_keyboard_support.as_ref() {
                    let virtual_keyboard_manager = globals
                        .bind::<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                            qh,
                            1..=1,
                            (),
                        )
                        .expect("no support virtual_keyboard");
                    let VirtualKeyboardSettings {
                        file,
                        keymap_size,
                        keymap_format,
                    } = virtual_keyboard_setting;
                    let seat = ev.get_seat();
                    let virtual_keyboard_in =
                        virtual_keyboard_manager.create_virtual_keyboard(seat, qh, ());
                    virtual_keyboard_in.keymap((*keymap_format).into(), file.as_fd(), *keymap_size);
                    ev.set_virtual_keyboard(virtual_keyboard_in);
                }
            }
            LayerShellEvent::RequestMessages(message) => {
                waiting_layer_shell_events.push_back((
                    layer_shell_id,
                    IcedLayerShellEvent::Window(LayerShellWindowEvent::from(message)),
                ));
            }
            LayerShellEvent::UserEvent(event) => {
                waiting_layer_shell_events
                    .push_back((layer_shell_id, IcedLayerShellEvent::UserAction(event)));
            }
            LayerShellEvent::NormalDispatch => {
                waiting_layer_shell_events
                    .push_back((layer_shell_id, IcedLayerShellEvent::NormalDispatch));
            }
            _ => {}
        }
        loop {
            let mut need_continue = false;

            // Handle the Future case first - this requires moving the state
            if matches!(context_state, ContextState::Future(_)) {
                context_state = match std::mem::replace(&mut context_state, ContextState::None) {
                    ContextState::Future(mut future) => {
                        tracing::debug!("poll context future");
                        match future.as_mut().poll(&mut task_context) {
                            Poll::Ready(context) => {
                                tracing::debug!("context future is ready");
                                need_continue = true;
                                ContextState::Context(context)
                            }
                            Poll::Pending => {
                                // Nothing wakes the loop for it; come back to poll again.
                                ev.wake_soon();
                                ContextState::Future(future)
                            }
                        }
                    }
                    other => other,
                };
            }

            // Handle the Context case in-place (no move!) to keep widget references valid
            if let ContextState::Context(ref mut context) = context_state
                && let Some((layer_shell_id, layer_shell_event)) =
                    waiting_layer_shell_events.pop_front()
            {
                need_continue = true;
                let (needs_compositor, waiting_layer_shell_event) =
                    context.handle_event(ev, layer_shell_id, layer_shell_event);
                if let Some(waiting_layer_shell_event) = waiting_layer_shell_event {
                    waiting_layer_shell_events
                        .push_front((layer_shell_id, waiting_layer_shell_event));
                }
                if let Some(window_wrapper) = needs_compositor {
                    // Compositor creation is the ONLY case that needs to move Context
                    context_state = match std::mem::replace(&mut context_state, ContextState::None)
                    {
                        ContextState::Context(context) => ContextState::Future(
                            context.create_compositor(window_wrapper).boxed_local(),
                        ),
                        other => other,
                    };
                }
            }
            if !need_continue {
                break;
            }
        }
        def_returndata
    });
    Ok(())
}

enum ContextState<Context> {
    None,
    Context(Context),
    Future(LocalBoxFuture<'static, Context>),
}

struct Context<P, E, C>
where
    P: IcedProgram + 'static,
    C: Compositor<Renderer = P::Renderer> + 'static,
    E: Executor + 'static,
    P::Theme: DefaultStyle,
    P::Message: 'static,
{
    compositor_settings: iced_graphics::Settings,
    runtime: MultiRuntime<E, P::Message>,
    system_theme: iced_core::theme::Mode,
    fonts: Vec<Cow<'static, [u8]>>,
    compositor: Option<C>,
    window_manager: WindowManager<P, C>,
    cached_layer_dimensions: HashMap<IcedId, (Size<u32>, f32)>,
    /// Windows that need auto-sizing after first render (measure phase)
    auto_size_pending: std::collections::HashSet<IcedId>,
    /// Windows hidden until auto-size resize completes - maps to expected (width, height) and frame count
    auto_size_hidden: HashMap<IcedId, (u32, u32, u8)>,
    /// Windows with continuous auto-sizing enabled (resize when content changes)
    auto_size_enabled: std::collections::HashSet<IcedId>,
    /// Track last content size for auto_size windows to detect changes
    auto_size_last_content: HashMap<IcedId, (u32, u32)>,
    /// Maximum size for auto_size windows (from original LayerShellSettings.size)
    auto_size_max: HashMap<IcedId, (u32, u32)>,
    /// Deferred shadow/blur/corner_radius for auto_size windows (applied after resize completes)
    auto_size_deferred_effects: HashMap<IcedId, (bool, bool, Option<[u32; 4]>)>,
    clipboard: LayerShellClipboard,
    /// Kept so every window can be given a `wl_region` of its own.
    wl_compositor: Option<WlCompositor>,
    /// One long-lived region per window, per purpose.
    ///
    /// They must outlive the request that sets them: `set_input_region` binds
    /// the region to the surface's pending state, and the compositor resolves
    /// it again on EVERY later commit — not just the one that applies it. A
    /// region destroyed after its first commit therefore resolves to nothing on
    /// the next frame, and the surface silently loses its input region a moment
    /// after gaining it.
    ///
    /// Per window rather than per application, because a region is copied at
    /// commit: one object shared between two surfaces gives whichever commits
    /// later the other's rectangles.
    wl_input_regions: HashMap<IcedId, WlRegion>,
    wl_blur_regions: HashMap<IcedId, WlRegion>,
    wl_adaptive_foreground_regions: HashMap<IcedId, WlRegion>,
    user_interfaces: UserInterfaces<P>,
    waiting_layer_shell_actions: Vec<(Option<IcedId>, LayershellCustomAction)>,
    iced_events: Vec<(IcedId, IcedEvent)>,
    messages: Vec<P::Message>,
    proxy: IcedProxy<Action<P::Message>>,
    /// Maps popup iced ID → layershell ID for popups that have been created
    /// (Show action processed) but not yet registered in window_manager
    /// (WindowOpened not yet received). This allows Hide to close popups
    /// even when they haven't fully initialized yet.
    pending_popups: HashMap<IcedId, layershellev::id::Id>,
    /// Popup IDs where Hide was requested before Show was processed.
    /// When Show arrives for one of these IDs, skip creation entirely.
    cancelled_popups: std::collections::HashSet<IcedId>,
    /// Popup IDs that need shadow applied after their first content frame.
    /// Shadow is deferred to avoid a visible flash of shadow around an
    /// empty/transparent surface before the popup content is drawn.
    popup_pending_shadow: std::collections::HashSet<IcedId>,
    /// Windows whose first drawn frame was committed.
    first_framed: std::collections::HashSet<IcedId>,
    /// What the automation door needs to remember between passes.
    #[cfg(all(feature = "automation", target_os = "linux"))]
    door: DoorState,
}

/// What the automation door needs to remember between passes.
#[cfg(all(feature = "automation", target_os = "linux"))]
#[derive(Debug, Default)]
struct DoorState {
    /// When each surface was first seen waiting for a frame that hasn't been
    /// drawn yet. Drawing it, or closing it, clears it.
    awaiting_frame: HashMap<IcedId, Instant>,
    /// When each surface still on its way was first seen: one the compositor
    /// has made that this loop hasn't taken in yet, or a popup asked for and
    /// not made yet. Each keeps the app busy for a while, not for ever.
    joining: HashMap<Joining, Instant>,
    /// The surface the door last placed iced's cursor on. Its frames are drawn
    /// with that cursor, so hovering shows, though the real pointer is elsewhere.
    pointer: Option<IcedId>,
    /// The surfaces hidden (`HideWindow`, or by the compositor) at the last
    /// pass, to notice one being hidden or shown again.
    hidden: std::collections::HashSet<layershellev::id::Id>,
}

/// Whether `event` only lets go of something an earlier event pressed (or
/// moves the pointer off): the only input a hidden surface still takes.
#[cfg(all(feature = "automation", target_os = "linux"))]
fn lets_go(event: &IcedEvent) -> bool {
    use iced_core::keyboard;

    matches!(
        event,
        IcedEvent::Mouse(mouse::Event::ButtonReleased(_) | mouse::Event::CursorLeft)
            | IcedEvent::Keyboard(
                keyboard::Event::KeyReleased { .. } | keyboard::Event::ModifiersChanged(_)
            )
    )
}

/// A surface on its way, for [`DoorState::joining`].
#[cfg(all(feature = "automation", target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Joining {
    Unit(layershellev::id::Id),
    Popup(IcedId),
}

impl<P, E, C> Context<P, E, C>
where
    P: IcedProgram + 'static,
    C: Compositor<Renderer = P::Renderer> + 'static,
    E: Executor + 'static,
    P::Theme: DefaultStyle,
    P::Message: 'static + TryInto<LayershellCustomActionWithId, Error = P::Message>,
{
    pub fn new(
        application: Instance<P>,
        compositor_settings: iced_graphics::Settings,
        runtime: MultiRuntime<E, P::Message>,
        fonts: Vec<Cow<'static, [u8]>>,
        system_theme: iced_core::theme::Mode,
        proxy: IcedProxy<Action<P::Message>>,
    ) -> Self {
        Self {
            compositor_settings,
            runtime,
            system_theme,
            fonts,
            compositor: Default::default(),
            window_manager: WindowManager::new(),
            cached_layer_dimensions: HashMap::new(),
            auto_size_pending: std::collections::HashSet::new(),
            auto_size_hidden: HashMap::new(),
            auto_size_enabled: std::collections::HashSet::new(),
            auto_size_last_content: HashMap::new(),
            auto_size_max: HashMap::new(),
            auto_size_deferred_effects: HashMap::new(),
            clipboard: LayerShellClipboard::unconnected(),
            wl_compositor: Default::default(),
            wl_input_regions: Default::default(),
            wl_blur_regions: Default::default(),
            wl_adaptive_foreground_regions: Default::default(),
            user_interfaces: UserInterfaces::new(application),
            waiting_layer_shell_actions: Default::default(),
            iced_events: Default::default(),
            messages: Default::default(),
            proxy,
            pending_popups: HashMap::new(),
            cancelled_popups: std::collections::HashSet::new(),
            popup_pending_shadow: std::collections::HashSet::new(),
            first_framed: std::collections::HashSet::new(),
            #[cfg(all(feature = "automation", target_os = "linux"))]
            door: DoorState::default(),
        }
    }

    async fn create_compositor(mut self, window: Arc<WindowWrapper>) -> Self {
        let shell = Shell::new(self.proxy.clone());
        let mut new_compositor = C::new(
            self.compositor_settings,
            window.clone(),
            window.clone(),
            shell,
        )
        .await
        .expect("Cannot create compositer");
        for font in self.fonts.clone() {
            new_compositor.load_font(font);
        }
        self.compositor = Some(new_compositor);
        self.clipboard = LayerShellClipboard::connect(&window);
        self
    }

    fn remove_compositor(&mut self) {
        self.compositor = None;
        self.clipboard = LayerShellClipboard::unconnected();
    }

    /// Handle a layer shell event. Returns:
    /// - `Option<Arc<WindowWrapper>>`: Some if compositor creation is needed (caller must invoke create_compositor)
    /// - `Option<IcedLayerShellEvent>`: event to re-queue if it needs to be retried
    fn handle_event(
        &mut self,
        ev: &mut WindowState<IcedId>,
        layer_shell_id: Option<LayerShellId>,
        layer_shell_event: IcedLayerShellEvent<P::Message>,
    ) -> (
        Option<Arc<WindowWrapper>>,
        Option<IcedLayerShellEvent<P::Message>>,
    ) {
        tracing::trace!(
            "Handle layer shell event, layer_shell_id: {:?},  waiting actions: {}, messages: {}",
            layer_shell_id,
            self.waiting_layer_shell_actions.len(),
            self.messages.len(),
        );
        if let IcedLayerShellEvent::Window(LayerShellWindowEvent::Refresh) = layer_shell_event
            && self.compositor.is_none()
        {
            let Some(layer_shell_window) = layer_shell_id.and_then(|lid| ev.get_unit_with_id(lid))
            else {
                tracing::error!("layer shell window not found: {:?}", layer_shell_id);
                return (None, None);
            };
            tracing::debug!("creating compositor");
            // Signal to caller that compositor creation is needed
            return (
                Some(Arc::new(layer_shell_window.gen_wrapper())),
                Some(layer_shell_event),
            );
        }

        match layer_shell_event {
            IcedLayerShellEvent::UpdateCompositor(compositor) => {
                self.wl_compositor = Some(compositor)
            }
            IcedLayerShellEvent::Window(LayerShellWindowEvent::Refresh) => {
                self.handle_refresh_event(ev, layer_shell_id)
            }
            IcedLayerShellEvent::Window(LayerShellWindowEvent::Closed) => {
                self.handle_closed_event(ev, layer_shell_id)
            }
            IcedLayerShellEvent::Window(window_event) => {
                // Some events need to trigger a refresh to process the subscription message
                let needs_refresh = self.handle_window_event(layer_shell_id, window_event);
                if needs_refresh {
                    // Process any iced_events that were added immediately
                    // This ensures the UI updates without waiting for the next NormalDispatch
                    if !self.iced_events.is_empty() {
                        tracing::debug!(
                            "handle_event: processing {} iced_events immediately after window event",
                            self.iced_events.len()
                        );
                        self.handle_normal_dispatch(ev);
                    }
                    ev.request_refresh_all(RefreshRequest::NextFrame);
                }
            }
            IcedLayerShellEvent::UserAction(user_action) => {
                self.handle_user_action(ev, user_action);
                // Immediately dispatch any messages added by subscription output
                // (e.g., keyboard events) to avoid waiting for the next timer-based
                // NormalDispatch (~50ms delay). (The automation door's wake-up
                // is answered by the dispatch the channel's ping runs next.)
                if !self.messages.is_empty() {
                    self.handle_normal_dispatch(ev);
                }
            }
            IcedLayerShellEvent::NormalDispatch => self.handle_normal_dispatch(ev),
        }

        // at each interaction try to resolve those waiting actions.
        let mut waiting_layer_shell_actions = Vec::new();
        mem::swap(
            &mut self.waiting_layer_shell_actions,
            &mut waiting_layer_shell_actions,
        );
        for (iced_id, action) in waiting_layer_shell_actions {
            self.handle_layer_shell_action(ev, iced_id, action);
        }

        (None, None)
    }

    fn handle_refresh_event(
        &mut self,
        ev: &mut WindowState<IcedId>,
        layer_shell_id: Option<LayerShellId>,
    ) {
        let Some(layer_shell_window) = layer_shell_id.and_then(|lid| ev.get_unit_with_id(lid))
        else {
            return;
        };

        #[cfg(all(feature = "automation", target_os = "linux"))]
        if let Some((iced_id, _)) =
            layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid))
        {
            let _ = self.door.awaiting_frame.remove(&iced_id);
        }

        let (width, height) = layer_shell_window.get_size();
        let scale_float = layer_shell_window.scale_float();
        // events may not be handled after RequestRefreshWithWrapper in the same
        // interaction, we dispatched them immediately.
        let mut events = Vec::new();
        let (iced_id, window) = if let Some((iced_id, window)) =
            self.window_manager.get_mut_alias(layer_shell_window.id())
        {
            let window_size = window.state.window_size();

            if window_size.width != width
                || window_size.height != height
                || window.state.wayland_scale_factor() != scale_float
            {
                let layout_span = iced_debug::layout(iced_id);
                window.state.update_view_port(width, height, scale_float);
                let logical_size = window.state.viewport().logical_size();
                // A stale window is rebuilt at the new size, which lays it out
                // already — relayout only what was live and still sized to the old
                // viewport.
                if self.user_interfaces.is_stale(&iced_id) {
                    self.user_interfaces
                        .ui_mut(&iced_id, &mut window.renderer, logical_size);
                } else if let Some(ui) =
                    self.user_interfaces
                        .ui_mut(&iced_id, &mut window.renderer, logical_size)
                {
                    ui.relayout(logical_size, &mut window.renderer);
                }
                layout_span.finish();
                events.push(IcedEvent::Window(IcedWindowEvent::Resized(
                    window.state.window_size_f32(),
                )));
            }
            (iced_id, window)
        } else {
            let wrapper = Arc::new(layer_shell_window.gen_wrapper());
            let iced_id = layer_shell_window
                .get_binding()
                .copied()
                .unwrap_or_else(IcedId::unique);

            let window = self.window_manager.insert(
                iced_id,
                (width, height),
                scale_float,
                wrapper,
                self.user_interfaces.application(),
                self.compositor
                    .as_mut()
                    .expect("It should have been created"),
                self.system_theme,
            );

            tracing::debug!(
                "New window registered: iced_id={:?}, size=({}, {}), scale_float={}, viewport_physical={:?}, viewport_logical={:?}, viewport_scale_factor={}",
                iced_id,
                width,
                height,
                scale_float,
                window.state.viewport().physical_size(),
                window.state.viewport().logical_size(),
                window.state.viewport().scale_factor()
            );

            // Popup is now in window_manager; remove from pending tracking
            self.pending_popups.remove(&iced_id);

            self.user_interfaces.build(
                iced_id,
                user_interface::Cache::default(),
                &mut window.renderer,
                window.state.viewport().logical_size(),
            );

            events.push(IcedEvent::Window(IcedWindowEvent::Opened {
                position: None,
                size: window.state.window_size_f32(),
            }));

            // Emit Rescaled event so apps can read the compositor scale via
            // window::scale_factor(). PreferredScale typically arrives before
            // the window is registered, so handle_window_event never fires
            // for the initial scale. Emit it here instead.
            if scale_float != 1.0 {
                events.push(IcedEvent::Window(IcedWindowEvent::Rescaled(
                    scale_float as f32,
                )));
            }

            (iced_id, window)
        };

        let compositor = self
            .compositor
            .as_mut()
            .expect("The compositor should have been created");

        let logical_size = window.state.viewport().logical_size();
        let mut ui = self
            .user_interfaces
            .ui_mut(&iced_id, &mut window.renderer, logical_size)
            .expect("Get User interface");

        let cursor = if ev.is_mouse_surface(layer_shell_window.id()) {
            window.state.cursor()
        } else {
            Cursor::Unavailable
        };

        #[cfg(all(feature = "automation", target_os = "linux"))]
        let cursor = if self.door.pointer == Some(iced_id) {
            window.state.cursor()
        } else {
            cursor
        };

        events.push(IcedEvent::Window(IcedWindowEvent::RedrawRequested(
            Instant::now(),
        )));

        let draw_span = iced_debug::draw(iced_id);
        let (ui_state, statuses) =
            ui.update(&events, cursor, &mut window.renderer, &mut self.messages);

        let physical_size = window.state.viewport().physical_size();

        if self
            .cached_layer_dimensions
            .get(&iced_id)
            .is_none_or(|(size, scale)| {
                *size != physical_size || *scale != window.state.viewport().scale_factor()
            })
        {
            self.cached_layer_dimensions.insert(
                iced_id,
                (physical_size, window.state.viewport().scale_factor()),
            );

            compositor.configure_surface(
                &mut window.surface,
                physical_size.width,
                physical_size.height,
            );
        }

        for (idx, event) in events.into_iter().enumerate() {
            let status = statuses
                .get(idx)
                .cloned()
                .unwrap_or(iced_core::event::Status::Ignored);
            self.runtime
                .broadcast(iced_futures::subscription::Event::Interaction {
                    window: iced_id,
                    event,
                    status,
                });
        }

        // For auto_size windows, measure the true content size BEFORE drawing and,
        // if it changed in EITHER direction (grow OR shrink), resize + relayout to
        // it atomically — set_size + viewport + swapchain reconfigure + relayout, all
        // before the draw — so the buffer committed THIS frame already matches the
        // new size. A bottom-anchored surface then keeps its bottom edge pinned in a
        // single frame: content never jumps-then-settles when expanding nor
        // lifts-then-settles when collapsing.
        //
        // Previously only the GROW direction was atomic (gated on the content
        // filling the window); SHRINK fell through to the post-draw dynamic block,
        // which called set_size AFTER the draw/present. That left the committed
        // buffer one frame TALLER than the just-configured geometry, so the compositor
        // (which positions a bottom-anchored surface at output_bottom - configured_h)
        // placed the oversized buffer too high and its bottom edge lifted off the
        // anchor each collapse frame. Handling both directions here removes that.
        //
        // `auto_size_handled_predraw` switches the post-draw dynamic block off when
        // this ran, so the two can't both issue a set_size in the same frame (a
        // second, post-draw set_size from a re-measure would re-introduce the
        // buffer/geometry mismatch this is meant to eliminate).
        let mut auto_size_handled_predraw = false;
        // Set wherever the interface is relaid out below, so the draw can put
        // back what a relayout drops (see the empty update before `ui.draw`).
        let mut relaid_out = false;
        let dynamic_resize_target: Option<(u32, u32)> = if self.auto_size_enabled.contains(&iced_id)
            && !self.auto_size_pending.contains(&iced_id)
        {
            auto_size_handled_predraw = true;
            relaid_out = true;
            let window_size = window.state.window_size();

            // Get max bounds
            let (max_width, max_height) = self
                .auto_size_max
                .get(&iced_id)
                .copied()
                .unwrap_or((10000, 10000));

            // Never let an auto_size surface grow taller than the display it's on:
            // clamp the height bound to the output's logical height (reported
            // per-surface via xdg_output). Without this, content taller than the
            // screen would have the client request — and draw — a buffer taller than
            // the output, which the compositor then clips and/or positions partly
            // off-screen. The app's own max (`auto_size_max`) still applies; this only
            // tightens it to the physical display.
            //
            // NB: this is the raw output height — it does NOT subtract other layers'
            // exclusive zones (e.g. a panel), so a bottom-anchored surface above a
            // panel could still want the slightly-smaller usable area. Clamping to the
            // display is the safe screen-overflow guard; usable-area precision is a
            // follow-up (the usable-area protocol is already plumbed for it).
            let max_height = layer_shell_window
                .get_xdgoutput_info()
                .map(|o| o.get_logical_size().1)
                .filter(|h| *h > 0)
                .map_or(max_height, |display_h| max_height.min(display_h as u32));

            // Measure true content at max bounds. The widget tree reports its own
            // intrinsic height (and, for an animated reveal, its currently-animating
            // height) regardless of the bound, so this tracks a height animation
            // frame by frame in both directions.
            let max_bounds = Size::new(max_width as f32, max_height as f32);
            ui = ui.relayout(max_bounds, &mut window.renderer);
            let true_content_size = ui.content_size();
            let measured_width = (true_content_size.width.ceil() as u32).min(max_width);
            let measured_height = (true_content_size.height.ceil() as u32).min(max_height);

            // Resize whenever the measured size differs from the last one we set, in
            // either dimension and either direction (grow or shrink).
            let last_size = self.auto_size_last_content.get(&iced_id).copied();
            let needs_resize = last_size
                .map(|(last_w, last_h)| measured_width != last_w || measured_height != last_h)
                .unwrap_or(true);

            if needs_resize && measured_height > 0 && measured_width > 0 {
                tracing::debug!(
                    "Auto-size atomic resize for {:?}: ({}, {}), max=({}, {})",
                    iced_id,
                    measured_width,
                    measured_height,
                    max_width,
                    max_height
                );
                layer_shell_window.set_size((measured_width, measured_height));
                window.state.update_view_port(
                    measured_width,
                    measured_height,
                    layer_shell_window.scale_float(),
                );
                ui = ui.relayout(window.state.viewport().logical_size(), &mut window.renderer);

                // update_view_port just changed the viewport's physical size, but the
                // per-frame wgpu reconfigure above already ran with the OLD size and
                // won't run again this frame. Reconfigure the swapchain NOW so the
                // buffer we draw+present below is at the new physical size. Without
                // this, present() renders into the stale surface and wp_viewport
                // stretches it to the new destination — the new content would arrive
                // distorted for a frame, defeating the atomic resize.
                let new_physical = window.state.viewport().physical_size();
                self.cached_layer_dimensions.insert(
                    iced_id,
                    (new_physical, window.state.viewport().scale_factor()),
                );
                compositor.configure_surface(
                    &mut window.surface,
                    new_physical.width,
                    new_physical.height,
                );

                self.auto_size_last_content
                    .insert(iced_id, (measured_width, measured_height));
            } else {
                // No size change: relayout back to the current size for the draw.
                let current_bounds = Size::new(window_size.width as f32, window_size.height as f32);
                ui = ui.relayout(current_bounds, &mut window.renderer);
            }
            // The resize (if any) is applied above; nothing is deferred to post-draw.
            None
        } else {
            None
        };

        // Check if this window needs initial auto-sizing (first render measurement)
        // Do this BEFORE draw so we can request resize before presenting
        let initial_auto_size_target: Option<(u32, u32)> =
            if self.auto_size_pending.remove(&iced_id) {
                relaid_out = true;
                let window_size = window.state.window_size_f32();

                // Get max bounds from stored settings (original size acts as maximum)
                let (max_width, max_height) = self
                    .auto_size_max
                    .get(&iced_id)
                    .copied()
                    .unwrap_or((10000, 10000));

                // Do layout at max size to measure true content size
                let max_bounds = Size::new(max_width as f32, max_height as f32);
                ui = ui.relayout(max_bounds, &mut window.renderer);
                let content_size = ui.content_size();

                tracing::debug!(
                    "Auto-size INITIAL check for {:?}: content={:?}, max=({}, {})",
                    iced_id,
                    content_size,
                    max_width,
                    max_height
                );

                // Resize to measured content size (clamped to max bounds)
                let new_width = (content_size.width.ceil() as u32).min(max_width);
                let new_height = (content_size.height.ceil() as u32).min(max_height);

                // Skip if size would be zero (invalid for layer shell)
                if new_width == 0 || new_height == 0 {
                    tracing::trace!(
                        "Auto-size: skipping zero size ({}, {})",
                        new_width,
                        new_height
                    );
                    // Relayout back to window size for draw
                    let current_bounds = Size::new(window_size.width, window_size.height);
                    ui = ui.relayout(current_bounds, &mut window.renderer);
                    None
                } else {
                    tracing::debug!(
                        "Auto-sizing window {:?} from ({}, {}) to ({}, {})",
                        iced_id,
                        window_size.width as u32,
                        window_size.height as u32,
                        new_width,
                        new_height
                    );

                    // Apply size change BEFORE draw
                    layer_shell_window.set_size((new_width, new_height));

                    // Relayout to the new target size for proper draw
                    let target_bounds = Size::new(new_width as f32, new_height as f32);
                    ui = ui.relayout(target_bounds, &mut window.renderer);

                    // Store initial content size for change detection
                    self.auto_size_last_content
                        .insert(iced_id, (new_width, new_height));

                    Some((new_width, new_height))
                }
            } else {
                None
            };

        // If we just requested initial auto-size, skip present until resize is confirmed
        // The issue is that window.state still has old size, so draw will use wrong viewport
        let skip_present = if let Some((target_w, target_h)) = initial_auto_size_target {
            let current_size = window.state.window_size();
            tracing::debug!(
                "Auto-size: requested resize to ({}, {}), current window state is ({}, {})",
                target_w,
                target_h,
                current_size.width,
                current_size.height
            );
            // Window state hasn't updated yet - wait for configure event
            self.auto_size_hidden
                .insert(iced_id, (target_w, target_h, 0));
            true
        } else if let Some((target_w, target_h, frame_count)) =
            self.auto_size_hidden.get_mut(&iced_id)
        {
            let current_size = window.state.window_size();
            // A compositor may give less than was asked for; its answer is final either way.
            if (current_size.width == *target_w && current_size.height == *target_h)
                || layer_shell_window.size_answered()
            {
                // Resize confirmed, remove from hidden
                tracing::debug!(
                    "Auto-size resize CONFIRMED for {:?}: size ({}, {})",
                    iced_id,
                    target_w,
                    target_h
                );
                self.auto_size_hidden.remove(&iced_id);
                // Re-apply deferred shadow/blur/corner_radius now that surface has its final size
                if let Some((shadow, blur, corner_radius)) =
                    self.auto_size_deferred_effects.remove(&iced_id)
                {
                    if shadow {
                        self.waiting_layer_shell_actions
                            .push((Some(iced_id), LayershellCustomAction::ShadowChange(true)));
                    }
                    if blur {
                        self.waiting_layer_shell_actions
                            .push((Some(iced_id), LayershellCustomAction::BlurChange(true)));
                    }
                    if corner_radius.is_some() {
                        self.waiting_layer_shell_actions.push((
                            Some(iced_id),
                            LayershellCustomAction::CornerRadiusChange(corner_radius),
                        ));
                    }
                }
                false
            } else {
                *frame_count += 1;
                tracing::info!(
                    "Auto-size waiting for resize {:?}: frame {}, current {:?}, target ({}, {})",
                    iced_id,
                    frame_count,
                    current_size,
                    target_w,
                    target_h
                );
                if *frame_count > 5 {
                    // Timeout - give up waiting
                    tracing::warn!(
                        "Auto-size TIMEOUT for {:?}: showing at size {:?}, wanted ({}, {})",
                        iced_id,
                        current_size,
                        target_w,
                        target_h
                    );
                    self.auto_size_hidden.remove(&iced_id);
                    // Re-apply deferred shadow/blur/corner_radius even on timeout
                    if let Some((shadow, blur, corner_radius)) =
                        self.auto_size_deferred_effects.remove(&iced_id)
                    {
                        if shadow {
                            self.waiting_layer_shell_actions
                                .push((Some(iced_id), LayershellCustomAction::ShadowChange(true)));
                        }
                        if blur {
                            self.waiting_layer_shell_actions
                                .push((Some(iced_id), LayershellCustomAction::BlurChange(true)));
                        }
                        if corner_radius.is_some() {
                            self.waiting_layer_shell_actions.push((
                                Some(iced_id),
                                LayershellCustomAction::CornerRadiusChange(corner_radius),
                            ));
                        }
                    }
                    false
                } else {
                    true
                }
            }
        } else {
            false
        };

        // Skip drawing widget content when waiting for auto-size resize.
        if !skip_present {
            // `relayout` rebuilds the interface without the overlay layout that
            // `update` derives, and `draw` paints an overlay only from that
            // layout. So on an auto-sized surface a menu, dropdown or tooltip
            // was built, sized and taking clicks — and never drawn. An update
            // with no events derives the layout again without doing anything
            // else.
            if relaid_out {
                let _ = ui.update(&[], cursor, &mut window.renderer, &mut self.messages);
            }
            ui.draw(
                &mut window.renderer,
                window.state.theme(),
                &iced_core::renderer::Style {
                    text_color: window.state.text_color(),
                },
                cursor,
            );
        }

        // Check for dynamic content size changes on auto_size windows.
        // Skipped when the pre-draw block already handled the resize atomically
        // this frame (`auto_size_handled_predraw`), so the two never both issue a
        // set_size — a post-draw re-measure could differ from the pre-draw one and
        // re-introduce the buffer/geometry mismatch. This block is now only a
        // fallback for frames where the pre-draw path didn't run.
        if self.auto_size_enabled.contains(&iced_id)
            && initial_auto_size_target.is_none()
            && !auto_size_handled_predraw
        {
            // Check for content size changes on auto_size windows (dynamic resize)
            // Use pre-measured target if available (for content that fills window)
            // Otherwise use current content size (for content shrinking)
            let window_size = window.state.window_size();
            let content_size = ui.content_size();
            let (new_width, new_height) = if let Some((w, h)) = dynamic_resize_target {
                (w, h)
            } else {
                (
                    content_size.width.ceil() as u32,
                    content_size.height.ceil() as u32,
                )
            };

            let last_size = self.auto_size_last_content.get(&iced_id).copied();

            tracing::trace!(
                "Auto-size dynamic check for {:?}: content=({}, {}), window=({}, {}), last={:?}",
                iced_id,
                new_width,
                new_height,
                window_size.width,
                window_size.height,
                last_size
            );

            let needs_resize = last_size
                .map(|(last_w, last_h)| last_w != new_width || last_h != new_height)
                .unwrap_or(true);

            if needs_resize {
                // Skip if size would be zero (invalid for layer shell)
                if new_width == 0 || new_height == 0 {
                    tracing::trace!(
                        "Auto-size dynamic: skipping zero size ({}, {})",
                        new_width,
                        new_height
                    );
                } else {
                    tracing::debug!(
                        "Auto-size content changed for {:?}: new size ({}, {})",
                        iced_id,
                        new_width,
                        new_height
                    );

                    // Update tracked size
                    self.auto_size_last_content
                        .insert(iced_id, (new_width, new_height));

                    // Apply size change
                    layer_shell_window.set_size((new_width, new_height));
                }
            }
        }

        draw_span.finish();

        // get layer_shell_id so that layer_shell_window can be drop, and ev can be borrow mut
        let layer_shell_id = layer_shell_window.id();

        Self::handle_ui_state(
            ev,
            window,
            ui_state,
            false,
            true,
            &mut self.clipboard,
            &mut self.iced_events,
            Some(&mut *compositor),
        );

        window.draw_preedit();

        // When waiting for auto-size resize, present a transparent buffer
        // so the Wayland protocol flow continues (compositor needs a commit
        // to process the resize request). Shadow, blur, and corner_radius are
        // deferred so the 1x1 transparent surface is truly invisible.
        let background_color = if skip_present {
            iced_core::Color::TRANSPARENT
        } else {
            window.state.background_color()
        };
        let present_span = iced_debug::present(iced_id);
        match compositor.present(
            &mut window.renderer,
            &mut window.surface,
            window.state.viewport(),
            background_color,
            || {
                ev.request_next_present(layer_shell_id);
            },
        ) {
            Ok(()) => {
                present_span.finish();

                // Apply deferred shadow for popups after their first
                // content frame to avoid shadow-around-empty-surface flash.
                if self.popup_pending_shadow.remove(&iced_id) {
                    self.waiting_layer_shell_actions
                        .push((Some(iced_id), LayershellCustomAction::ShadowChange(true)));
                }
                if !skip_present && self.first_framed.insert(iced_id) {
                    crate::event::send_first_frame_event(iced_id);
                }
            }
            Err(error) => match error {
                compositor::SurfaceError::OutOfMemory => {
                    panic!("{error:?}");
                }
                _ => {
                    tracing::error!("Error {error:?} when presenting surface.");
                }
            },
        }
    }

    fn handle_closed_event(
        &mut self,
        ev: &mut WindowState<IcedId>,
        layer_shell_id: Option<LayerShellId>,
    ) {
        // Resolve the iced id from the layershell id. Prefer the window_manager
        // alias (registered for every window on its first refresh) so this also
        // works for auto-created `AllScreens` panels, which carry no unit
        // binding; fall back to the unit binding for windows that were closed
        // before their first refresh.
        let Some(iced_id) = layer_shell_id
            .and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id))
            .or_else(|| {
                layer_shell_id
                    .and_then(|lid| ev.get_unit_with_id(lid))
                    .and_then(|layer_shell_window| layer_shell_window.get_binding().copied())
            })
        else {
            return;
        };
        self.cached_layer_dimensions.remove(&iced_id);

        // Events still queued for it would never be handed out, and the
        // automation door would wait on them for ever.
        self.iced_events.retain(|(id, _)| *id != iced_id);

        #[cfg(all(feature = "automation", target_os = "linux"))]
        {
            let _ = self.door.awaiting_frame.remove(&iced_id);
            self.door.pointer = self.door.pointer.filter(|id| *id != iced_id);
        }

        self.auto_size_pending.remove(&iced_id);
        self.auto_size_hidden.remove(&iced_id);
        self.auto_size_enabled.remove(&iced_id);
        self.auto_size_last_content.remove(&iced_id);
        self.auto_size_max.remove(&iced_id);
        self.auto_size_deferred_effects.remove(&iced_id);
        self.first_framed.remove(&iced_id);
        self.window_manager.remove(iced_id);
        self.user_interfaces.remove(&iced_id);
        self.runtime
            .broadcast(iced_futures::subscription::Event::Interaction {
                window: iced_id,
                event: IcedEvent::Window(IcedWindowEvent::Closed),
                status: iced_core::event::Status::Ignored,
            });
        // if now there is no windows now, then break the compositor, and unlink the clipboard.
        // A surface made as this one closed is not drawn yet; it keeps the compositor, which
        // takes a moment to build again.
        let on_its_way = ev
            .get_unit_iter()
            .any(|unit| Some(unit.id()) != layer_shell_id);
        if self.window_manager.is_empty() && !on_its_way {
            self.remove_compositor();
        }
    }

    /// Handle window events. Returns true if a refresh should be requested
    /// (for events that go through subscription channels).
    fn handle_window_event(
        &mut self,
        layer_shell_id: Option<LayerShellId>,
        event: LayerShellWindowEvent,
    ) -> bool {
        // Handle foreign toplevel events specially - they go through a subscription channel
        #[cfg(feature = "foreign-toplevel")]
        if let LayerShellWindowEvent::ForeignToplevel(ref toplevel_event) = event {
            crate::event::send_foreign_toplevel_event(toplevel_event.clone());
            return true; // Request refresh to process subscription message
        }

        // Handle screencopy events - they go through a subscription channel
        #[cfg(feature = "screencopy")]
        if let LayerShellWindowEvent::Screencopy(ref screencopy_event) = event {
            crate::event::send_screencopy_event(screencopy_event.clone());
            return true;
        }

        #[cfg(feature = "workspaces")]
        if let LayerShellWindowEvent::Workspaces(ref workspaces_event) = event {
            crate::event::send_workspaces_event(workspaces_event.clone());
            return true;
        }

        // Handle output-info events (the logical size of the output the surface is
        // shown on) - they go through a subscription channel.
        if let LayerShellWindowEvent::OutputLogicalSize {
            width,
            height,
            output_name,
            output_x,
            output_y,
        } = event
        {
            let window =
                layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id));
            crate::event::send_output_info_event(crate::event::OutputInfoEvent {
                width: width.max(0) as u32,
                height: height.max(0) as u32,
                name: output_name,
                x: output_x,
                y: output_y,
                window,
            });
            return true;
        }

        if let LayerShellWindowEvent::WorkspaceTransition(transition) = event {
            crate::event::send_workspace_transition_event(transition);
            return true;
        }

        // Size transitions are per surface, so they carry the window they are for.
        if let LayerShellWindowEvent::SizeTransition(transition) = event {
            let window =
                layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id));
            if let Some(window) = window {
                crate::event::send_size_transition_event(crate::event::SizeTransitionEvent {
                    window,
                    transition,
                });
            }
            return true;
        }

        // The full output layout (every monitor's name + global geometry).
        if let LayerShellWindowEvent::OutputLayout(outputs) = event {
            crate::event::send_output_layout_event(crate::event::OutputLayoutEvent { outputs });
            return true;
        }

        // Handle usable-area events (the non-exclusive area of the surface's
        // output) - they go through a separate subscription channel.
        if let LayerShellWindowEvent::OutputUsableArea {
            x,
            y,
            width,
            height,
        } = event
        {
            crate::event::send_usable_area_event(crate::event::UsableAreaEvent {
                x,
                y,
                width,
                height,
            });
            return true;
        }

        // Handle dismiss events - convert to iced event for immediate delivery
        // The device's special key, resolved by the compositor. Routed to the
        // first surface: the compositor already decided who should receive it,
        // so there is nothing left to route here.
        if let LayerShellWindowEvent::SpecialAction(action) = event {
            use layershellev::special_action::SpecialActionEvent;
            let iced_event = IcedEvent::SpecialAction(match action {
                SpecialActionEvent::Activate => iced_special_action::Event::Activate,
                SpecialActionEvent::HoldStart => iced_special_action::Event::HoldStart,
                SpecialActionEvent::HoldEnd => iced_special_action::Event::HoldEnd,
                SpecialActionEvent::Cancel => iced_special_action::Event::Cancel,
            });
            if let Some((iced_id, _)) = self.window_manager.iter_mut().next() {
                self.iced_events.push((iced_id, iced_event));
            }
            return true;
        }

        if let LayerShellWindowEvent::DismissRequested = event {
            tracing::debug!("handle_window_event: received DismissRequested event");
            let iced_event = IcedEvent::Dismiss(iced_dismiss::Event::Requested);
            if let Some((iced_id, _)) = self.window_manager.iter_mut().next() {
                self.iced_events.push((iced_id, iced_event));
            }
            return true;
        }

        // Backdrop luminance readings - convert to an iced event for the surface
        // that marked the zones.
        if let LayerShellWindowEvent::AdaptiveForeground { readings } = event {
            tracing::debug!(
                "handle_window_event: received AdaptiveForeground: {} reading(s)",
                readings.len()
            );
            let iced_event =
                IcedEvent::AdaptiveForeground(iced_adaptive_foreground::Event::Updated {
                    readings: readings.clone(),
                });
            // Resolved from the layershell id, as auto-hide is below: in
            // `AllScreens` mode each monitor has its own surface with its own
            // zones, and routing every reading to the first window would restyle
            // one monitor from another's wallpaper.
            let mut target =
                layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id));
            if target.is_none() {
                target = self.window_manager.iter_mut().next().map(|(id, _)| id);
            }
            if let Some(iced_id) = target {
                self.iced_events.push((iced_id, iced_event));
            }
            return true;
        }

        // Handle auto-hide visibility events - convert to iced event for immediate delivery
        if let LayerShellWindowEvent::AutoHideVisibilityChanged { visible } = event {
            tracing::debug!(
                "handle_window_event: received AutoHideVisibilityChanged: visible={}, layer_shell_id={:?}",
                visible,
                layer_shell_id
            );
            let iced_event = IcedEvent::AutoHide(if visible {
                iced_auto_hide::Event::Shown
            } else {
                iced_auto_hide::Event::Hidden
            });
            // Route the event to the surface that actually toggled, resolved from
            // its layershell id. In `AllScreens` mode each monitor has its own
            // panel surface; without this every surface received the first
            // window's id and only one panel would react. Fall back to the first
            // window when the id is missing (older single-surface path).
            let mut target =
                layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id));
            if target.is_none() {
                target = self.window_manager.iter_mut().next().map(|(id, _)| id);
            }
            if let Some(iced_id) = target {
                self.iced_events.push((iced_id, iced_event));
            }
            return true;
        }

        // Handle layer-surface-visibility events - convert to iced event
        if let LayerShellWindowEvent::SurfaceVisibilityChanged { visible } = event {
            tracing::debug!(
                "handle_window_event: received SurfaceVisibilityChanged: visible={}, layer_shell_id={:?}",
                visible,
                layer_shell_id
            );
            let iced_event = IcedEvent::SurfaceVisibility(if visible {
                iced_surface_visibility::Event::Shown
            } else {
                iced_surface_visibility::Event::Hidden
            });
            // Route to the surface that actually changed, resolved from its
            // layershell id, exactly as auto-hide is above. A process can hide
            // its surfaces independently, and delivering every change to the
            // first window tells a surface it is hidden while it is on screen —
            // which any widget that skips work while away then acts on.
            let mut target =
                layer_shell_id.and_then(|lid| self.window_manager.get_alias(lid).map(|(id, _)| id));
            if target.is_none() {
                target = self.window_manager.iter_mut().next().map(|(id, _)| id);
            }
            if let Some(iced_id) = target {
                self.iced_events.push((iced_id, iced_event));
            }
            return true;
        }

        let id_and_window = if let Some(layer_shell_id) = layer_shell_id {
            self.window_manager.get_mut_alias(layer_shell_id)
        } else {
            self.window_manager.iter_mut().next()
        };
        let Some((iced_id, window)) = id_and_window else {
            return false;
        };
        // In previous implementation, event without layer_shell_id won't call `update` here, but
        // will broadcast to the application. I'm not sure why, but I think it is
        // reasonable to call `update` here.
        window
            .state
            .update(&event, self.user_interfaces.application());

        // Reset cached mouse_interaction on cursor leave/enter so the next
        // UI update will re-send the cursor shape to the compositor.
        // Per the Wayland protocol, after every wl_pointer::enter the cursor
        // shape is undefined and must be re-set by the client.  Without this
        // reset the cached value may match the UI value, causing the shape
        // request to be skipped (e.g. when a new layer-shell surface is
        // created and the compositor re-enters the panel surface).
        if matches!(
            event,
            LayerShellWindowEvent::CursorLeft | LayerShellWindowEvent::CursorEnter { .. }
        ) {
            tracing::trace!(
                "Cursor {:?} — resetting cached mouse_interaction for window {:?}",
                if matches!(event, LayerShellWindowEvent::CursorLeft) {
                    "left"
                } else {
                    "enter"
                },
                iced_id,
            );
            window.mouse_interaction = mouse::Interaction::Idle;
        }

        if let Some(event) = conversion::window_event(
            &event,
            window.state.application_scale_factor(),
            window.state.modifiers(),
        ) {
            self.iced_events.push((iced_id, event));
        }
        false
    }

    /// Answers the automation door's requests from this loop's surfaces and
    /// pending events. Runs before the pending events are handed out, so
    /// anything injected is processed in the same pass, exactly as if the
    /// compositor had sent it.
    #[cfg(all(feature = "automation", target_os = "linux"))]
    fn serve_automation(&mut self, ev: &mut WindowState<IcedId>) {
        use iced_automation::{Answer, Ask, Collector, Surface};

        if !iced_automation::is_open() {
            return;
        }

        let now = Instant::now();

        // Surfaces hidden with `HideWindow` (or by the compositor) live on in
        // `window_manager`, but nothing of them is drawn and a person can't
        // reach them.
        let hidden: std::collections::HashSet<layershellev::id::Id> = ev
            .get_unit_iter()
            .map(|unit| unit.id())
            .filter(|id| ev.is_hidden(*id))
            .collect();

        for id in hidden.difference(&self.door.hidden) {
            // Hidden: a person's pointer would have left it, so the door's
            // does too, and a hover it showed isn't drawn with it when it
            // comes back.
            if let Some((iced_id, window)) = self.window_manager.get_mut_alias(*id) {
                window.state.clear_cursor();
                self.door.pointer = self.door.pointer.filter(|pointed| *pointed != iced_id);
            }
        }

        for id in self.door.hidden.difference(&hidden) {
            // Shown again: a frame asked for while hidden was never drawn, so
            // the wait for one starts now, not when it was asked for.
            if let Some((iced_id, _)) = self.window_manager.get_alias(*id) {
                let _ = self.door.awaiting_frame.remove(&iced_id);
            }
        }

        self.door.hidden.clone_from(&hidden);
        let is_hidden = |id: layershellev::id::Id| hidden.contains(&id);

        let awaiting_frame = &mut self.door.awaiting_frame;
        let mut idle = self.iced_events.is_empty()
            && self.messages.is_empty()
            && self.window_manager.iter_mut().all(|(iced_id, window)| {
                let refresh = ev
                    .get_unit_with_id(window.id)
                    .map_or(RefreshRequest::Wait, |unit| unit.refresh_request());

                // A timed redraw that has come due is waiting for a frame like
                // any other, so it stops counting once the surface isn't drawn.
                let (requested_at, redraw_at) = match refresh {
                    RefreshRequest::NextFrame => {
                        (Some(*awaiting_frame.entry(iced_id).or_insert(now)), None)
                    }
                    RefreshRequest::At(at) if at <= now => {
                        (Some(*awaiting_frame.entry(iced_id).or_insert(at)), None)
                    }
                    RefreshRequest::At(at) => (None, Some(at)),
                    RefreshRequest::Wait => (None, None),
                };

                !iced_automation::frame_due(requested_at, redraw_at, now)
            });

        // A click that opens a surface returns before the surface joins this
        // loop: asked for (waiting for layershellev to make it), made but not
        // yet refreshed into `window_manager`, or a popup not made yet, it is
        // coming, so the app isn't idle. Timed like a frame, so one that never
        // comes can't keep the app busy for ever.
        let on_the_way: Vec<Joining> = ev
            .requested_surfaces()
            .chain(
                ev.get_unit_iter()
                    .map(|unit| unit.id())
                    .filter(|id| self.window_manager.get_alias(*id).is_none()),
            )
            .map(Joining::Unit)
            .chain(self.pending_popups.keys().map(|id| Joining::Popup(*id)))
            .collect();

        self.door
            .joining
            .retain(|surface, _| on_the_way.contains(surface));

        for surface in on_the_way {
            let since = *self.door.joining.entry(surface).or_insert(now);

            idle = idle && !iced_automation::frame_due(Some(since), None, now);
        }

        // The surface the compositor last gave input to. (The door itself sends
        // keys to the surface it last clicked, while this stays the same.)
        let focused = ev
            .current_surface_id()
            .and_then(|lid| self.window_manager.get_alias(lid))
            .map(|(iced_id, _)| iced_id);

        let popups: std::collections::HashSet<layershellev::id::Id> = ev
            .get_unit_iter()
            .filter(|unit| unit.is_popup())
            .map(|unit| unit.id())
            .collect();
        let is_popup = |id: layershellev::id::Id| popups.contains(&id);

        // The door doesn't list, read or press hidden surfaces either, nor
        // send keys to one.
        let focused = focused.filter(|iced_id| {
            self.window_manager
                .get(*iced_id)
                .is_some_and(|window| !is_hidden(window.id))
        });
        let first_shown = self
            .window_manager
            .iter_mut()
            .find(|(_, window)| !is_hidden(window.id))
            .map(|(iced_id, _)| iced_id);

        let window_manager = &mut self.window_manager;
        let user_interfaces = &mut self.user_interfaces;
        let iced_events = &mut self.iced_events;
        let door = &mut self.door;

        iced_automation::serve(|ask| match ask {
            Ask::Idle => Answer::Idle(idle),
            Ask::Info => Answer::Info(
                window_manager
                    .iter_mut()
                    .filter(|(_, window)| !is_hidden(window.id))
                    .map(|(iced_id, window)| {
                        Surface::new(
                            iced_id,
                            is_popup(window.id),
                            window.state.viewport().logical_size(),
                            window.state.viewport().scale_factor(),
                            focused == Some(iced_id),
                        )
                    })
                    .collect(),
            ),
            Ask::Focused => Answer::Focused(focused.or(first_shown)),
            Ask::Tree => {
                let mut nodes = Vec::new();

                for (iced_id, window) in window_manager.iter_mut() {
                    if is_hidden(window.id) {
                        continue;
                    }

                    let logical_size = window.state.viewport().logical_size();

                    if let Some(mut ui) =
                        user_interfaces.ui_mut(&iced_id, &mut window.renderer, logical_size)
                    {
                        let mut collector =
                            Collector::new(iced_id, is_popup(window.id), logical_size);

                        ui.operate(&window.renderer, &mut collector);
                        nodes.extend(collector.into_nodes());
                    }
                }

                Answer::Tree(nodes)
            }
            Ask::Inject {
                window,
                cursor,
                events,
                leave,
                ..
            } => {
                // The door's pointer leaves the surface it was on: forget the
                // cursor placed there, and tell it, as a person's pointer would.
                if let Some(left) = leave
                    && let Some(surface) = window_manager.get_mut(left)
                {
                    surface.state.clear_cursor();
                    door.pointer = door.pointer.filter(|id| *id != left);
                    iced_events.push((left, IcedEvent::Mouse(mouse::Event::CursorLeft)));
                    idle = false;
                }

                let Some(surface) = window_manager.get_mut(window) else {
                    return Answer::Injected(false);
                };

                // Nothing new goes into a hidden surface. What lets go of a
                // press does: the key or click that hid it (Enter in a
                // launcher's search, say) is still let go of, as a person's
                // would be, so nothing stays held down when it comes back.
                if is_hidden(surface.id) {
                    if !events.iter().all(lets_go) {
                        return Answer::Injected(false);
                    }
                } else if let Some(cursor) = cursor {
                    surface.state.place_cursor(cursor);
                    door.pointer = Some(window);
                }

                iced_events.extend(events.into_iter().map(|event| (window, event)));
                idle = false;

                Answer::Injected(true)
            }
            _ => Answer::Unsupported,
        });
    }

    fn handle_user_action(&mut self, ev: &mut WindowState<IcedId>, action: Action<P::Message>) {
        let mut should_exit = false;
        let mut fitted_popups = Vec::new();
        run_action(
            &mut self.user_interfaces,
            &mut self.compositor,
            action,
            &mut self.messages,
            &mut self.clipboard,
            &mut self.waiting_layer_shell_actions,
            &mut should_exit,
            &mut self.window_manager,
            &mut self.system_theme,
            &mut self.runtime,
            ev,
            &mut self.iced_events,
            &mut self.pending_popups,
            &mut self.cancelled_popups,
            &mut self.popup_pending_shadow,
            &mut fitted_popups,
        );
        // A popup fitted to its content is refitted as that changes, like an
        // auto-sized layer surface, without the hidden first frame: it was measured.
        for (id, max, size) in fitted_popups {
            self.auto_size_enabled.insert(id);
            self.auto_size_max.insert(id, max);
            self.auto_size_last_content.insert(id, size);
        }
        if should_exit {
            ev.append_return_data(ReturnData::RequestExit);
        }
    }

    /// This window's region for `purpose`, cleared and ready to be filled.
    ///
    /// Created once per window and kept for its lifetime — see the note on
    /// `wl_input_regions` for why it must not be destroyed after use. Cleared
    /// by subtracting a rectangle far larger than any surface, since the only
    /// way to empty a `wl_region` is to subtract from it, and subtracting just
    /// the current window's bounds would leave behind anything set while it was
    /// larger.
    fn region_for<'a>(
        regions: &'a mut HashMap<IcedId, WlRegion>,
        compositor: &WlCompositor,
        qh: &wayland_client::QueueHandle<layershellev::WindowState<IcedId>>,
        iced_id: IcedId,
    ) -> &'a WlRegion {
        let region = regions
            .entry(iced_id)
            .or_insert_with(|| compositor.create_region(qh, ()));
        region.subtract(i32::MIN / 2, i32::MIN / 2, i32::MAX, i32::MAX);
        region
    }

    fn handle_layer_shell_action(
        &mut self,
        ev: &mut WindowState<IcedId>,
        mut iced_id: Option<IcedId>,
        action: LayershellCustomAction,
    ) {
        let layer_shell_window;
        macro_rules! ref_layer_shell_window {
            ($ev: ident, $iced_id: ident, $layer_shell_id: ident, $layer_shell_window: ident) => {
                if $iced_id.is_none() {
                    // Make application also works
                    if let Some(window) = self.window_manager.first() {
                        $iced_id = Some(window.iced_id);
                        $layer_shell_id = Some(window.id);
                    }
                    if $iced_id.is_none() {
                        tracing::error!(
                            "Here should be an id, it is a bug, please report an issue for us"
                        );
                        return;
                    }
                }
                if let Some(ls_window) =
                    $layer_shell_id.and_then(|layer_shell_id| $ev.get_unit_with_id(layer_shell_id))
                {
                    layer_shell_window = ls_window;
                } else {
                    return;
                }
            };
        }
        // check if window is ready
        let mut layer_shell_id = iced_id
            .and_then(|iced_id| self.window_manager.get(iced_id))
            .map(|window| window.id);
        if iced_id.is_some() && layer_shell_id.is_none() {
            // still waiting
            self.waiting_layer_shell_actions.push((iced_id, action));
            return;
        }
        match action {
            LayershellCustomAction::AnchorChange(anchor) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_anchor(anchor);
            }
            LayershellCustomAction::AnchorSizeChange(anchor, size) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_anchor_with_size(anchor, size);
            }
            LayershellCustomAction::LayerChange(layer) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_layer(layer);
            }
            LayershellCustomAction::MarginChange(margin) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_margin(margin);
            }
            LayershellCustomAction::SizeChange((width, height)) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_size((width, height));
                // Note: For auto-size windows, unhiding happens when we receive the resize event
                // with the correct size (in handle_refresh_event)
            }
            LayershellCustomAction::CornerRadiusChange(radii) => {
                // Extract surface in a block to drop the borrow before calling mutable method
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_corner_radius_for_surface(&surface, radii);
            }
            LayershellCustomAction::RequestFocus => {
                // Same borrow dance as above: drop the window borrow first.
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.request_focus_for_surface(&surface);
            }
            LayershellCustomAction::SetVerticalPlacement {
                fraction,
                offset,
                min_margin,
            } => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_vertical_placement_for_surface(&surface, fraction, offset, min_margin);
            }
            LayershellCustomAction::UnsetVerticalPlacement => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.unset_vertical_placement_for_surface(&surface);
            }
            LayershellCustomAction::SetMaxHeight {
                fraction,
                min_height,
            } => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_max_height_for_surface(&surface, fraction, min_height);
            }
            LayershellCustomAction::UnsetMaxHeight => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.unset_max_height_for_surface(&surface);
            }
            LayershellCustomAction::SetEdgeResize {
                min_width,
                max_width,
            } => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_edge_resize_for_surface(&surface, min_width, max_width);
            }
            LayershellCustomAction::UnsetEdgeResize => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.unset_edge_resize_for_surface(&surface);
            }
            LayershellCustomAction::TransitionChange(transition) => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_surface_transition(&surface, transition);
            }
            LayershellCustomAction::BlurChange(enabled) => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_blur_for_surface(&surface, enabled);
            }
            LayershellCustomAction::SetSurfaceBlur {
                radius,
                saturation,
                tint,
                border,
            } => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_blur_for_surface_with_params(&surface, radius, saturation, tint, border);
            }
            LayershellCustomAction::SetBlurRegion {
                callback,
                radii,
                geometry,
                opacity,
            } => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let set_region = callback.0;
                let Some(compositor) = &self.wl_compositor else {
                    tracing::warn!(
                        "wl_compositor is not bound, ignore SetBlurRegion, window_id: {:?}",
                        iced_id
                    );
                    return;
                };

                let Some(id) = iced_id else {
                    return;
                };
                let compositor = compositor.clone();
                let region = Self::region_for(
                    &mut self.wl_blur_regions,
                    &compositor,
                    layer_shell_window.qh(),
                    id,
                );

                let surface = layer_shell_window.get_wlsurface().clone();
                ev.set_blur_region_for_surface(
                    &surface,
                    region,
                    &radii,
                    &geometry,
                    &opacity,
                    |r| set_region(r),
                );
                tracing::debug!(?iced_id, "SetBlurRegion: done");
            }
            LayershellCustomAction::SetAdaptiveForegroundRegion { callback } => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let add_zones = callback.0;
                let Some(compositor) = &self.wl_compositor else {
                    tracing::warn!(
                        "wl_compositor is not bound, ignore SetAdaptiveForegroundRegion, window_id: {:?}",
                        iced_id
                    );
                    return;
                };

                let Some(id) = iced_id else {
                    return;
                };
                let compositor = compositor.clone();
                let region = Self::region_for(
                    &mut self.wl_adaptive_foreground_regions,
                    &compositor,
                    layer_shell_window.qh(),
                    id,
                );

                let surface = layer_shell_window.get_wlsurface().clone();
                ev.set_adaptive_foreground_region_for_surface(&surface, region, |r| add_zones(r));
                tracing::debug!(?iced_id, "SetAdaptiveForegroundRegion: done");
            }
            LayershellCustomAction::ShadowChange(enabled) => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_shadow_for_surface(&surface, enabled);
            }
            LayershellCustomAction::KeyboardShortcutsInhibitChange(enabled) => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_keyboard_shortcuts_inhibit_for_surface(&surface, enabled);
            }
            LayershellCustomAction::CaptureExcludedChange(excluded) => {
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_capture_excluded_for_surface(&surface, excluded);
            }
            LayershellCustomAction::AutoHideChange {
                edge,
                edge_zone,
                mode,
            } => {
                // Extract surface in a block to drop the borrow before calling mutable method
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.set_auto_hide_for_surface(&surface, edge, edge_zone, mode);
            }
            LayershellCustomAction::AutoHideUnset => {
                // Extract surface in a block to drop the borrow before calling mutable method
                let surface = {
                    ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                    layer_shell_window.get_wlsurface().clone()
                };
                ev.unset_auto_hide_for_surface(&surface);
            }
            LayershellCustomAction::ExclusiveZoneChange(zone_size) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_exclusive_zone(zone_size);
            }
            LayershellCustomAction::ExclusiveEdgeChange(edge) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_exclusive_edge(edge);
            }
            LayershellCustomAction::KeyboardInteractivityChange(interactivity) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_keyboard_interactivity(interactivity);
            }
            LayershellCustomAction::SetInputRegion(set_region) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let set_region = set_region.0;
                let Some(compositor) = &self.wl_compositor else {
                    tracing::warn!(
                        "wl_compositor is not bound, ignore SetInputRegion, window_id: {:?}",
                        iced_id
                    );
                    return;
                };

                let Some(id) = iced_id else {
                    return;
                };
                let compositor = compositor.clone();
                let region = Self::region_for(
                    &mut self.wl_input_regions,
                    &compositor,
                    layer_shell_window.qh(),
                    id,
                );
                set_region(region);

                let surface = layer_shell_window.get_wlsurface();
                surface.set_input_region(Some(region));
                // Applied with the next frame, in one commit with the drawing.
                let window_id = layer_shell_window.id();
                ev.request_refresh(window_id, RefreshRequest::NextFrame);
            }
            LayershellCustomAction::VirtualKeyboardPressed { time, key } => {
                use layershellev::reexport::wayland_client::KeyState;
                let ky = ev.get_virtual_keyboard().unwrap();
                ky.key(time, key, KeyState::Pressed.into());
                ev.set_virtual_key_release(layershellev::VirtualKeyRelease {
                    delay: Duration::from_micros(100),
                    time,
                    key,
                });
            }
            LayershellCustomAction::NewLayerShell {
                mut settings,
                id: iced_id,
                ..
            } => {
                // Track this window for auto-sizing if enabled
                if settings.auto_size
                    && let Some(size) = settings.first_size
                {
                    // Already measured by the app: map at it, and measure from the first frame.
                    self.auto_size_enabled.insert(iced_id);
                    self.auto_size_max
                        .insert(iced_id, settings.size.unwrap_or((10000, 10000)));
                    self.auto_size_last_content.insert(iced_id, size);
                    settings.size = Some(size);
                } else if settings.auto_size {
                    // Store the original size as max bounds for auto-sizing
                    let max_size = settings.size.unwrap_or((10000, 10000));
                    tracing::debug!(
                        "Auto-size enabled for window {:?}, max_size={:?}, forcing initial size to 1x1",
                        iced_id,
                        max_size
                    );
                    self.auto_size_pending.insert(iced_id);
                    self.auto_size_enabled.insert(iced_id);
                    self.auto_size_max.insert(iced_id, max_size);
                    // Force initial size to 1x1 to avoid visual flash before auto-size completes
                    // The surface will be resized to content size on first render
                    settings.size = Some((1, 1));
                    let deferred_shadow = settings.shadow;
                    let deferred_blur = settings.blur;
                    let deferred_corner_radius = settings.corner_radius;
                    settings.shadow = false;
                    settings.blur = false;
                    settings.corner_radius = None;
                    if deferred_shadow || deferred_blur || deferred_corner_radius.is_some() {
                        self.auto_size_deferred_effects.insert(
                            iced_id,
                            (deferred_shadow, deferred_blur, deferred_corner_radius),
                        );
                    }
                }
                let layer_shell_id = layershellev::id::Id::unique();
                ev.append_return_data(ReturnData::NewLayerShell((
                    settings,
                    layer_shell_id,
                    Some(iced_id),
                )));
            }
            LayershellCustomAction::NewBaseWindow {
                settings,
                id: iced_id,
                ..
            } => {
                let layer_shell_id = layershellev::id::Id::unique();
                ev.append_return_data(ReturnData::NewXdgBase((
                    settings.into(),
                    layer_shell_id,
                    Some(iced_id),
                )));
            }
            LayershellCustomAction::RemoveWindow => {
                if let Some(layer_shell_id) = layer_shell_id {
                    ev.request_close(layer_shell_id)
                }
            }
            LayershellCustomAction::NewPopUp {
                settings: menusettings,
                id: iced_id,
            } => {
                let IcedNewPopupSettings {
                    size,
                    position,
                    shadow,
                    corner_radius,
                    auto_size,
                } = menusettings;
                let Some(parent_layer_shell_id) = ev.current_surface_id() else {
                    return;
                };
                let popup_settings = NewPopUpSettings {
                    size,
                    position,
                    id: parent_layer_shell_id,
                    blur: false,
                    shadow,
                    corner_radius,
                    auto_size,
                    anchor_rect_size: None,
                    anchor: 0,
                    gravity: 0,
                    constraint_adjustment: 0,
                    offset: None,
                    reactive: false,
                    grab: false,
                    input_passthrough: false,
                    tooltip_offset: None,
                    tooltip_anchor: None,
                    tooltip_delay_ms: None,
                    capture_excluded: false,
                };
                let layer_shell_id = layershellev::id::Id::unique();
                ev.append_return_data(ReturnData::NewPopUp((
                    popup_settings,
                    layer_shell_id,
                    Some(iced_id),
                )));
            }
            LayershellCustomAction::NewMenu {
                settings: menu_setting,
                id: iced_id,
            } => {
                let Some(parent_layer_shell_id) = ev.current_surface_id() else {
                    return;
                };
                let Some((_, window)) = self.window_manager.get_alias(parent_layer_shell_id) else {
                    return;
                };

                let Some(point) = window.state.mouse_position() else {
                    return;
                };

                let (x, mut y) = (point.x as i32, point.y as i32);
                if let MenuDirection::Up = menu_setting.direction {
                    y -= menu_setting.size.1 as i32;
                }
                let popup_settings = NewPopUpSettings {
                    size: menu_setting.size,
                    position: (x, y),
                    id: parent_layer_shell_id,
                    blur: false,
                    shadow: false,
                    corner_radius: None,
                    auto_size: false,
                    anchor_rect_size: None,
                    anchor: 0,
                    gravity: 0,
                    constraint_adjustment: 0,
                    offset: None,
                    reactive: false,
                    grab: false,
                    input_passthrough: false,
                    tooltip_offset: None,
                    tooltip_anchor: None,
                    tooltip_delay_ms: None,
                    capture_excluded: false,
                };
                let layer_shell_id = layershellev::id::Id::unique();
                ev.append_return_data(ReturnData::NewPopUp((
                    popup_settings,
                    layer_shell_id,
                    Some(iced_id),
                )))
            }
            LayershellCustomAction::NewInputPanel {
                settings,
                id: iced_id,
            } => {
                let layer_shell_id = layershellev::id::Id::unique();
                ev.append_return_data(ReturnData::NewInputPanel((
                    settings,
                    layer_shell_id,
                    Some(iced_id),
                )));
            }
            LayershellCustomAction::ForgetLastOutput => {
                ev.forget_last_output();
            }
            LayershellCustomAction::DragIconChange(icon) => {
                ev.update_drag_icon(layershellev::DndIconPixels {
                    width: icon.width,
                    height: icon.height,
                    scale: icon.scale,
                    argb: icon.pixels,
                });
            }
            LayershellCustomAction::HideWindow => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                tracing::info!(
                    ?iced_id,
                    "HideWindow: calling hide_surface (visibility protocol)"
                );
                ev.hide_surface(&surface);
            }
            LayershellCustomAction::SetFullscreen(fullscreen) => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                layer_shell_window.set_fullscreen(fullscreen);
            }
            LayershellCustomAction::ShowWindow => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                tracing::info!(
                    ?iced_id,
                    "ShowWindow: calling show_surface (visibility protocol)"
                );
                ev.show_surface(&surface);
            }
            LayershellCustomAction::AssignRealm { workspace } => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                if !ev.assign_realm(&surface, &workspace) {
                    tracing::warn!(
                        ?iced_id,
                        workspace,
                        "AssignRealm: not offered by the compositor; the surface stays machine-plane"
                    );
                }
            }
            #[cfg(feature = "foreign-toplevel")]
            LayershellCustomAction::ToplevelAction(action) => {
                log::info!("Processing ToplevelAction: {:?}", action);
                let result = ev.execute_toplevel_action(action);
                log::info!("ToplevelAction result: {}", result);
            }
            #[cfg(not(feature = "foreign-toplevel"))]
            LayershellCustomAction::ToplevelAction(action) => match action {},
            #[cfg(feature = "screencopy")]
            LayershellCustomAction::ScreencopyAction(action) => {
                log::debug!("Processing ScreencopyAction: {:?}", action);
                ev.execute_screencopy_action_internal(action);
            }
            #[cfg(not(feature = "screencopy"))]
            LayershellCustomAction::ScreencopyAction(action) => match action {},
            #[cfg(feature = "workspaces")]
            LayershellCustomAction::WorkspaceAction(action) => {
                log::debug!("Processing WorkspaceAction: {:?}", action);
                ev.execute_workspace_action(action);
            }
            #[cfg(not(feature = "workspaces"))]
            LayershellCustomAction::WorkspaceAction(action) => match action {},
            LayershellCustomAction::ArmDismiss => {
                // Resolve the surface with the single-window fallback (id == None →
                // the first/only window), like every other action does. Without this
                // a single-window `application` (which sends id == None) could never
                // arm dismiss.
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                ev.arm_dismiss(&surface);
            }
            LayershellCustomAction::DisarmDismiss => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                ev.disarm_dismiss(&surface);
            }
            LayershellCustomAction::SetDismissIgnoreLayerClicks => {
                ref_layer_shell_window!(ev, iced_id, layer_shell_id, layer_shell_window);
                let surface = layer_shell_window.get_wlsurface().clone();
                ev.set_dismiss_ignore_layer_clicks(&surface);
            }
            LayershellCustomAction::AddMainSurfaceToDismissGroup => {
                // Get the popup surface
                let popup_surface = layer_shell_id.and_then(|id| {
                    ev.get_unit_with_id(id)
                        .map(|unit| unit.get_wlsurface().clone())
                });
                // Get the main (first) surface
                let main_surface = ev.main_window().get_wlsurface().clone();
                if let Some(popup_surface) = popup_surface {
                    ev.add_to_dismiss_group(&popup_surface, &main_surface);
                }
            }
            LayershellCustomAction::AddAllSurfacesToDismissGroup => {
                // Get the popup surface
                let popup_surface = layer_shell_id.and_then(|id| {
                    ev.get_unit_with_id(id)
                        .map(|unit| unit.get_wlsurface().clone())
                });
                if let Some(popup_surface) = popup_surface {
                    // Add every known window surface to the dismiss group
                    let all_surfaces: Vec<_> = ev
                        .windows()
                        .iter()
                        .map(|unit| unit.get_wlsurface().clone())
                        .collect();
                    for surface in all_surfaces {
                        ev.add_to_dismiss_group(&popup_surface, &surface);
                    }
                }
            }
            LayershellCustomAction::RemoveMainSurfaceFromDismissGroup => {
                // Get the popup surface
                let popup_surface = layer_shell_id.and_then(|id| {
                    ev.get_unit_with_id(id)
                        .map(|unit| unit.get_wlsurface().clone())
                });
                // Get the main (first) surface
                let main_surface = ev.main_window().get_wlsurface().clone();
                if let Some(popup_surface) = popup_surface {
                    ev.remove_from_dismiss_group(&popup_surface, &main_surface);
                }
            }
        }
    }

    fn handle_normal_dispatch(&mut self, ev: &mut WindowState<IcedId>) {
        #[cfg(all(feature = "automation", target_os = "linux"))]
        self.serve_automation(ev);

        if self.iced_events.is_empty() && self.messages.is_empty() {
            return;
        }

        tracing::debug!(
            "handle_normal_dispatch: iced_events={}, messages={}",
            self.iced_events.len(),
            self.messages.len()
        );

        let mut rebuilds = Vec::new();
        let mut deferred_events = Vec::new();
        // Group only adjacent events: a release in a departing window must reach
        // its widgets and subscriptions before a later press in another window.
        for (iced_id, window_events) in event_queue::batches(mem::take(&mut self.iced_events)) {
            let Some(window) = self.window_manager.get_mut(iced_id) else {
                deferred_events.extend(window_events.into_iter().map(|event| (iced_id, event)));
                continue;
            };

            let interact_span = iced_debug::interact(iced_id);
            let logical_size = window.state.viewport().logical_size();
            let (ui_state, statuses) = self
                .user_interfaces
                .ui_mut(&iced_id, &mut window.renderer, logical_size)
                .expect("Get user interface")
                .update(
                    &window_events,
                    window.state.cursor(),
                    &mut window.renderer,
                    &mut self.messages,
                );

            #[cfg(feature = "unconditional-rendering")]
            let unconditional_rendering = true;
            #[cfg(not(feature = "unconditional-rendering"))]
            let unconditional_rendering = false;
            if Self::handle_ui_state(
                ev,
                window,
                ui_state,
                unconditional_rendering,
                false,
                &mut self.clipboard,
                &mut self.iced_events,
                self.compositor.as_mut(),
            ) {
                rebuilds.push(iced_id);
            }

            for (event, status) in window_events.into_iter().zip(statuses) {
                self.runtime
                    .broadcast(iced_futures::subscription::Event::Interaction {
                        window: iced_id,
                        event,
                        status,
                    });
            }
            interact_span.finish();
        }

        if !deferred_events.is_empty() {
            deferred_events.append(&mut self.iced_events);
            self.iced_events = deferred_events;
            // They wait for a window still joining; come back for them.
            ev.wake_soon();
        }

        if !self.messages.is_empty() {
            ev.request_refresh_all(RefreshRequest::NextFrame);
            // Every window's view may depend on the state these messages change,
            // so all of them go stale. They are NOT rebuilt here: a task chain
            // that hands one message back at a time (`Task::done`) re-enters this
            // branch once per message, and rebuilding eagerly would pay a `view()`
            // + layout per window per hop. The refresh above brings each window
            // through `ui_mut`, which rebuilds it once, right before it is drawn.
            let application = self.user_interfaces.invalidate_all();

            // Update application
            update(
                application,
                &mut self.runtime,
                &mut self.messages,
                &mut self.waiting_layer_shell_actions,
            );

            for (_, window) in self.window_manager.iter_mut() {
                window.state.synchronize(application);
            }
            iced_debug::theme_changed(|| {
                self.window_manager
                    .first()
                    .and_then(|window| theme::Base::palette(window.state.theme()))
            });
        } else {
            // A widget reported its tree outdated. Same deal: mark it and let the
            // refresh it already requested drive the one rebuild.
            for iced_id in rebuilds {
                self.user_interfaces.invalidate(&iced_id);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_ui_state(
        ev: &mut WindowState<IcedId>,
        window: &mut Window<P, C>,
        ui_state: user_interface::State,
        unconditional_rendering: bool,
        update_ime: bool,
        clipboard: &mut LayerShellClipboard,
        iced_events: &mut Vec<(IcedId, IcedEvent)>,
        mut compositor: Option<&mut C>,
    ) -> bool {
        match ui_state {
            user_interface::State::Outdated {
                clipboard: mut clipboard_requests,
                ..
            } => {
                if let Some(c) = compositor.as_deref_mut() {
                    resolve_dnd_icon_elements::<P, C>(
                        &mut clipboard_requests,
                        c,
                        window.state.viewport(),
                        window.state.theme(),
                        window.state.text_color(),
                    );
                }
                // Process clipboard requests even when rebuilding
                run_clipboard(
                    ev,
                    clipboard,
                    clipboard_requests,
                    window.iced_id,
                    iced_events,
                );
                tracing::trace!(
                    "handle_ui_state: Outdated for window {:?} (rebuild needed)",
                    window.iced_id,
                );
                true
            }
            user_interface::State::Updated {
                redraw_request,
                input_method,
                mouse_interaction,
                clipboard: mut clipboard_requests,
                ..
            } => {
                if let Some(c) = compositor {
                    resolve_dnd_icon_elements::<P, C>(
                        &mut clipboard_requests,
                        c,
                        window.state.viewport(),
                        window.state.theme(),
                        window.state.text_color(),
                    );
                }
                // Process clipboard requests from widgets
                run_clipboard(
                    ev,
                    clipboard,
                    clipboard_requests,
                    window.iced_id,
                    iced_events,
                );

                tracing::trace!(
                    "handle_ui_state: Updated for window {:?}, mouse_interaction={:?}, cached={:?}",
                    window.iced_id,
                    mouse_interaction,
                    window.mouse_interaction,
                );
                if unconditional_rendering {
                    ev.request_refresh(window.id, RefreshRequest::NextFrame);
                } else {
                    match redraw_request {
                        RedrawRequest::NextFrame => {
                            ev.request_refresh(window.id, RefreshRequest::NextFrame);
                        }
                        RedrawRequest::At(instant) => {
                            ev.request_refresh(window.id, RefreshRequest::At(instant));
                        }
                        RedrawRequest::Wait => {}
                    }
                }

                if update_ime {
                    let ime_flags = window.request_input_method(input_method.clone());
                    match input_method {
                        iced_core::InputMethod::Disabled => {
                            if ime_flags.contains(ImeState::Disabled) {
                                ev.set_ime_allowed(false);
                            }
                        }
                        iced_core::InputMethod::Enabled {
                            purpose, cursor, ..
                        } => {
                            if ime_flags.contains(ImeState::Allowed) {
                                ev.set_ime_allowed(true);
                            }

                            if ime_flags.contains(ImeState::Update) {
                                ev.set_ime_purpose(conversion::ime_purpose(purpose));
                                ev.set_ime_cursor_area(
                                    layershellev::dpi::LogicalPosition::new(cursor.x, cursor.y),
                                    layershellev::dpi::LogicalSize {
                                        width: cursor.width,
                                        height: cursor.height,
                                    },
                                    window.id,
                                );
                            }

                            if ime_flags.contains(ImeState::Surrounding) {
                                ev.set_ime_surrounding_text(
                                    window
                                        .ime_surrounding
                                        .as_ref()
                                        .map(conversion::ime_surrounding_text)
                                        .as_ref(),
                                );
                            }
                        }
                    }
                }

                if mouse_interaction != window.mouse_interaction {
                    // Only send cursor shape requests when the cursor is
                    // actually over this window.  Without this guard, newly
                    // created windows (e.g. popups) that have no cursor would
                    // override the cursor shape set by the window the pointer
                    // is really on, because they share the same wl_pointer.
                    if window.state.mouse_position().is_some() {
                        tracing::trace!(
                            "Cursor shape changing for window {:?}: {:?} -> {:?}",
                            window.id,
                            window.mouse_interaction,
                            mouse_interaction,
                        );
                        if let Some(pointer) = ev.get_pointer() {
                            ev.append_return_data(ReturnData::RequestSetCursorShape((
                                conversion::mouse_interaction(mouse_interaction),
                                pointer.clone(),
                            )));
                        }
                    }
                    window.mouse_interaction = mouse_interaction;
                }
                false
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update<P: IcedProgram, E: Executor>(
    application: &mut Instance<P>,
    runtime: &mut MultiRuntime<E, P::Message>,
    messages: &mut Vec<P::Message>,
    waiting_layer_shell_actions: &mut Vec<(Option<iced_core::window::Id>, LayershellCustomAction)>,
) where
    P::Theme: DefaultStyle,
    P::Message: 'static + TryInto<LayershellCustomActionWithId, Error = P::Message>,
{
    let waker = futures::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);

    let pending: Vec<_> = std::mem::take(messages);
    for message in pending {
        let task = runtime.enter(|| application.update(message));

        if let Some(mut stream) = iced_runtime::task::into_stream(task) {
            // Synchronously drain any immediately-ready actions from the
            // task stream.  This is critical for `Task::done(msg)` which
            // wraps `future::ready` — the single item resolves on first
            // poll, so we can process it inline instead of spawning it on
            // the async executor and waiting for a channel roundtrip.
            //
            // This eliminates ~16-250ms of latency for show/hide actions
            // that would otherwise need to travel:
            //   update() → tokio spawn → channel send → calloop dispatch
            //
            // We enter the runtime context so that any futures polled
            // inline (e.g. zbus property caches) can access the Tokio
            // reactor if needed.
            loop {
                match runtime.enter(|| stream.as_mut().poll_next(&mut cx)) {
                    Poll::Ready(Some(action)) => {
                        // Process immediately via run_action's Output path
                        match action {
                            Action::Output(msg) => match msg.try_into() {
                                Ok(action) => {
                                    let LayershellCustomActionWithId(id, action) = action;
                                    waiting_layer_shell_actions.push((id, action));
                                }
                                Err(msg) => {
                                    // Not a layer shell action — re-queue as message
                                    messages.push(msg);
                                }
                            },
                            other => {
                                // Non-Output actions (clipboard, widget, window, etc.)
                                // still need to go through the runtime since we don't
                                // have all the context needed to handle them here.
                                // Wrap it back into a stream for the runtime.
                                let remaining = futures::stream::once(async move { other })
                                    .chain(stream)
                                    .boxed();
                                runtime.run(remaining);
                                break;
                            }
                        }
                    }
                    Poll::Ready(None) => break, // stream exhausted
                    Poll::Pending => {
                        // Async items remain — hand the rest to the runtime
                        runtime.run(stream);
                        break;
                    }
                }
            }
        }
    }

    let subscription = runtime.enter(|| application.subscription());
    let recipes = iced_futures::subscription::into_recipes(subscription.map(Action::Output));

    iced_debug::subscriptions_tracked(recipes.len());
    runtime.track(recipes);
}

/// Read one [`Kind`] of clipboard content.
///
/// [`Kind::Files`] does not go through `window_clipboard`, which speaks only
/// text: a file list lives on the Wayland selection as `text/uri-list`, and the
/// event loop is the one holding that offer. Everything else is text and reads
/// the way it always has.
///
/// [`Kind`]: iced_core::clipboard::Kind
/// The Wayland surface of the iced window `id`, if it is still shown.
fn window_surface<P, C>(
    window_manager: &WindowManager<P, C>,
    ev: &WindowState<IcedId>,
    id: IcedId,
) -> Option<wayland_client::protocol::wl_surface::WlSurface>
where
    P: IcedProgram,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: DefaultStyle,
{
    let window = window_manager.get(id)?;
    ev.get_unit_with_id(window.id)
        .map(|unit| unit.get_wlsurface().clone())
}

fn read_kind(
    ev: &WindowState<IcedId>,
    clipboard: &LayerShellClipboard,
    kind: iced_core::clipboard::Kind,
) -> Result<iced_core::clipboard::Content, iced_core::clipboard::Error> {
    use iced_core::clipboard::{Content, Error, Kind};

    if kind == Kind::Files {
        return ev
            .read_selection_uri_list()
            .map(Content::Files)
            .ok_or(Error::ContentNotAvailable);
    }
    clipboard.read_sync(kind)
}

/// Process clipboard requests from widget updates and feed results back as events.
fn run_clipboard(
    ev: &mut WindowState<IcedId>,
    clipboard: &mut LayerShellClipboard,
    requests: iced_core::Clipboard,
    window: IcedId,
    iced_events: &mut Vec<(IcedId, IcedEvent)>,
) {
    use std::sync::Arc;

    for kind in requests.reads {
        let result = read_kind(ev, clipboard, kind);
        iced_events.push((
            window,
            IcedEvent::Clipboard(iced_core::clipboard::Event::Read(result.map(Arc::new))),
        ));
    }

    if let Some(content) = requests.write {
        let result = clipboard.write_sync(content);
        iced_events.push((
            window,
            IcedEvent::Clipboard(iced_core::clipboard::Event::Written(result)),
        ));
    }

    // The primary selection is a separate channel from the clipboard, so its
    // results come back as their own events — a middle-click paste must not be
    // swallowed by whichever text input is focused.
    if requests.primary_reads {
        let result = clipboard.read_primary_sync();
        iced_events.push((
            window,
            IcedEvent::Clipboard(iced_core::clipboard::Event::PrimaryRead(
                result.map(Arc::new),
            )),
        ));
    }

    if let Some(text) = requests.primary_write {
        let result = clipboard.write_primary_sync(text);
        iced_events.push((
            window,
            IcedEvent::Clipboard(iced_core::clipboard::Event::PrimaryWritten(result)),
        ));
    }

    // Outgoing drag-and-drop requests issued by widgets via `shell.start_dnd(..)`.
    // We support starting a drag (source side); the incoming drop path is
    // auto-handled in layershellev, so the other requests are no-ops here.
    for request in requests.dnd_requests {
        if let iced_core::dnd::Request::StartDrag {
            mime_types,
            actions,
            data,
            icon,
            hotspot,
            ..
        } = request
        {
            // Element icons are resolved to Pixels by `resolve_dnd_icon_elements`
            // before this point; the pixels are pre-multiplied ARGB (== Argb8888),
            // which layershellev wraps in an SHM drag-icon surface.
            let icon = match icon {
                Some(iced_core::dnd::DndIcon::Pixels {
                    width,
                    height,
                    pixels,
                    scale,
                }) => Some(layershellev::DndIconPixels {
                    width,
                    height,
                    scale,
                    argb: pixels,
                }),
                _ => None,
            };
            ev.start_drag(mime_types, actions.bits(), data, icon, hotspot);
        }
    }
}

/// Render any `DndIcon::Element` in the pending drag requests to pre-multiplied
/// ARGB pixels (offscreen, via the compositor), mirroring iced_winit. After this,
/// `run_clipboard` only sees `DndIcon::Pixels`.
fn resolve_dnd_icon_elements<P, C>(
    clipboard_requests: &mut iced_core::Clipboard,
    compositor: &mut C,
    viewport: &iced_graphics::Viewport,
    theme: &P::Theme,
    text_color: iced_core::Color,
) where
    P: IcedProgram + 'static,
    C: Compositor<Renderer = P::Renderer>,
    P::Renderer: 'static,
{
    for req in &mut clipboard_requests.dnd_requests {
        let iced_core::dnd::Request::StartDrag { icon, .. } = req else {
            continue;
        };
        if !matches!(icon, Some(iced_core::dnd::DndIcon::Element(_))) {
            continue;
        }
        let Some(iced_core::dnd::DndIcon::Element(elem_icon)) = icon.take() else {
            continue;
        };
        let Some((mut element, state)) = elem_icon.downcast::<P::Theme, P::Renderer>() else {
            continue;
        };

        let mut icon_renderer = compositor.create_renderer();
        let icon_scale = viewport.scale_factor().max(2.0);
        let lim = iced_core::layout::Limits::new(
            iced_core::Size::new(1.0, 1.0),
            iced_core::Size::new(
                viewport.physical_width() as f32,
                viewport.physical_height() as f32,
            ),
        );
        let mut tree = iced_core::widget::Tree {
            tag: element.as_widget().tag(),
            state,
            children: element.as_widget().children(),
        };
        let layout_node = element
            .as_widget_mut()
            .layout(&mut tree, &icon_renderer, &lim);
        let size = lim.resolve(
            iced_core::Length::Shrink,
            iced_core::Length::Shrink,
            layout_node.size(),
        );
        let physical = iced_core::Size::new(
            (size.width * icon_scale).ceil() as u32,
            (size.height * icon_scale).ceil() as u32,
        );
        let icon_viewport = iced_graphics::Viewport::with_physical_size(physical, icon_scale);
        let mut ui = user_interface::UserInterface::build(
            element,
            size,
            user_interface::Cache::default(),
            &mut icon_renderer,
        );
        ui.draw(
            &mut icon_renderer,
            theme,
            &iced_core::renderer::Style { text_color },
            iced_core::mouse::Cursor::Unavailable,
        );
        let mut bytes = compositor.screenshot(
            &mut icon_renderer,
            &icon_viewport,
            iced_core::Color::TRANSPARENT,
        );
        // RGBA -> pre-multiplied ARGB (Argb8888 byte order).
        for pix in bytes.as_chunks_mut::<4>().0 {
            pix.swap(0, 2);
        }
        *icon = Some(iced_core::dnd::DndIcon::Pixels {
            width: icon_viewport.physical_width(),
            height: icon_viewport.physical_height(),
            pixels: bytes,
            scale: icon_scale.ceil() as i32,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_action<P, C, E: Executor>(
    user_interfaces: &mut UserInterfaces<P>,
    compositor: &mut Option<C>,
    event: Action<P::Message>,
    messages: &mut Vec<P::Message>,
    clipboard: &mut LayerShellClipboard,
    waiting_layer_shell_actions: &mut Vec<(Option<iced_core::window::Id>, LayershellCustomAction)>,
    should_exit: &mut bool,
    window_manager: &mut WindowManager<P, C>,
    system_theme: &mut iced_core::theme::Mode,
    runtime: &mut MultiRuntime<E, P::Message>,
    ev: &mut WindowState<IcedId>,
    iced_events: &mut Vec<(IcedId, IcedEvent)>,
    pending_popups: &mut HashMap<IcedId, layershellev::id::Id>,
    cancelled_popups: &mut std::collections::HashSet<IcedId>,
    popup_pending_shadow: &mut std::collections::HashSet<IcedId>,
    fitted_popups: &mut Vec<FittedPopup>,
) where
    P: IcedProgram + 'static,
    C: Compositor<Renderer = P::Renderer> + 'static,
    P::Theme: DefaultStyle,
    P::Message: 'static + TryInto<LayershellCustomActionWithId, Error = P::Message>,
{
    use iced_core::widget::operation;
    use iced_runtime::Action;
    use iced_runtime::clipboard;

    use iced_runtime::window::Action as WindowAction;
    match event {
        Action::Output(stream) => match stream.try_into() {
            Ok(action) => {
                let LayershellCustomActionWithId(id, action) = action;
                waiting_layer_shell_actions.push((id, action));
            }
            Err(stream) => {
                messages.push(stream);
            }
        },
        Action::Image(action) => match action {
            iced_runtime::image::Action::Allocate(handle, sender) => {
                use iced_core::Renderer as _;

                // TODO: Shared image cache in compositor
                if let Some((_id, window)) = window_manager.iter_mut().next() {
                    window.renderer.allocate_image(&handle, move |allocation| {
                        let _ = sender.send(allocation);
                    });
                }
            }
        },
        Action::Clipboard(action) => match action {
            clipboard::Action::Read { kind, channel } => {
                let _ = channel.send(read_kind(ev, clipboard, kind));
            }
            clipboard::Action::Write { content, channel } => {
                clipboard.write(content, move |result| {
                    let _ = channel.send(result);
                });
            }
            clipboard::Action::ReadPrimary { channel } => {
                clipboard.read_primary(move |result| {
                    let _ = channel.send(result);
                });
            }
            clipboard::Action::WritePrimary { text, channel } => {
                clipboard.write_primary(text, move |result| {
                    let _ = channel.send(result);
                });
            }
        },
        Action::Widget(action) => {
            let mut current_operation = Some(action);

            while let Some(mut operation) = current_operation.take() {
                // kind of suboptimal that we have to iterate over all windows, but since an operation does not have
                // a window id associated with it, this is the best we can do for now
                for (id, window) in window_manager.iter_mut() {
                    let logical_size = window.state.viewport().logical_size();
                    if let Some(mut ui) =
                        user_interfaces.ui_mut(&id, &mut window.renderer, logical_size)
                    {
                        ui.operate(&window.renderer, operation.as_mut());
                    }
                }

                match operation.finish() {
                    operation::Outcome::None => {}
                    operation::Outcome::Some(()) => {}
                    operation::Outcome::Chain(next) => {
                        current_operation = Some(next);
                    }
                }
            }
            // Request refresh after widget operations (e.g., focus) to update visual state
            ev.request_refresh_all(RefreshRequest::NextFrame);
        }
        Action::Window(action) => match action {
            WindowAction::Close(id) => {
                waiting_layer_shell_actions.push((Some(id), LayershellCustomAction::RemoveWindow));
            }
            WindowAction::GetOldest(channel) => {
                let _ = channel.send(window_manager.first_window().map(|(id, _)| *id));
            }
            WindowAction::GetLatest(channel) => {
                let _ = channel.send(window_manager.last_window().map(|(id, _)| *id));
            }
            WindowAction::GetSize(id, channel) => 'out: {
                let Some(window) = window_manager.get(id) else {
                    break 'out;
                };
                let _ = channel.send(window.state.window_size_f32());
            }
            WindowAction::Screenshot(id, channel) => 'out: {
                let Some(window) = window_manager.get_mut(id) else {
                    break 'out;
                };
                let Some(compositor) = compositor else {
                    break 'out;
                };
                let bytes = compositor.screenshot(
                    &mut window.renderer,
                    window.state.viewport(),
                    window.state.background_color(),
                );

                let _ = channel.send(iced_core::window::Screenshot::new(
                    bytes,
                    window.state.viewport().physical_size(),
                    window.state.viewport().scale_factor(),
                ));
            }
            WindowAction::GetScaleFactor(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let sf = window.state.wayland_scale_factor() as f32;
                    tracing::trace!("GetScaleFactor: id={:?}, wayland_scale_factor={}", id, sf);
                    let _ = channel.send(sf);
                };
            }
            WindowAction::RedrawAll => {
                ev.request_refresh_all(RefreshRequest::NextFrame);
            }
            WindowAction::RelayoutAll => {
                // Stale every user interface; the refresh rebuilds each one once.
                for (iced_id, _window) in window_manager.iter_mut() {
                    user_interfaces.invalidate(&iced_id);
                }
                ev.request_refresh_all(RefreshRequest::NextFrame);
            }
            WindowAction::RequestActivationToken(id, channel) => {
                match window_surface(window_manager, ev, id) {
                    Some(surface) => ev.request_activation_token(&surface, move |token| {
                        let _ = channel.send(token);
                    }),
                    None => {
                        let _ = channel.send(None);
                    }
                }
            }
            WindowAction::Activate(id, token) => {
                if let Some(surface) = window_surface(window_manager, ev, id) {
                    ev.activate_surface_with_token(&surface, token);
                }
            }
            _ => {}
        },
        Action::System(action) => match action {
            iced_runtime::system::Action::GetTheme(channel) => {
                let _ = channel.send(*system_theme);
            }
            iced_runtime::system::Action::NotifyTheme(mode) => {
                if mode != *system_theme {
                    *system_theme = mode;

                    runtime.broadcast(iced_futures::subscription::Event::SystemThemeChanged(mode));
                }

                for (_id, window) in window_manager.iter_mut() {
                    window.state.update(
                        &LayerShellWindowEvent::ThemeChanged(mode),
                        user_interfaces.application(),
                    );
                }

                ev.request_refresh_all(RefreshRequest::NextFrame);
            }

            _ => {}
        },
        Action::Event { window, event } => {
            iced_events.push((window, event));
        }
        Action::Exit => {
            *should_exit = true;
        }
        Action::LoadFont { bytes, channel } => {
            if let Some(compositor) = compositor {
                // TODO: Error handling (?)
                compositor.load_font(bytes.clone());

                let _ = channel.send(Ok(()));
            }
        }
        Action::Reload => {
            for (iced_id, _window) in window_manager.iter_mut() {
                user_interfaces.invalidate(&iced_id);
            }
            ev.request_refresh_all(RefreshRequest::NextFrame);
        }
        Action::Tick => {
            // Tick is handled internally by the runtime
        }
        Action::PlatformSpecific(platform_action) => {
            use iced_runtime::platform_specific;
            match platform_action {
                platform_specific::Action::Wayland(wayland_action) => match wayland_action {
                    platform_specific::wayland::Action::Popup(popup_action) => {
                        use iced_runtime::platform_specific::wayland::popup;
                        match popup_action {
                            popup::Action::Show { settings } => {
                                let parent_iced_id = settings.parent;
                                let popup_iced_id = settings.id;
                                let positioner = &settings.positioner;

                                // If this popup was already cancelled by a Hide
                                // that arrived before this Show, skip creation.
                                if cancelled_popups.remove(&popup_iced_id) {
                                    popup_pending_shadow.remove(&popup_iced_id);
                                    tracing::debug!(
                                        popup = ?popup_iced_id,
                                        "xdg_popup: Show skipped — popup was pre-cancelled by Hide"
                                    );
                                    return;
                                }

                                tracing::debug!(
                                    parent = ?parent_iced_id,
                                    popup = ?popup_iced_id,
                                    size = ?positioner.size,
                                    anchor = ?positioner.anchor,
                                    gravity = ?positioner.gravity,
                                    anchor_rect = ?positioner.anchor_rect,
                                    offset = ?positioner.offset,
                                    "xdg_popup: processing Show action"
                                );

                                // Look up the parent's layershell ID
                                let Some(parent_window) = window_manager.get_mut(parent_iced_id)
                                else {
                                    tracing::warn!(
                                        parent = ?parent_iced_id,
                                        "xdg_popup show: parent window not found"
                                    );
                                    return;
                                };
                                let parent_layer_shell_id = parent_window.id;

                                // Without a size, the popup fits its content, measured now
                                // and again whenever it is drawn (see `auto_size_enabled`).
                                let (width, height) = match positioner.size {
                                    Some(size) => size,
                                    None => {
                                        let (min, max) = (
                                            positioner.size_limits.min(),
                                            positioner.size_limits.max(),
                                        );
                                        let content = user_interfaces.measure(
                                            popup_iced_id,
                                            &mut parent_window.renderer,
                                            max,
                                        );
                                        let fit = |content: f32, min: f32, max: f32| {
                                            content.ceil().clamp(min.max(1.0), max.max(1.0)) as u32
                                        };
                                        let size = (
                                            fit(content.width, min.width, max.width),
                                            fit(content.height, min.height, max.height),
                                        );
                                        let max = (max.width as u32, max.height as u32);
                                        fitted_popups.push((popup_iced_id, max, size));
                                        size
                                    }
                                };

                                let popup_settings = NewPopUpSettings {
                                    size: (width, height),
                                    position: (positioner.anchor_rect.x, positioner.anchor_rect.y),
                                    id: parent_layer_shell_id,
                                    blur: settings.blur,
                                    shadow: settings.shadow,
                                    corner_radius: settings.corner_radius.map(|radius| [radius; 4]),
                                    auto_size: positioner.size.is_none(),
                                    anchor_rect_size: Some((
                                        positioner.anchor_rect.width,
                                        positioner.anchor_rect.height,
                                    )),
                                    anchor: positioner.anchor as u32,
                                    gravity: positioner.gravity as u32,
                                    constraint_adjustment: positioner.constraint_adjustment,
                                    offset: if positioner.offset != (0, 0) {
                                        Some(positioner.offset)
                                    } else {
                                        None
                                    },
                                    reactive: positioner.reactive,
                                    grab: settings.grab,
                                    input_passthrough: settings.input_passthrough,
                                    tooltip_offset: settings.tooltip_offset,
                                    tooltip_anchor: settings.tooltip_anchor,
                                    tooltip_delay_ms: settings.tooltip_delay_ms,
                                    capture_excluded: popup::excluded_from_capture(popup_iced_id),
                                };

                                let layer_shell_id = layershellev::id::Id::unique();
                                // Track popup before it's registered in window_manager
                                // so that a Hide arriving before WindowOpened can still
                                // find and close the layershell surface.
                                pending_popups.insert(popup_iced_id, layer_shell_id);
                                // Defer shadow for popups so it's applied after
                                // the first content frame (avoids transparent flash).
                                if settings.shadow || ev.has_shadow() {
                                    popup_pending_shadow.insert(popup_iced_id);
                                }
                                ev.append_return_data(ReturnData::NewPopUp((
                                    popup_settings,
                                    layer_shell_id,
                                    Some(popup_iced_id),
                                )));
                            }
                            popup::Action::Hide { id } => {
                                popup::forget_capture_exclusion(id);
                                // Map from iced ID to layershell ID and request close.
                                // Check window_manager first (popup fully initialized),
                                // then pending_popups (Show processed but WindowOpened
                                // not yet received).
                                if let Some(window) = window_manager.get(id) {
                                    tracing::debug!(
                                        iced_id = ?id,
                                        layer_id = ?window.id,
                                        "xdg_popup: Hide via window_manager"
                                    );
                                    pending_popups.remove(&id);
                                    popup_pending_shadow.remove(&id);
                                    // Faded out where the compositor can, then closed.
                                    if !ev.fade_out_popup(window.id, POPUP_FADE_OUT) {
                                        ev.request_close(window.id);
                                    }
                                } else if let Some(layer_id) = pending_popups.remove(&id) {
                                    tracing::debug!(
                                        iced_id = ?id,
                                        layer_id = ?layer_id,
                                        "xdg_popup: Hide via pending_popups (before WindowOpened)"
                                    );
                                    popup_pending_shadow.remove(&id);
                                    ev.request_close(layer_id);
                                } else {
                                    // Hide arrived before Show was processed.
                                    // Remember this ID so when Show arrives later
                                    // we skip creating the popup entirely.
                                    tracing::debug!(
                                        iced_id = ?id,
                                        "xdg_popup: Hide before Show — marking as cancelled"
                                    );
                                    cancelled_popups.insert(id);
                                }
                            }
                            popup::Action::Resize { id, width, height } => {
                                let Some(layer_id) = window_manager.get(id).map(|w| w.id) else {
                                    tracing::warn!(popup = ?id, "xdg_popup resize: popup not found");
                                    return;
                                };
                                if let Some(unit) = ev.get_unit_with_id(layer_id) {
                                    unit.set_size((width, height));
                                }
                            }
                            popup::Action::Reposition { id, positioner } => {
                                // Look up the popup's layershell ID
                                let layer_shell_id = window_manager
                                    .get(id)
                                    .map(|w| w.id)
                                    .or_else(|| pending_popups.get(&id).copied());

                                let Some(layer_id) = layer_shell_id else {
                                    tracing::warn!(
                                        popup = ?id,
                                        "xdg_popup reposition: popup not found"
                                    );
                                    return;
                                };

                                // A popup fitted to its content keeps the size it has.
                                let (width, height) = positioner
                                    .size
                                    .or_else(|| {
                                        let size = window_manager.get(id)?.state.window_size();
                                        Some((size.width, size.height))
                                    })
                                    .unwrap_or((
                                        positioner.size_limits.max().width as u32,
                                        positioner.size_limits.max().height as u32,
                                    ));

                                let reposition_settings = layershellev::RepositionPopUpSettings {
                                    popup_id: layer_id,
                                    size: (width, height),
                                    position: (positioner.anchor_rect.x, positioner.anchor_rect.y),
                                    anchor_rect_size: Some((
                                        positioner.anchor_rect.width,
                                        positioner.anchor_rect.height,
                                    )),
                                    anchor: positioner.anchor as u32,
                                    gravity: positioner.gravity as u32,
                                    constraint_adjustment: positioner.constraint_adjustment,
                                    offset: if positioner.offset != (0, 0) {
                                        Some(positioner.offset)
                                    } else {
                                        None
                                    },
                                    reactive: positioner.reactive,
                                };

                                tracing::debug!(
                                    popup = ?id,
                                    layer_id = ?layer_id,
                                    position = ?(positioner.anchor_rect.x, positioner.anchor_rect.y),
                                    "xdg_popup: processing Reposition action"
                                );

                                ev.append_return_data(ReturnData::RepositionPopUp(
                                    reposition_settings,
                                ));
                            }
                        }
                    }
                },
            }
        }
    }
}
