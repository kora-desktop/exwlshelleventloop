//! Publishes each surface's accessibility tree to assistive technology
//! (AT-SPI on Linux) and replays the actions it asks for.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use iced_accessibility::accesskit::{Action as AtAction, NodeId};
use iced_accessibility::{Adapter, TreeBuilder, Waker};
use iced_core::keyboard::key::Named;
use iced_core::widget::operation::focusable;
use iced_core::window::{Event as IcedWindowEvent, Id as IcedId};
use iced_core::{Event as IcedEvent, Point, Rectangle, widget};
use iced_runtime::Action;

use crate::proxy::IcedProxy;

type NodeMap = HashMap<NodeId, (Option<widget::Id>, Rectangle)>;

/// What a surface must do for one action assistive technology asked for.
pub(crate) enum Replay {
    /// Move keyboard focus to the widget with this id.
    Focus(IcedId, widget::Id),
    /// Put iced's cursor here on the surface, then feed it these events.
    Input(IcedId, Point, Vec<IcedEvent>),
}

/// The accessibility state of every surface of one app.
pub(crate) struct A11y {
    /// Names a surface whose program gives it no title.
    pub(crate) namespace: String,
    wake: Waker,
    adapters: HashMap<IcedId, Adapter>,
    node_maps: HashMap<IcedId, NodeMap>,
    dirty: bool,
}

impl A11y {
    pub(crate) fn new<Message: Send + 'static>(proxy: IcedProxy<Action<Message>>) -> Self {
        let proxy = Mutex::new(proxy);

        Self {
            namespace: String::new(),
            wake: Arc::new(move || {
                if let Ok(proxy) = proxy.lock() {
                    proxy.send_action(Action::Tick);
                }
            }),
            adapters: HashMap::new(),
            node_maps: HashMap::new(),
            dirty: true,
        }
    }

    /// Marks every tree stale: the next [`Self::refresh`] rebuilds them.
    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Keeps one adapter per open surface and tells each whether its
    /// surface has keyboard focus, from the events about to be handled.
    pub(crate) fn sync<'a>(
        &mut self,
        surfaces: impl Iterator<Item = (IcedId, &'a str)>,
        events: &[(IcedId, IcedEvent)],
    ) {
        let mut open = Vec::new();

        for (id, title) in surfaces {
            open.push(id);
            let title = if title.is_empty() {
                self.namespace.as_str()
            } else {
                title
            };
            let wake = Arc::clone(&self.wake);
            let _ = self
                .adapters
                .entry(id)
                .or_insert_with(|| Adapter::new(id, title, wake));
        }

        self.adapters.retain(|id, _| open.contains(id));
        self.node_maps.retain(|id, _| open.contains(id));

        for (id, event) in events {
            let focused = match event {
                IcedEvent::Window(IcedWindowEvent::Focused) => true,
                IcedEvent::Window(IcedWindowEvent::Unfocused) => false,
                _ => continue,
            };

            if let Some(adapter) = self.adapters.get_mut(id) {
                adapter.set_focused(focused);
            }
        }
    }

    /// Turns the actions assistive technology asked for into what each
    /// surface must replay.
    pub(crate) fn drain(&mut self) -> Vec<Replay> {
        let mut replays = Vec::new();

        for (id, adapter) in &self.adapters {
            for request in adapter.drain_action_requests() {
                let Some((widget_id, bounds)) = self
                    .node_maps
                    .get(id)
                    .and_then(|nodes| nodes.get(&request.request.target_node))
                else {
                    continue;
                };
                let center = bounds.center();

                let replay = match request.request.action {
                    AtAction::Focus => match widget_id {
                        Some(widget_id) => Replay::Focus(*id, widget_id.clone()),
                        None => Replay::Input(
                            *id,
                            center,
                            vec![iced_accessibility::synthetic_cursor_move(center)],
                        ),
                    },
                    AtAction::Click => Replay::Input(
                        *id,
                        center,
                        iced_accessibility::synthetic_click(center).into(),
                    ),
                    AtAction::Increment => Replay::Input(
                        *id,
                        center,
                        iced_accessibility::synthetic_arrow_key(center, Named::ArrowUp).into(),
                    ),
                    AtAction::Decrement => Replay::Input(
                        *id,
                        center,
                        iced_accessibility::synthetic_arrow_key(center, Named::ArrowDown).into(),
                    ),
                    other => {
                        log::debug!("a11y: unhandled action {other:?}");
                        continue;
                    }
                };

                replays.push(replay);
                self.dirty = true;
            }
        }

        replays
    }

    /// The focus operation for a [`Replay::Focus`].
    pub(crate) fn focus(widget_id: widget::Id) -> impl iced_core::widget::Operation {
        focusable::focus(widget_id)
    }

    /// Whether a surface's tree must be rebuilt: something changed, or
    /// assistive technology just connected and holds only a placeholder.
    pub(crate) fn wants_tree(&mut self, id: IcedId) -> bool {
        let Some(adapter) = self.adapters.get(&id) else {
            return false;
        };
        let activated = adapter.take_activated();

        adapter.is_active() && (self.dirty || activated)
    }

    /// Builds a surface's tree with `operate`, which walks its widgets, and
    /// publishes it.
    pub(crate) fn publish(
        &mut self,
        id: IcedId,
        title: &str,
        announcements: &[String],
        operate: impl FnOnce(&mut TreeBuilder),
    ) {
        let Some(adapter) = self.adapters.get_mut(&id) else {
            return;
        };
        let title = if title.is_empty() {
            self.namespace.as_str()
        } else {
            title
        };

        let mut builder = TreeBuilder::new(title).with_announcements(announcements);
        operate(&mut builder);
        let tree = builder.build();

        let _ = self.node_maps.insert(id, tree.node_map);
        adapter.update_if_active(|| tree.update);
    }

    /// Marks the trees fresh, after every surface had its chance.
    pub(crate) fn published(&mut self) {
        self.dirty = false;
    }
}
