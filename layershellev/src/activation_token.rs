//! xdg-activation tokens a surface asks for on behalf of something it starts.
//!
//! A launcher hands such a token to the app it starts (`XDG_ACTIVATION_TOKEN`)
//! so the compositor lets that app's window take focus. The answer comes back
//! as an event, so it is delivered to a reply the caller gives.

use std::sync::{Arc, Mutex};

use wayland_client::protocol::{
    wl_callback::{self, WlCallback},
    wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_token_v1::{
    self, XdgActivationTokenV1,
};

use crate::WindowState;

/// Takes a requested activation token: the token, or `None` when there is none.
pub type ActivationTokenReply = Box<dyn FnOnce(Option<String>) + Send>;

/// One token request, shared by the token object and the `wl_display.sync`
/// sent after it.
///
/// The compositor answers a token when it reads its commit, so by the time it
/// answers that sync it has either sent the token or will not: the sync is the
/// end of the wait, with no timer. The reply runs once; a request dropped
/// unanswered, its connection gone, answers `None`.
#[derive(Clone)]
pub(crate) struct TokenRequest(Arc<TokenReply>);

pub(crate) struct TokenReply(Mutex<Option<ActivationTokenReply>>);

impl TokenRequest {
    fn new(reply: ActivationTokenReply) -> Self {
        Self(Arc::new(TokenReply(Mutex::new(Some(reply)))))
    }

    pub(crate) fn answer(&self, token: Option<String>) {
        self.0.answer(token);
    }
}

impl TokenReply {
    fn answer(&self, token: Option<String>) {
        let reply = match self.0.lock() {
            Ok(mut reply) => reply.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if let Some(reply) = reply {
            reply(token);
        }
    }
}

impl Drop for TokenReply {
    fn drop(&mut self) {
        self.answer(None);
    }
}

impl<T: 'static> WindowState<T> {
    /// Ask the compositor for an activation token for `surface`, for an app it
    /// starts or asks to show something, and hand it to `reply`.
    ///
    /// The token is tied to the latest input on this seat (a click, key or
    /// touch), which is what makes it worth honouring, so ask in answer to
    /// one. `reply` gets `None` when the compositor has no xdg-activation or
    /// gives no token by the time it has read the request.
    pub fn request_activation_token(
        &mut self,
        surface: &WlSurface,
        reply: impl FnOnce(Option<String>) + Send + 'static,
    ) {
        self.mint_activation_token(surface, Box::new(reply));
    }

    /// [`Self::request_activation_token`], returning the token and the sync
    /// that bounds the wait for it.
    pub(crate) fn mint_activation_token(
        &mut self,
        surface: &WlSurface,
        reply: ActivationTokenReply,
    ) -> Option<(XdgActivationTokenV1, WlCallback)> {
        let request = TokenRequest::new(reply);
        let Some(activation) = self.xdg_activation.clone() else {
            log::debug!("xdg_activation_v1 unavailable — no activation token");
            request.answer(None);
            return None;
        };
        let Some(unit) = self.units.first() else {
            request.answer(None);
            return None;
        };
        let qh = unit.qh.clone();
        let display = unit.display.clone();

        let token = activation.get_activation_token(&qh, request.clone());
        if let (Some(seat), Some(serial)) = (self.seat.as_ref(), self.last_input_serial) {
            token.set_serial(serial, seat);
        }
        token.set_surface(surface);
        token.commit();
        let answered = display.sync(&qh, request);
        Some((token, answered))
    }

    /// Ask the compositor to activate `surface` with a `token` another client
    /// obtained, the way a single-instance app answers a second launch. The
    /// compositor decides from the token whether it may take focus.
    pub fn activate_surface_with_token(&self, surface: &WlSurface, token: String) {
        let Some(activation) = self.xdg_activation.as_ref() else {
            log::debug!("xdg_activation_v1 unavailable — cannot activate with a token");
            return;
        };
        activation.activate(token, surface);
    }
}

impl<T: 'static> Dispatch<XdgActivationTokenV1, TokenRequest> for WindowState<T> {
    fn event(
        _state: &mut Self,
        proxy: &XdgActivationTokenV1,
        event: <XdgActivationTokenV1 as Proxy>::Event,
        request: &TokenRequest,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            request.answer(Some(token));
            proxy.destroy();
        }
    }
}

impl<T: 'static> Dispatch<WlCallback, TokenRequest> for WindowState<T> {
    fn event(
        _state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        request: &TokenRequest,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            request.answer(None);
        }
    }
}
