use super::{PopupPlacement, Shell, WindowState, WindowStateUnitBuilder, id};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::{Connection, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

struct Fixture {
    _connection: Connection,
    state: WindowState<()>,
    parent: id::Id,
    popup: id::Id,
}

/// A layer surface and a popup that opens from it, on a connection with no compositor.
fn fixture(grab: bool) -> Fixture {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh: QueueHandle<WindowState<()>> = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
    let wm_base = registry.bind::<XdgWmBase, _, _>(2, 6, &qh, ());
    let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(3, 4, &qh, ());
    let mut state = WindowState::<()>::new("popup-fade-test");

    let surface = compositor.create_surface(&qh, ());
    let layer = layer_shell.get_layer_surface(&surface, None, Layer::Top, "bar".into(), &qh, ());
    let parent = id::Id::unique();
    state.push_window(
        WindowStateUnitBuilder::new(
            parent,
            qh.clone(),
            connection.display(),
            surface,
            Shell::LayerShell(layer),
        )
        .build(),
    );

    let surface = compositor.create_surface(&qh, ());
    let xdg = wm_base.get_xdg_surface(&surface, &qh, ());
    let placement = PopupPlacement {
        parent,
        grab,
        wm_base: wm_base.clone(),
        anchor_rect: (0, 0, 10, 10),
        anchor: 1,
        gravity: 1,
        constraint_adjustment: 15,
        offset: None,
        reactive: false,
    };
    let positioner = placement.positioner(&qh, (40, 20));
    let xdg_popup = xdg.get_popup(None, &positioner, &qh, ());
    let popup = id::Id::unique();
    state.push_window(
        WindowStateUnitBuilder::new(
            popup,
            qh.clone(),
            connection.display(),
            surface,
            Shell::PopUp((xdg_popup, xdg)),
        )
        .popup_placement(placement)
        .build(),
    );
    Fixture {
        _connection: connection,
        state,
        parent,
        popup,
    }
}

fn unit(state: &mut WindowState<()>, id: id::Id) -> &mut super::WindowStateUnit<()> {
    state.units.iter_mut().find(|u| u.id == id).unwrap()
}

/// Destroying a layer surface while a popup of it lives on, fading, would leave the popup
/// without its parent. It goes first.
#[test]
fn a_surface_closes_after_its_popups() {
    let Fixture {
        mut state,
        parent,
        popup,
        ..
    } = fixture(false);
    unit(&mut state, parent).request_close();
    assert_eq!(state.closing_order(), vec![popup, parent]);
}

#[test]
fn a_fading_popup_draws_nothing_and_closes_when_the_fade_ends() {
    let Fixture {
        mut state, popup, ..
    } = fixture(false);
    let fading = unit(&mut state, popup);
    fading.configured = true;
    fading.request_refresh(super::RefreshRequest::NextFrame);
    fading.fade_close = Some(Instant::now() + Duration::from_secs(60));
    assert!(!fading.take_present_slot(), "the last frame is what fades");
    assert!(state.closing_order().is_empty());
    assert!(
        state.next_timed_refresh().is_some(),
        "a wake-up for the end"
    );

    unit(&mut state, popup).fade_close = Some(Instant::now() - Duration::from_millis(1));
    assert!(state.pass_due_now(false));
    assert_eq!(state.closing_order(), vec![popup]);
}

#[test]
fn a_hidden_popup_fades_where_the_compositor_can() {
    use super::layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1;
    let Fixture {
        _connection: connection,
        mut state,
        popup,
        ..
    } = fixture(false);
    let queue = connection.new_event_queue::<WindowState<()>>();
    let registry = connection.display().get_registry(&queue.handle(), ());
    state.layer_surface_visibility_manager = Some(
        registry.bind::<ZcosmicLayerSurfaceVisibilityManagerV1, _, _>(
            4,
            5,
            &queue.handle(),
            super::layer_surface_visibility::LayerSurfaceVisibilityManagerData,
        ),
    );
    unit(&mut state, popup).configured = true;
    assert!(state.fade_out_popup(popup, Duration::from_millis(150)));
    assert!(unit(&mut state, popup).fade_close.is_some());
    assert!(
        state.closing_order().is_empty(),
        "it lives on while it fades"
    );
}

#[test]
fn a_popup_closes_at_once_where_it_cannot_fade() {
    // No visibility protocol here, so nothing fades.
    let Fixture {
        mut state, popup, ..
    } = fixture(false);
    unit(&mut state, popup).configured = true;
    assert!(!state.fade_out_popup(popup, Duration::from_millis(150)));
    assert_eq!(unit(&mut state, popup).fade_close, None);

    // A menu holding the grab would keep it while fading.
    let Fixture {
        mut state, popup, ..
    } = fixture(true);
    unit(&mut state, popup).configured = true;
    assert!(!state.fade_out_popup(popup, Duration::from_millis(150)));
}

/// A surface closed by a task's message (a toolbar that faded out, say) is closed between
/// passes, and a hidden one brings no Wayland event to wake the loop, so the pass that
/// closes it runs at once.
#[test]
fn a_surface_asked_to_close_between_passes_runs_the_next_pass_at_once() {
    let Fixture {
        mut state, parent, ..
    } = fixture(false);
    // Opening the surfaces queued their focus news; once a pass took it, the app sleeps.
    state.message.clear();
    assert!(!state.pass_due_now(false), "an idle app sleeps");
    unit(&mut state, parent).request_close();
    assert!(state.pass_due_now(false), "the close runs now");
}
