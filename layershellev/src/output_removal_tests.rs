//! A bar per output (`AllScreens`) keeps one surface per output: a removed output takes its
//! surface with it, and an output that is gone, or already has a surface, gets no new one.

use super::{Shell, WindowState, WindowStateUnitBuilder, id};
use std::os::unix::net::UnixStream;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_output::WlOutput};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

struct Fixture {
    connection: Connection,
    _peer: UnixStream,
    qh: QueueHandle<WindowState<()>>,
    registry: WlRegistry,
    compositor: WlCompositor,
    layer_shell: ZwlrLayerShellV1,
    state: WindowState<()>,
}

impl Fixture {
    fn new() -> Self {
        let (socket, peer) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(socket).unwrap();
        let queue = connection.new_event_queue::<WindowState<()>>();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
        let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(2, 4, &qh, ());
        let state = WindowState::<()>::new("output-removal-test");
        Self {
            connection,
            _peer: peer,
            qh,
            registry,
            compositor,
            layer_shell,
            state,
        }
    }

    /// An output the compositor announced as global `name`.
    fn output(&mut self, name: u32) -> WlOutput {
        let output = self.registry.bind::<WlOutput, _, _>(name, 4, &self.qh, ());
        self.state.outputs.push((name, output.clone()));
        output
    }

    /// The bar surface bound to `output`.
    fn bar_on(&mut self, output: &WlOutput) -> id::Id {
        let surface = self.compositor.create_surface(&self.qh, ());
        let layer = self.layer_shell.get_layer_surface(
            &surface,
            Some(output),
            Layer::Top,
            "bar".into(),
            &self.qh,
            (),
        );
        let unit = id::Id::unique();
        self.state.push_window(
            WindowStateUnitBuilder::new(
                unit,
                self.qh.clone(),
                self.connection.display(),
                surface,
                Shell::LayerShell(layer),
            )
            .wl_output(Some(output.clone()))
            .becreated(true)
            .build(),
        );
        unit
    }

    fn global_remove(&mut self, name: u32) {
        <WindowState<()> as Dispatch<WlRegistry, ()>>::event(
            &mut self.state,
            &self.registry,
            wl_registry::Event::GlobalRemove { name },
            &(),
            &self.connection,
            &self.qh,
        );
    }

    fn closing(&mut self) -> Vec<id::Id> {
        self.state.closing_order()
    }
}

#[test]
fn removing_an_output_closes_its_bar_only() {
    let mut f = Fixture::new();
    let left = f.output(10);
    let right = f.output(11);
    let left_bar = f.bar_on(&left);
    let _right_bar = f.bar_on(&right);

    f.global_remove(10);

    assert_eq!(f.closing(), vec![left_bar]);
}

#[test]
fn an_output_with_a_bar_gets_no_second_one() {
    let mut f = Fixture::new();
    let output = f.output(10);
    assert!(f.state.wants_surface_for(&output));
    f.bar_on(&output);

    assert!(!f.state.wants_surface_for(&output));
}

#[test]
fn an_output_removed_before_its_surface_is_made_gets_none() {
    let mut f = Fixture::new();
    let output = f.output(10);

    f.global_remove(10);

    assert!(!f.state.wants_surface_for(&output));
}

#[test]
fn a_re_announced_output_swaps_its_bar() {
    let mut f = Fixture::new();
    let old = f.output(10);
    let old_bar = f.bar_on(&old);

    f.global_remove(10);
    let new = f.output(12);

    assert_eq!(f.closing(), vec![old_bar]);
    assert!(f.state.wants_surface_for(&new));
    assert!(!f.state.wants_surface_for(&old));
}
