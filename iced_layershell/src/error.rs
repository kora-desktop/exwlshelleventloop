use iced_futures::futures;

/// An error that occurred while running an application.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The futures executor could not be created.
    #[error("the futures executor could not be created")]
    ExecutorCreationFailed(futures::io::Error),

    /// The application window could not be created.
    #[error("the application window could not be created")]
    WindowCreationFailed(Box<dyn std::error::Error + Send + Sync>),

    /// The application graphics context could not be created.
    #[error("the application graphics context could not be created")]
    GraphicsCreationFailed(iced_graphics::Error),

    /// The event loop stopped, for one because the Wayland connection was lost.
    #[error("the event loop stopped: {0}")]
    EventLoopFailed(layershellev::LayerEventError),
}

impl From<iced_graphics::Error> for Error {
    fn from(error: iced_graphics::Error) -> Error {
        Error::GraphicsCreationFailed(error)
    }
}
