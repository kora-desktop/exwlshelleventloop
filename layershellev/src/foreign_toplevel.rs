//! Foreign toplevel management client
//!
//! This module provides client-side support for tracking opened application windows
//! using Wayland protocols. It supports:
//!
//! - `ext_foreign_toplevel_list_v1` (standard Wayland protocol, preferred)
//! - `zcosmic_toplevel_info_v1` (COSMIC extension for state info like minimized/maximized)
//! - `zcosmic_toplevel_manager_v1` (COSMIC extension for control - activate, close, etc.)
//! - `zwlr_foreign_toplevel_manager_v1` (wlroots fallback)
//!
//! Use this to create taskbars and docks that need to track open windows.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

// ext_foreign_toplevel_list_v1 protocol (standard, minimal)
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};

// COSMIC toplevel info extension (for state information)
#[cfg(feature = "cosmic-toplevel")]
use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1::{self, ZcosmicToplevelHandleV1},
    zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
};

// COSMIC toplevel management extension (for control - activate, close, etc.)
#[cfg(feature = "cosmic-toplevel")]
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::{
    self, ZcosmicToplevelManagerV1,
};

// wlroots fallback protocol
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Information about a foreign toplevel window
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToplevelInfo {
    /// Unique identifier for this toplevel (object ID)
    pub id: u32,
    /// Compositor-provided identity; absent with the wlroots fallback.
    pub identifier: Option<String>,
    /// Compositor-authenticated Kora workspace, matched to `identifier`.
    /// `None` means unknown; `Some("")` is the machine plane. Requires
    /// `cosmic-toplevel` and the Kora identity protocol on the compositor.
    pub workspace: Option<String>,
    /// The desktops the window is on, by `ext_workspace_handle_v1` protocol id
    /// (`WorkspaceInfo::id`). Empty unless COSMIC toplevel info v3 and the
    /// `workspaces` feature are both available.
    pub desktops: Vec<u32>,
    /// Window title
    pub title: String,
    /// Application ID (app_id)
    pub app_id: String,
    /// Whether the window is currently active/focused
    pub is_activated: bool,
    /// Whether the window is maximized
    pub is_maximized: bool,
    /// Whether the window is minimized
    pub is_minimized: bool,
    /// Whether the window is fullscreen
    pub is_fullscreen: bool,
}

impl ToplevelInfo {
    /// Get a display name for this toplevel
    pub fn display_name(&self) -> &str {
        if self.title.is_empty() {
            &self.app_id
        } else {
            &self.title
        }
    }
}

/// Events from the foreign toplevel manager
#[derive(Debug, Clone)]
pub enum ForeignToplevelEvent {
    /// A new toplevel was created
    Created(ToplevelInfo),
    /// A toplevel's info was updated (title, app_id, or state changed)
    Changed(ToplevelInfo),
    /// A toplevel was closed
    Closed(u32),
    /// The manager has finished (compositor no longer sending events)
    Finished,
}

/// Internal state for a toplevel handle
#[derive(Debug, Default, Clone)]
pub(crate) struct ToplevelHandleData {
    pub identifier: Option<String>,
    pub workspace: Option<String>,
    pub desktops: Vec<u32>,
    pub title: String,
    pub app_id: String,
    pub is_activated: bool,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
    /// Whether initial properties have been received (done event received)
    pub initialized: bool,
}

impl ToplevelHandleData {
    pub fn to_info(&self, id: u32) -> ToplevelInfo {
        ToplevelInfo {
            id,
            identifier: self.identifier.clone(),
            workspace: self.workspace.clone(),
            desktops: self.desktops.clone(),
            title: self.title.clone(),
            app_id: self.app_id.clone(),
            is_activated: self.is_activated,
            is_maximized: self.is_maximized,
            is_minimized: self.is_minimized,
            is_fullscreen: self.is_fullscreen,
        }
    }
}

/// User data for the manager - empty, events go through the handler trait
#[derive(Debug, Clone, Default)]
pub struct ForeignToplevelManagerData;

/// User data for toplevel handles - just tracks the object ID
#[derive(Debug, Clone, Default)]
pub struct ToplevelHandleUserData;

/// User data for COSMIC toplevel manager
#[cfg(feature = "cosmic-toplevel")]
#[derive(Debug, Clone, Default)]
pub struct CosmicToplevelManagerData;

/// Trait for handling foreign toplevel events
#[allow(private_interfaces)]
pub trait ForeignToplevelHandler {
    /// Called when a toplevel event occurs
    fn foreign_toplevel_event(&mut self, event: ForeignToplevelEvent);

    /// Get the pending handle data for a toplevel ID (internal use)
    fn get_toplevel_data(&mut self, id: u32) -> &mut ToplevelHandleData;

    /// Remove toplevel data (internal use)
    fn remove_toplevel_data(&mut self, id: u32);

    /// Store a toplevel handle for later use (internal use)
    fn store_toplevel_handle(&mut self, id: u32, handle: ZwlrForeignToplevelHandleV1);

    /// Remove a toplevel handle (internal use)
    fn remove_toplevel_handle(&mut self, id: u32);

    /// Get a stored toplevel handle (for sending commands)
    fn get_toplevel_handle(&self, id: u32) -> Option<&ZwlrForeignToplevelHandleV1>;

    /// Store an ext_foreign_toplevel_handle_v1 for later use (screencopy, etc.)
    fn store_ext_toplevel_handle(&mut self, id: u32, handle: ExtForeignToplevelHandleV1);

    /// Remove an ext_foreign_toplevel_handle_v1
    fn remove_ext_toplevel_handle(&mut self, id: u32);

    /// Get a stored ext_foreign_toplevel_handle_v1
    fn get_ext_toplevel_handle(&self, id: u32) -> Option<&ExtForeignToplevelHandleV1>;

    /// Store a COSMIC toplevel handle for later use (internal use)
    #[cfg(feature = "cosmic-toplevel")]
    fn store_cosmic_toplevel_handle(&mut self, id: u32, handle: ZcosmicToplevelHandleV1);

    /// Remove a COSMIC toplevel handle (internal use)
    #[cfg(feature = "cosmic-toplevel")]
    fn remove_cosmic_toplevel_handle(&mut self, id: u32);

    /// Get a stored COSMIC toplevel handle (for sending commands)
    #[cfg(feature = "cosmic-toplevel")]
    fn get_cosmic_toplevel_handle(&self, id: u32) -> Option<&ZcosmicToplevelHandleV1>;

    /// A desktop's handle and an output its group is on: what a move names.
    #[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
    fn desktop_target(
        &self,
        desktop: u32,
    ) -> Option<(
        &wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1,
        &wayland_client::protocol::wl_output::WlOutput,
    )>;

    /// Get the COSMIC toplevel manager (for control operations)
    #[cfg(feature = "cosmic-toplevel")]
    fn get_cosmic_toplevel_manager(&self) -> Option<&ZcosmicToplevelManagerV1>;

    /// Get the COSMIC toplevel info manager (for creating COSMIC handles from ext handles)
    #[cfg(feature = "cosmic-toplevel")]
    fn cosmic_toplevel_info(&self) -> Option<&ZcosmicToplevelInfoV1>;
}

/// Actions that can be performed on a toplevel window
#[derive(Debug, Clone)]
pub enum ToplevelAction {
    /// Activate/focus the window
    Activate(u32),
    /// Close the window (polite request, can be ignored by unresponsive apps)
    Close(u32),
    /// Force close the window with SIGKILL (for unresponsive apps)
    /// Requires COSMIC toplevel management protocol v5+
    #[cfg(feature = "cosmic-toplevel")]
    ForceClose(u32),
    /// Maximize the window
    SetMaximized(u32),
    /// Unmaximize the window
    UnsetMaximized(u32),
    /// Minimize the window
    SetMinimized(u32),
    /// Unminimize the window (activate is usually sufficient)
    UnsetMinimized(u32),
    /// Set fullscreen
    SetFullscreen(u32),
    /// Unset fullscreen
    UnsetFullscreen(u32),
    /// Move the window to a desktop, named by its `ext_workspace_handle_v1` id, on an
    /// output that desktop's group is on. Needs zcosmic_toplevel_manager_v1 v4+.
    #[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
    MoveToDesktop { toplevel: u32, desktop: u32 },
}

/// Blanket implementation for foreign toplevel manager dispatch
impl<D> Dispatch<ZwlrForeignToplevelManagerV1, ForeignToplevelManagerData, D> for ()
where
    D: Dispatch<ZwlrForeignToplevelManagerV1, ForeignToplevelManagerData>
        + Dispatch<ZwlrForeignToplevelHandleV1, ToplevelHandleUserData>
        + ForeignToplevelHandler
        + 'static,
{
    fn event(
        state: &mut D,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _data: &ForeignToplevelManagerData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                // Get the object ID for this toplevel handle
                let id = toplevel.id().protocol_id();
                log::info!(
                    "zwlr_foreign_toplevel: new toplevel handle, storing id={}",
                    id
                );
                // Initialize empty state for this toplevel
                let _ = state.get_toplevel_data(id);
                // Store the handle for later use (activate, close, etc.)
                state.store_toplevel_handle(id, toplevel);
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                state.foreign_toplevel_event(ForeignToplevelEvent::Finished);
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<D>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            // toplevel event (opcode 0)
            0 => qhandle.make_data::<ZwlrForeignToplevelHandleV1, _>(ToplevelHandleUserData),
            _ => panic!("Unknown opcode in event_created_child: {}", opcode),
        }
    }
}

/// Blanket implementation for toplevel handle dispatch
impl<D> Dispatch<ZwlrForeignToplevelHandleV1, ToplevelHandleUserData, D> for ()
where
    D: Dispatch<ZwlrForeignToplevelHandleV1, ToplevelHandleUserData> + ForeignToplevelHandler,
{
    fn event(
        state: &mut D,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _data: &ToplevelHandleUserData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        // Use the protocol ID as our identifier
        let id = proxy.id().protocol_id();

        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                state.get_toplevel_data(id).title = title;
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                state.get_toplevel_data(id).app_id = app_id;
            }
            zwlr_foreign_toplevel_handle_v1::Event::State {
                state: states_array,
            } => {
                let handle_data = state.get_toplevel_data(id);
                // Reset all states first
                handle_data.is_activated = false;
                handle_data.is_maximized = false;
                handle_data.is_minimized = false;
                handle_data.is_fullscreen = false;

                // Parse state array - it's an array of u32 values
                let bytes: &[u8] = states_array.as_ref();
                for chunk in bytes.chunks_exact(4) {
                    if let Ok(arr) = <[u8; 4]>::try_from(chunk) {
                        let value = u32::from_ne_bytes(arr);
                        match value {
                            0 => handle_data.is_maximized = true,
                            1 => handle_data.is_minimized = true,
                            2 => handle_data.is_activated = true,
                            3 => handle_data.is_fullscreen = true,
                            _ => {}
                        }
                    }
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                let handle_data = state.get_toplevel_data(id);
                let info = handle_data.to_info(id);

                if handle_data.initialized {
                    // Update existing toplevel
                    state.foreign_toplevel_event(ForeignToplevelEvent::Changed(info));
                } else {
                    // New toplevel - first done event
                    handle_data.initialized = true;
                    state.foreign_toplevel_event(ForeignToplevelEvent::Created(info));
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                let info = state.get_toplevel_data(id).to_info(id);
                state.remove_toplevel_data(id);
                state.remove_toplevel_handle(id);
                state.foreign_toplevel_event(ForeignToplevelEvent::Closed(info.id));
                // Destroy the handle
                proxy.destroy();
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { .. } => {
                // Could track which outputs the toplevel is on
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { .. } => {
                // Could track which outputs the toplevel is on
            }
            zwlr_foreign_toplevel_handle_v1::Event::Parent { .. } => {
                // Could track parent-child relationships
            }
            _ => {}
        }
    }
}

// ============================================================================
// ext_foreign_toplevel_list_v1 protocol (standard Wayland, preferred)
// ============================================================================

/// User data for ext_foreign_toplevel_list_v1
#[derive(Debug, Clone, Default)]
pub struct ExtForeignToplevelListData;

/// User data for ext_foreign_toplevel_handle_v1
#[derive(Debug, Clone, Default)]
pub struct ExtToplevelHandleData;

#[cfg(feature = "cosmic-toplevel")]
/// Dispatch for ext_foreign_toplevel_list_v1 WITH cosmic-toplevel feature
impl<D> Dispatch<ExtForeignToplevelListV1, ExtForeignToplevelListData, D> for ()
where
    D: Dispatch<ExtForeignToplevelListV1, ExtForeignToplevelListData>
        + Dispatch<ExtForeignToplevelHandleV1, ExtToplevelHandleData>
        + Dispatch<ZcosmicToplevelHandleV1, CosmicToplevelHandleData>
        + ForeignToplevelHandler
        + 'static,
{
    fn event(
        state: &mut D,
        _proxy: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &ExtForeignToplevelListData,
        _conn: &Connection,
        qhandle: &QueueHandle<D>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                let id = toplevel.id().protocol_id();
                log::trace!("ext_foreign_toplevel_list: new toplevel handle id={}", id);
                // Initialize empty state for this toplevel
                let _ = state.get_toplevel_data(id);

                // If we have the COSMIC toplevel info manager, create a COSMIC handle for control
                if let Some(cosmic_info) = state.cosmic_toplevel_info() {
                    let cosmic_handle = cosmic_info.get_cosmic_toplevel(
                        &toplevel,
                        qhandle,
                        CosmicToplevelHandleData { ext_handle_id: id },
                    );
                    log::debug!(
                        "Created COSMIC toplevel handle for ext handle id={}, cosmic_id={}",
                        id,
                        cosmic_handle.id().protocol_id()
                    );
                    state.store_cosmic_toplevel_handle(id, cosmic_handle);
                }

                // Store the ext handle for screencopy and other ext-protocol uses
                state.store_ext_toplevel_handle(id, toplevel);
            }
            ext_foreign_toplevel_list_v1::Event::Finished => {
                log::trace!("ext_foreign_toplevel_list: finished");
                state.foreign_toplevel_event(ForeignToplevelEvent::Finished);
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<D>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            // toplevel event (opcode 0)
            0 => qhandle.make_data::<ExtForeignToplevelHandleV1, _>(ExtToplevelHandleData),
            _ => panic!(
                "Unknown ext toplevel opcode in event_created_child: {}",
                opcode
            ),
        }
    }
}

#[cfg(not(feature = "cosmic-toplevel"))]
/// Dispatch for ext_foreign_toplevel_list_v1 WITHOUT cosmic-toplevel feature
impl<D> Dispatch<ExtForeignToplevelListV1, ExtForeignToplevelListData, D> for ()
where
    D: Dispatch<ExtForeignToplevelListV1, ExtForeignToplevelListData>
        + Dispatch<ExtForeignToplevelHandleV1, ExtToplevelHandleData>
        + ForeignToplevelHandler
        + 'static,
{
    fn event(
        state: &mut D,
        _proxy: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &ExtForeignToplevelListData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                let id = toplevel.id().protocol_id();
                log::trace!("ext_foreign_toplevel_list: new toplevel handle id={}", id);
                // Initialize empty state for this toplevel
                let _ = state.get_toplevel_data(id);

                // Store the ext handle for screencopy and other ext-protocol uses
                state.store_ext_toplevel_handle(id, toplevel);
            }
            ext_foreign_toplevel_list_v1::Event::Finished => {
                log::trace!("ext_foreign_toplevel_list: finished");
                state.foreign_toplevel_event(ForeignToplevelEvent::Finished);
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<D>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            // toplevel event (opcode 0)
            0 => qhandle.make_data::<ExtForeignToplevelHandleV1, _>(ExtToplevelHandleData),
            _ => panic!(
                "Unknown ext toplevel opcode in event_created_child: {}",
                opcode
            ),
        }
    }
}

/// Dispatch for ext_foreign_toplevel_handle_v1
impl<D> Dispatch<ExtForeignToplevelHandleV1, ExtToplevelHandleData, D> for ()
where
    D: Dispatch<ExtForeignToplevelHandleV1, ExtToplevelHandleData> + ForeignToplevelHandler,
{
    fn event(
        state: &mut D,
        proxy: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &ExtToplevelHandleData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        let id = proxy.id().protocol_id();

        match event {
            ext_foreign_toplevel_handle_v1::Event::Title { title } => {
                log::trace!("ext_foreign_toplevel_handle {}: title={}", id, title);
                state.get_toplevel_data(id).title = title;
            }
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                log::trace!("ext_foreign_toplevel_handle {}: app_id={}", id, app_id);
                state.get_toplevel_data(id).app_id = app_id;
            }
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                log::trace!(
                    "ext_foreign_toplevel_handle {}: identifier={}",
                    id,
                    identifier
                );
                state.get_toplevel_data(id).identifier = Some(identifier);
            }
            ext_foreign_toplevel_handle_v1::Event::Done => {
                let handle_data = state.get_toplevel_data(id);
                let info = handle_data.to_info(id);
                log::trace!(
                    "ext_foreign_toplevel_handle {}: done, title={}, app_id={}, initialized={}",
                    id,
                    info.title,
                    info.app_id,
                    handle_data.initialized
                );

                if handle_data.initialized {
                    state.foreign_toplevel_event(ForeignToplevelEvent::Changed(info));
                } else {
                    handle_data.initialized = true;
                    state.foreign_toplevel_event(ForeignToplevelEvent::Created(info));
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                let info = state.get_toplevel_data(id).to_info(id);
                state.remove_toplevel_data(id);
                state.remove_ext_toplevel_handle(id);
                state.foreign_toplevel_event(ForeignToplevelEvent::Closed(info.id));
                proxy.destroy();
            }
            _ => {}
        }
    }
}

// ============================================================================
// zcosmic_toplevel_info_v1 protocol (COSMIC extension for state info)
// ============================================================================

#[cfg(feature = "cosmic-toplevel")]
/// User data for zcosmic_toplevel_info_v1
#[derive(Debug, Clone, Default)]
pub struct CosmicToplevelInfoData;

#[cfg(feature = "cosmic-toplevel")]
/// User data for zcosmic_toplevel_handle_v1 - tracks the ext handle ID this extends
#[derive(Debug, Clone)]
pub struct CosmicToplevelHandleData {
    /// The ext_foreign_toplevel_handle_v1 protocol ID this cosmic handle extends
    pub ext_handle_id: u32,
}

#[cfg(feature = "cosmic-toplevel")]
/// Extended trait for COSMIC toplevel state management
pub trait CosmicToplevelHandler: ForeignToplevelHandler {
    /// Get the COSMIC toplevel info manager (for extending ext handles)
    fn cosmic_toplevel_info(&self) -> Option<&ZcosmicToplevelInfoV1>;

    /// Store mapping from cosmic handle ID to ext handle ID
    fn set_cosmic_handle_mapping(&mut self, cosmic_id: u32, ext_id: u32);

    /// Get the ext handle ID for a cosmic handle ID
    fn get_ext_handle_id(&self, cosmic_id: u32) -> Option<u32>;
}

#[cfg(feature = "cosmic-toplevel")]
/// Dispatch for zcosmic_toplevel_info_v1
impl<D> Dispatch<ZcosmicToplevelInfoV1, CosmicToplevelInfoData, D> for ()
where
    D: Dispatch<ZcosmicToplevelInfoV1, CosmicToplevelInfoData>
        + Dispatch<ZcosmicToplevelHandleV1, CosmicToplevelHandleData>
        + CosmicToplevelHandler
        + 'static,
{
    fn event(
        _state: &mut D,
        _proxy: &ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _data: &CosmicToplevelInfoData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        match event {
            zcosmic_toplevel_info_v1::Event::Finished => {
                log::debug!("COSMIC toplevel info finished");
            }
            _ => {}
        }
    }
}

#[cfg(feature = "cosmic-toplevel")]
/// Dispatch for zcosmic_toplevel_handle_v1
impl<D> Dispatch<ZcosmicToplevelHandleV1, CosmicToplevelHandleData, D> for ()
where
    D: Dispatch<ZcosmicToplevelHandleV1, CosmicToplevelHandleData> + CosmicToplevelHandler,
{
    fn event(
        state: &mut D,
        proxy: &ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        data: &CosmicToplevelHandleData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        // Get the ext handle ID that this cosmic handle extends
        let ext_id = data.ext_handle_id;

        match event {
            zcosmic_toplevel_handle_v1::Event::State {
                state: states_array,
            } => {
                let handle_data = state.get_toplevel_data(ext_id);
                // Reset all states first
                handle_data.is_activated = false;
                handle_data.is_maximized = false;
                handle_data.is_minimized = false;
                handle_data.is_fullscreen = false;

                // Parse state array - array of u32 values
                // COSMIC state enum: maximized=0, minimized=1, activated=2, fullscreen=3, sticky=4
                let bytes: &[u8] = states_array.as_ref();
                for chunk in bytes.chunks_exact(4) {
                    if let Ok(arr) = <[u8; 4]>::try_from(chunk) {
                        let value = u32::from_ne_bytes(arr);
                        match value {
                            0 => handle_data.is_maximized = true,
                            1 => handle_data.is_minimized = true,
                            2 => handle_data.is_activated = true,
                            3 => handle_data.is_fullscreen = true,
                            4 => { /* sticky - not tracked */ }
                            _ => {}
                        }
                    }
                }
            }
            zcosmic_toplevel_handle_v1::Event::OutputEnter { .. } => {
                // Could track which outputs the toplevel is on
            }
            zcosmic_toplevel_handle_v1::Event::OutputLeave { .. } => {
                // Could track which outputs the toplevel is on
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceEnter { workspace } => {
                let id = workspace.id().protocol_id();
                let desktops = &mut state.get_toplevel_data(ext_id).desktops;
                if !desktops.contains(&id) {
                    desktops.push(id);
                }
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceLeave { workspace } => {
                let id = workspace.id().protocol_id();
                state
                    .get_toplevel_data(ext_id)
                    .desktops
                    .retain(|d| *d != id);
            }
            zcosmic_toplevel_handle_v1::Event::Closed => {
                // COSMIC handle closed, destroy it
                proxy.destroy();
            }
            _ => {}
        }
    }
}

// ============================================================================
// COSMIC toplevel manager dispatch
// ============================================================================

/// Dispatch for COSMIC toplevel manager (control operations)
#[cfg(feature = "cosmic-toplevel")]
impl<D> Dispatch<ZcosmicToplevelManagerV1, CosmicToplevelManagerData, D> for ()
where
    D: Dispatch<ZcosmicToplevelManagerV1, CosmicToplevelManagerData>
        + ForeignToplevelHandler
        + 'static,
{
    fn event(
        _state: &mut D,
        _proxy: &ZcosmicToplevelManagerV1,
        event: zcosmic_toplevel_manager_v1::Event,
        _data: &CosmicToplevelManagerData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        match event {
            zcosmic_toplevel_manager_v1::Event::Capabilities { capabilities } => {
                log::debug!("COSMIC toplevel manager capabilities: {:?}", capabilities);
            }
            _ => {}
        }
    }
}

// ============================================================================
// Toplevel action execution
// ============================================================================

/// Execute a toplevel action on a stored handle
///
/// Returns true if the action was executed, false if the handle was not found.
/// Prefers COSMIC toplevel manager if available, falls back to wlr protocol.
pub fn execute_toplevel_action<D: ForeignToplevelHandler>(
    state: &D,
    action: ToplevelAction,
    seat: Option<&wayland_client::protocol::wl_seat::WlSeat>,
) -> bool {
    let id = match &action {
        ToplevelAction::Activate(id) => *id,
        ToplevelAction::Close(id) => *id,
        #[cfg(feature = "cosmic-toplevel")]
        ToplevelAction::ForceClose(id) => *id,
        ToplevelAction::SetMaximized(id) => *id,
        ToplevelAction::UnsetMaximized(id) => *id,
        ToplevelAction::SetMinimized(id) => *id,
        ToplevelAction::UnsetMinimized(id) => *id,
        ToplevelAction::SetFullscreen(id) => *id,
        ToplevelAction::UnsetFullscreen(id) => *id,
        #[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
        ToplevelAction::MoveToDesktop { toplevel, .. } => *toplevel,
    };

    log::info!("execute_toplevel_action: action={:?}, id={}", action, id);

    // The wlr protocol has no desktops, so a move has no fallback.
    #[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
    if let ToplevelAction::MoveToDesktop { toplevel, desktop } = action {
        return move_to_desktop(state, toplevel, desktop);
    }

    // Try COSMIC toplevel manager first (if available)
    #[cfg(feature = "cosmic-toplevel")]
    {
        if let Some(manager) = state.get_cosmic_toplevel_manager() {
            if let Some(handle) = state.get_cosmic_toplevel_handle(id) {
                log::info!("Using COSMIC toplevel manager for id {}", id);
                return execute_cosmic_action(manager, handle, action, seat);
            } else {
                log::debug!("No COSMIC handle for id {}, trying wlr fallback", id);
            }
        }
    }

    // Fall back to wlr protocol
    let Some(handle) = state.get_toplevel_handle(id) else {
        log::warn!("Toplevel handle not found for id {}", id);
        return false;
    };

    log::info!("Found wlr toplevel handle for id {}", id);
    execute_wlr_action(handle, action, seat)
}

/// Ask the compositor to move `toplevel` to `desktop`. False when the compositor
/// cannot, or when the window or the desktop is gone or not on an output.
#[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
fn move_to_desktop<D: ForeignToplevelHandler>(state: &D, toplevel: u32, desktop: u32) -> bool {
    use wayland_client::Proxy;
    let Some(manager) = state.get_cosmic_toplevel_manager() else {
        log::warn!("Move to desktop dropped: zcosmic_toplevel_manager_v1 not bound");
        return false;
    };
    if manager.version() < 4 {
        log::warn!(
            "Move to desktop dropped: zcosmic_toplevel_manager_v1 v{}, need v4+",
            manager.version()
        );
        return false;
    }
    let Some(handle) = state.get_cosmic_toplevel_handle(toplevel) else {
        log::warn!("Move to desktop dropped: no COSMIC handle for toplevel {toplevel}");
        return false;
    };
    let Some((workspace, output)) = state.desktop_target(desktop) else {
        log::warn!("Move to desktop dropped: desktop {desktop} is gone or not on an output");
        return false;
    };
    manager.move_to_ext_workspace(handle, workspace, output);
    true
}

/// Execute action using COSMIC toplevel manager
#[cfg(feature = "cosmic-toplevel")]
fn execute_cosmic_action(
    manager: &ZcosmicToplevelManagerV1,
    handle: &ZcosmicToplevelHandleV1,
    action: ToplevelAction,
    seat: Option<&wayland_client::protocol::wl_seat::WlSeat>,
) -> bool {
    match action {
        ToplevelAction::Activate(_) => {
            if let Some(seat) = seat {
                log::info!("COSMIC: Activating toplevel");
                manager.activate(handle, seat);
            } else {
                log::warn!("Cannot activate toplevel without a seat");
                return false;
            }
        }
        ToplevelAction::Close(_) => {
            log::info!("COSMIC: Closing toplevel");
            manager.close(handle);
        }
        ToplevelAction::ForceClose(_) => {
            // force_close requires protocol version 5+
            // Check manager version to avoid protocol errors on older compositors
            use wayland_client::Proxy;
            if manager.version() >= 5 {
                log::info!(
                    "COSMIC: Force closing toplevel with SIGKILL (protocol v{})",
                    manager.version()
                );
                manager.force_close(handle);
            } else {
                log::warn!(
                    "COSMIC: force_close not supported (compositor v{}, need v5+), falling back to regular close",
                    manager.version()
                );
                manager.close(handle);
            }
        }
        ToplevelAction::SetMaximized(_) => {
            log::info!("COSMIC: Setting maximized");
            manager.set_maximized(handle);
        }
        ToplevelAction::UnsetMaximized(_) => {
            log::info!("COSMIC: Unsetting maximized");
            manager.unset_maximized(handle);
        }
        ToplevelAction::SetMinimized(_) => {
            log::info!("COSMIC: Setting minimized");
            manager.set_minimized(handle);
        }
        ToplevelAction::UnsetMinimized(_) => {
            log::info!("COSMIC: Unsetting minimized");
            manager.unset_minimized(handle);
        }
        ToplevelAction::SetFullscreen(_) => {
            log::info!("COSMIC: Setting fullscreen");
            manager.set_fullscreen(handle, None);
        }
        ToplevelAction::UnsetFullscreen(_) => {
            log::info!("COSMIC: Unsetting fullscreen");
            manager.unset_fullscreen(handle);
        }
        // Sent by `move_to_desktop`, which needs the desktop's handle and output.
        #[cfg(feature = "workspaces")]
        ToplevelAction::MoveToDesktop { .. } => return false,
    }
    true
}

/// Execute action using wlr foreign toplevel protocol
fn execute_wlr_action(
    handle: &ZwlrForeignToplevelHandleV1,
    action: ToplevelAction,
    seat: Option<&wayland_client::protocol::wl_seat::WlSeat>,
) -> bool {
    match action {
        ToplevelAction::Activate(_) => {
            if let Some(seat) = seat {
                log::info!("wlr: Activating toplevel");
                handle.activate(seat);
            } else {
                log::warn!("Cannot activate toplevel without a seat");
                return false;
            }
        }
        ToplevelAction::Close(_) => {
            handle.close();
        }
        #[cfg(feature = "cosmic-toplevel")]
        ToplevelAction::ForceClose(_) => {
            // wlr protocol doesn't support force close, fall back to regular close
            log::warn!("wlr protocol doesn't support force_close, using regular close");
            handle.close();
        }
        ToplevelAction::SetMaximized(_) => {
            handle.set_maximized();
        }
        ToplevelAction::UnsetMaximized(_) => {
            handle.unset_maximized();
        }
        ToplevelAction::SetMinimized(_) => {
            handle.set_minimized();
        }
        ToplevelAction::UnsetMinimized(_) => {
            handle.unset_minimized();
        }
        ToplevelAction::SetFullscreen(_) => {
            handle.set_fullscreen(None);
        }
        ToplevelAction::UnsetFullscreen(_) => {
            handle.unset_fullscreen();
        }
        #[cfg(all(feature = "cosmic-toplevel", feature = "workspaces"))]
        ToplevelAction::MoveToDesktop { .. } => {
            log::warn!("wlr protocol has no desktops; move dropped");
            return false;
        }
    }

    true
}
