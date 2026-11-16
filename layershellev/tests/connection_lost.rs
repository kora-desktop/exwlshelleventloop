//! A client whose Wayland connection dies must leave its event loop at once, with an
//! error, instead of spinning on the closed socket and logging the error forever.
//!
//! The test runs a fake compositor on its own socket and starts this binary again as the
//! client (`CHILD` set), so it can time the client's exit and count what it printed.

use layershellev::{LayerShellEvent, ReturnData, StartMode, WindowState};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use wayland_protocols::xdg::shell::server::xdg_wm_base::XdgWmBase;
use wayland_protocols::xdg::xdg_output::zv1::server::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols_wlr::layer_shell::v1::server::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use wayland_server::protocol::{
    wl_compositor::{self, WlCompositor},
    wl_region::WlRegion,
    wl_seat::WlSeat,
    wl_shm::WlShm,
    wl_surface::WlSurface,
};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, ListeningSocket, New,
    Resource,
};

const CHILD: &str = "LAYERSHELLEV_CONNECTION_LOST_CHILD";
const RUNNING: &str = "client: event loop running";
const REALM_ERROR: &str = "a client in workspace work cannot assign surfaces to workspaces";
/// Enough of the client's stderr to count lines without ever holding an unbounded log.
const READ_CAP: u64 = 1 << 20;

#[derive(Default)]
struct Compositor {
    bound: Vec<WlCompositor>,
}

struct NoClientData;
impl ClientData for NoClientData {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

/// Globals the event loop binds, with nothing behind them.
macro_rules! plain_global {
    ($($interface:ty),*) => {$(
        impl GlobalDispatch<$interface, ()> for Compositor {
            fn bind(
                _: &mut Self,
                _: &DisplayHandle,
                _: &Client,
                resource: New<$interface>,
                _: &(),
                data_init: &mut DataInit<'_, Self>,
            ) {
                data_init.init(resource, ());
            }
        }
        impl Dispatch<$interface, ()> for Compositor {
            fn request(
                _: &mut Self,
                _: &Client,
                _: &$interface,
                _: <$interface as Resource>::Request,
                _: &(),
                _: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
            }
        }
    )*};
}
plain_global!(
    WlShm,
    WlSeat,
    XdgWmBase,
    ZxdgOutputManagerV1,
    ZwlrLayerShellV1
);

impl Dispatch<WlSurface, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlSurface,
        _: <WlSurface as Resource>::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<WlRegion, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlRegion,
        _: <WlRegion as Resource>::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl GlobalDispatch<WlCompositor, ()> for Compositor {
    fn bind(
        state: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlCompositor>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        state.bound.push(data_init.init(resource, ()));
    }
}

impl Dispatch<WlCompositor, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                data_init.init(id, ());
            }
            wl_compositor::Request::CreateRegion { id } => {
                data_init.init(id, ());
            }
            _ => {}
        }
    }
}

/// How the fake compositor ends the client's connection once its loop runs.
#[derive(Clone, Copy, Debug)]
enum Ending {
    /// A protocol error, as cosmic-comp's realm manager sends one.
    ProtocolError,
    /// The compositor goes away without a word.
    Hangup,
}

struct Outcome {
    exited_within: Option<Duration>,
    success: bool,
    lines: Vec<String>,
}

fn run(ending: Ending) -> Outcome {
    let runtime = tempfile::tempdir().unwrap();
    let socket = runtime.path().join("wayland-connection-lost");
    let listening = ListeningSocket::bind_absolute(socket.clone()).unwrap();
    let display = Display::<Compositor>::new().unwrap();
    {
        // Not kept: a live handle would keep the client connections open after the display goes.
        let handle = display.handle();
        handle.create_global::<Compositor, WlCompositor, ()>(5, ());
        handle.create_global::<Compositor, WlShm, ()>(1, ());
        handle.create_global::<Compositor, WlSeat, ()>(7, ());
        handle.create_global::<Compositor, XdgWmBase, ()>(6, ());
        handle.create_global::<Compositor, ZxdgOutputManagerV1, ()>(3, ());
        handle.create_global::<Compositor, ZwlrLayerShellV1, ()>(4, ());
    }
    let mut state = Compositor::default();

    // Only this test's socket: the client never sees the session's compositor.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env("WAYLAND_DISPLAY", &socket)
        .env_remove("WAYLAND_SOCKET")
        .env("RUST_LOG", "error")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let lines = read_lines(&mut child);

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut ended = None;
    let mut display = Some(display);
    let mut status = None;
    while status.is_none() {
        if let Some(display) = display.as_mut() {
            if let Some(stream) = listening.accept().unwrap() {
                display
                    .handle()
                    .insert_client(stream, Arc::new(NoClientData))
                    .unwrap();
            }
            display.dispatch_clients(&mut state).unwrap();
            display.flush_clients().unwrap();
        }
        if ended.is_none() && lines.running.try_recv().is_ok() {
            match ending {
                Ending::ProtocolError => {
                    state.bound[0].post_error(0u32, REALM_ERROR);
                    display.as_mut().unwrap().flush_clients().unwrap();
                    // As cosmic-comp does, close the connection after the error.
                    display = None;
                }
                Ending::Hangup => display = None,
            }
            ended = Some(Instant::now());
        }
        status = child.try_wait().unwrap();
        let waited_out = match ended {
            Some(at) => at.elapsed() > Duration::from_secs(1),
            None => Instant::now() > deadline,
        };
        if status.is_none() && waited_out {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let exited_within = status.and(ended.map(|at| at.elapsed()));
    if status.is_none() {
        child.kill().unwrap();
    }
    let status = child.wait().unwrap();
    assert!(ended.is_some(), "the client's event loop never started");
    Outcome {
        exited_within,
        success: status.success(),
        lines: lines.all.recv().unwrap(),
    }
}

struct Lines {
    running: Receiver<()>,
    all: Receiver<Vec<String>>,
}

/// The client's stderr up to `READ_CAP`, with a signal once its loop runs. The rest is
/// drained unread, so a client that floods it is still running when the test looks.
fn read_lines(child: &mut Child) -> Lines {
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let (running_sender, running) = mpsc::channel();
    let (all_sender, all) = mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = Vec::new();
        for line in (&mut stderr).take(READ_CAP).lines() {
            let Ok(line) = line else { break };
            if line.contains(RUNNING) {
                let _ = running_sender.send(());
            }
            lines.push(line);
        }
        let _ = all_sender.send(lines);
        let _ = std::io::copy(&mut stderr, &mut std::io::sink());
    });
    Lines { running, all }
}

fn check(ending: Ending, cause: &str) {
    let outcome = run(ending);
    let after: Vec<&String> = outcome
        .lines
        .iter()
        .skip_while(|line| !line.contains(RUNNING))
        .skip(1)
        .collect();
    let within = outcome.exited_within.unwrap_or_else(|| {
        panic!(
            "{ending:?}: the client was still running a second later and printed {} lines: {:?}",
            after.len(),
            &after[..after.len().min(8)]
        )
    });
    assert!(
        within < Duration::from_secs(1),
        "{ending:?}: took {within:?}"
    );
    assert!(
        !outcome.success,
        "{ending:?}: the client exited with success"
    );
    assert!(
        after.len() <= 6,
        "{ending:?}: the client printed {} lines: {after:?}",
        after.len()
    );
    assert!(
        after.iter().any(|line| line.contains(cause)),
        "{ending:?}: no line names {cause:?}: {after:?}"
    );
    println!("{ending:?}: exited after {within:?}, printing {after:#?}");
}

/// The client: an event loop with no surface of its own, as kora-notifications starts.
fn child() -> ! {
    let state = WindowState::<()>::new("connection-lost-test")
        .with_start_mode(StartMode::Background)
        .build()
        .expect("the client could not reach the fake compositor");
    let result = state.running(|event, _, _| {
        if matches!(event, LayerShellEvent::InitRequest) {
            eprintln!("{RUNNING}");
        }
        ReturnData::None
    });
    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("client: {error}");
            std::process::exit(1)
        }
    }
}

fn main() {
    if std::env::var_os(CHILD).is_some() {
        child();
    }
    check(Ending::ProtocolError, "wl_compositor");
    println!("test protocol_error_ends_the_loop ... ok");
    check(Ending::Hangup, "connection was lost");
    println!("test hangup_ends_the_loop ... ok");
}
