use super::cursor_tests::requests;
use super::{WindowState, layer_surface_visibility};
use std::os::unix::net::UnixStream;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::{Connection, Proxy};

const GET_VISIBILITY_CONTROLLER: u16 = 1;
const SET_HIDDEN: u16 = 1;
const SET_HIDDEN_IMMEDIATELY: u16 = 4;
const COMMIT: u16 = 6;

/// What a daemon's startup surface sends with `start_hidden` set as given, against a
/// compositor offering `layer_surface_visibility` at `version`. Each request is
/// (object, opcode), and the surface's id comes back with them.
fn first_commit(start_hidden: Option<bool>, version: u32) -> (u32, u32, Vec<(u32, u16)>) {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
    let manager = registry.bind::<
        layer_surface_visibility::zcosmic_layer_surface_visibility_manager_v1::ZcosmicLayerSurfaceVisibilityManagerV1,
        _,
        _,
    >(2, version, &qh, layer_surface_visibility::LayerSurfaceVisibilityManagerData);
    let manager_id = manager.id().protocol_id();
    let surface = compositor.create_surface(&qh, ());
    let mut state = WindowState::<()>::new("start-hidden-test");
    if let Some(without_animating) = start_hidden {
        state = state.with_start_hidden(true, without_animating);
    }
    state.layer_surface_visibility_manager = Some(manager);
    connection.flush().unwrap();
    requests(&mut peer);

    state.commit_startup_surface(&surface, &qh);
    connection.flush().unwrap();
    let sent = requests(&mut peer)
        .into_iter()
        .map(|(object, opcode, _)| (object, opcode))
        .collect();
    (surface.id().protocol_id(), manager_id, sent)
}

/// A surface mapped visible has no hidden state for its first show to open from, so the
/// compositor plays no open animation on it: the Kora menu's first open after sign-in.
#[test]
fn a_surface_that_starts_hidden_is_hidden_before_it_maps() {
    for (without_animating, version, hide) in [
        (true, 4, SET_HIDDEN_IMMEDIATELY),
        (false, 4, SET_HIDDEN),
        (true, 3, SET_HIDDEN),
    ] {
        let (surface, manager, sent) = first_commit(Some(without_animating), version);
        let [
            (get_object, get),
            (controller, hidden),
            ..,
            (committed, commit),
        ] = sent[..]
        else {
            panic!("expected a controller, a hide and a commit, got {sent:?}");
        };
        assert_eq!((get_object, get), (manager, GET_VISIBILITY_CONTROLLER));
        assert_eq!(
            hidden, hide,
            "v{version}, without_animating {without_animating}"
        );
        assert_ne!(controller, surface);
        assert_eq!(
            (committed, commit),
            (surface, COMMIT),
            "the hide goes before the commit"
        );
    }
}

#[test]
fn a_surface_that_does_not_start_hidden_just_commits() {
    let (surface, _, sent) = first_commit(None, 4);
    assert_eq!(sent, [(surface, COMMIT)]);
}
