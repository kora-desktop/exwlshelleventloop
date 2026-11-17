use iced_core::clipboard::{Content, Error, Kind};
use layershellev::WindowWrapper;

pub struct LayerShellClipboard {
    state: State,
}

enum State {
    Connected(window_clipboard::Clipboard),
    Unavailable,
}

impl LayerShellClipboard {
    /// Creates a new [`Clipboard`] for the given window.
    pub fn connect(window: &WindowWrapper) -> Self {
        #[allow(unsafe_code)]
        let state = unsafe { window_clipboard::Clipboard::connect(window) }
            .ok()
            .map(State::Connected)
            .unwrap_or(State::Unavailable);

        Self { state }
    }

    /// Creates a new [`Clipboard`] that isn't associated with a window.
    /// This clipboard will never contain a copied value.
    #[allow(unused)]
    pub fn unconnected() -> Self {
        Self {
            state: State::Unavailable,
        }
    }

    /// Whether it speaks to the compositor's clipboard.
    pub fn is_connected(&self) -> bool {
        matches!(self.state, State::Connected(_))
    }

    /// Reads the current content of the [`Clipboard`] synchronously.
    pub fn read_sync(&self, kind: Kind) -> Result<Content, Error> {
        match &self.state {
            State::Connected(clipboard) => match kind {
                Kind::Text => clipboard
                    .read()
                    .map(Content::Text)
                    .map_err(|_| Error::ContentNotAvailable),
                _ => Err(Error::ContentNotAvailable),
            },
            State::Unavailable => Err(Error::ClipboardUnavailable),
        }
    }

    /// Writes the given content to the [`Clipboard`] synchronously.
    pub fn write_sync(&mut self, content: Content) -> Result<(), Error> {
        match &mut self.state {
            State::Connected(clipboard) => match content {
                Content::Text(text) => clipboard.write(text).map_err(|e| {
                    log::warn!("error writing to clipboard: {e}");
                    Error::ContentNotAvailable
                }),
                _ => Err(Error::ContentNotAvailable),
            },
            State::Unavailable => Err(Error::ClipboardUnavailable),
        }
    }

    /// Writes the given content to the [`Clipboard`].
    pub fn write(
        &mut self,
        content: Content,
        callback: impl FnOnce(Result<(), Error>) + Send + 'static,
    ) {
        match &mut self.state {
            State::Connected(clipboard) => {
                let result = match content {
                    Content::Text(text) => clipboard.write(text).map_err(|e| {
                        log::warn!("error writing to clipboard: {e}");
                        Error::ContentNotAvailable
                    }),
                    _ => Err(Error::ContentNotAvailable),
                };
                callback(result);
            }
            State::Unavailable => {
                callback(Err(Error::ClipboardUnavailable));
            }
        }
    }

    /// Reads the primary selection synchronously.
    ///
    /// The primary selection is text-only, so unlike [`read_sync`] there is no
    /// [`Kind`] to choose. A `None` from `window_clipboard` means the seat
    /// offers no primary selection — nothing to paste, not a broken clipboard.
    ///
    /// [`read_sync`]: Self::read_sync
    pub fn read_primary_sync(&self) -> Result<String, Error> {
        match &self.state {
            State::Connected(clipboard) => match clipboard.read_primary() {
                Some(result) => result.map_err(|_| Error::ContentNotAvailable),
                None => Err(Error::ContentNotAvailable),
            },
            State::Unavailable => Err(Error::ClipboardUnavailable),
        }
    }

    /// Reads the primary selection.
    pub fn read_primary(&self, callback: impl FnOnce(Result<String, Error>) + Send + 'static) {
        callback(self.read_primary_sync());
    }

    /// Publishes the given text as the primary selection synchronously.
    ///
    /// Does not touch the clipboard: the two are separate selections, so
    /// copy-on-select must not overwrite what the user last copied.
    pub fn write_primary_sync(&mut self, text: String) -> Result<(), Error> {
        match &mut self.state {
            State::Connected(clipboard) => match clipboard.write_primary(text) {
                Some(result) => result.map_err(|e| {
                    log::warn!("error writing to primary selection: {e}");
                    Error::ContentNotAvailable
                }),
                None => Err(Error::ClipboardUnavailable),
            },
            State::Unavailable => Err(Error::ClipboardUnavailable),
        }
    }

    /// Publishes the given text as the primary selection.
    pub fn write_primary(
        &mut self,
        text: String,
        callback: impl FnOnce(Result<(), Error>) + Send + 'static,
    ) {
        callback(self.write_primary_sync(text));
    }
}
