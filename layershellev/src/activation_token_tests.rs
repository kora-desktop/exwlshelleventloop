use super::activation_token::TokenRequest;
use super::{Shell, WindowState, WindowStateUnitBuilder, id};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::{
    xdg_activation_token_v1::{self, XdgActivationTokenV1},
    xdg_activation_v1::XdgActivationV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

struct Fixture {
    connection: Connection,
    _peer: UnixStream,
    qh: QueueHandle<WindowState<()>>,
    state: WindowState<()>,
    surface: WlSurface,
}

fn fixture(activation: bool) -> Fixture {
    let (socket, peer) = UnixStream::pair().unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
    let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(2, 4, &qh, ());
    let surface = compositor.create_surface(&qh, ());
    let layer = layer_shell.get_layer_surface(&surface, None, Layer::Top, "panel".into(), &qh, ());
    let mut state = WindowState::<()>::new("activation-token-test");
    if activation {
        state.xdg_activation = Some(registry.bind::<XdgActivationV1, _, _>(3, 1, &qh, ()));
    }
    state.push_window(
        WindowStateUnitBuilder::new(
            id::Id::unique(),
            qh.clone(),
            connection.display(),
            surface.clone(),
            Shell::LayerShell(layer),
        )
        .build(),
    );
    Fixture {
        connection,
        _peer: peer,
        qh,
        state,
        surface,
    }
}

impl Fixture {
    /// Ask for a token; what the reply receives comes out of the returned channel.
    fn ask(
        &mut self,
    ) -> (
        Option<(XdgActivationTokenV1, WlCallback)>,
        mpsc::Receiver<Option<String>>,
    ) {
        let (sent, received) = mpsc::channel();
        let surface = self.surface.clone();
        let asked = self.state.mint_activation_token(
            &surface,
            Box::new(move |token| {
                let _ = sent.send(token);
            }),
        );
        (asked, received)
    }

    fn token_done(&mut self, token: &XdgActivationTokenV1, value: &str) {
        let request = token.data::<TokenRequest>().unwrap().clone();
        <WindowState<()> as Dispatch<XdgActivationTokenV1, TokenRequest>>::event(
            &mut self.state,
            token,
            xdg_activation_token_v1::Event::Done {
                token: value.into(),
            },
            &request,
            &self.connection,
            &self.qh,
        );
    }

    fn sync_done(&mut self, callback: &WlCallback) {
        let request = callback.data::<TokenRequest>().unwrap().clone();
        <WindowState<()> as Dispatch<WlCallback, TokenRequest>>::event(
            &mut self.state,
            callback,
            wl_callback::Event::Done { callback_data: 0 },
            &request,
            &self.connection,
            &self.qh,
        );
    }
}

#[test]
fn the_compositors_token_reaches_the_reply_once() {
    let mut fixture = fixture(true);
    let (asked, reply) = fixture.ask();
    let (token, answered) = asked.expect("a token was asked for");
    assert!(
        reply.try_recv().is_err(),
        "nothing before the compositor answers"
    );

    fixture.token_done(&token, "launch-1");
    assert_eq!(reply.try_recv(), Ok(Some("launch-1".to_owned())));

    fixture.sync_done(&answered);
    assert!(reply.try_recv().is_err(), "the sync after it adds nothing");
}

/// The panel launches on this answer, so a compositor that keeps quiet must
/// not hold the launch back.
#[test]
fn a_compositor_that_reads_the_request_without_a_token_answers_none() {
    let mut fixture = fixture(true);
    let (asked, reply) = fixture.ask();
    let (token, answered) = asked.expect("a token was asked for");

    fixture.sync_done(&answered);
    assert_eq!(reply.try_recv(), Ok(None));

    fixture.token_done(&token, "too-late");
    assert!(
        reply.try_recv().is_err(),
        "a late token is not a second answer"
    );
}

#[test]
fn without_xdg_activation_the_answer_is_none_at_once() {
    let mut fixture = fixture(false);
    let (asked, reply) = fixture.ask();
    assert!(asked.is_none());
    assert_eq!(reply.try_recv(), Ok(None));
}

#[test]
fn a_request_whose_connection_is_gone_answers_none() {
    let mut fixture = fixture(true);
    let (asked, reply) = fixture.ask();
    drop(asked);
    drop(fixture);
    assert_eq!(reply.try_recv(), Ok(None));
}
