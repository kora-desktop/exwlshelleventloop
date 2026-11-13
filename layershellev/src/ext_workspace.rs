//! The desktops of every Kora workspace: `ext_workspace_v1` for the desktops
//! themselves, plus `kora_workspace_realm_v1` for which workspace each group
//! of them belongs to.

use std::collections::HashMap;

use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, protocol::wl_output::WlOutput,
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1, GroupCapabilities},
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1, State, WorkspaceCapabilities},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};

use crate::{DispatchMessageInner, WindowState};

pub use realm::kora_workspace_realm_manager_v1;
use realm::kora_workspace_realm_manager_v1::KoraWorkspaceRealmManagerV1;

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    unused_imports
)]
mod realm {
    use wayland_client;
    use wayland_client::protocol::*;
    use wayland_protocols::ext::workspace::v1::client::*;

    pub mod __interfaces {
        use wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        use wayland_protocols::ext::workspace::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/kora-workspace-realm.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/kora-workspace-realm.xml");
}

/// One group of desktops: a Kora workspace on one output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGroupInfo {
    /// The group object's protocol id.
    pub id: u32,
    /// The Kora workspace registry id, once the compositor has said.
    pub realm: Option<String>,
    /// Whether the group is on an output right now (the active workspace's are).
    pub on_screen: bool,
    /// The outputs the group is on, by `xdg_output` name, in the order they entered.
    /// A name the compositor has not sent yet is left out until it arrives.
    pub outputs: Vec<String>,
    pub can_create: bool,
}

/// One desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    /// The handle's protocol id; what a capture or an action names.
    pub id: u32,
    pub group: Option<u32>,
    pub name: String,
    pub ext_id: Option<String>,
    /// Position within the group, in the protocol's own axes.
    pub coordinates: Vec<u32>,
    pub active: bool,
    pub urgent: bool,
    pub hidden: bool,
    pub can_activate: bool,
    pub can_remove: bool,
}

/// Everything known, sent whole after every batch of changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspacesSnapshot {
    pub groups: Vec<WorkspaceGroupInfo>,
    pub workspaces: Vec<WorkspaceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    Changed(WorkspacesSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceAction {
    /// Show this desktop.
    Activate(u32),
    /// Add a desktop to a group.
    Create { group: u32, name: String },
    /// Remove a desktop.
    Remove(u32),
    /// Move a desktop into another group, such as the same workspace's group on another output.
    Assign { workspace: u32, group: u32 },
    /// Several actions, committed together so the compositor applies them as one change.
    Batch(Vec<WorkspaceAction>),
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceManagerData;
#[derive(Debug, Clone, Default)]
pub struct WorkspaceGroupData;
#[derive(Debug, Clone, Default)]
pub struct WorkspaceHandleData;
#[derive(Debug, Clone, Default)]
pub struct RealmData;

#[derive(Debug, Default)]
pub(crate) struct WorkspacesState {
    pub manager: Option<ExtWorkspaceManagerV1>,
    pub realm_manager: Option<KoraWorkspaceRealmManagerV1>,
    groups: HashMap<u32, (ExtWorkspaceGroupHandleV1, WorkspaceGroupInfo)>,
    handles: HashMap<u32, (ExtWorkspaceHandleV1, WorkspaceInfo)>,
    /// The outputs each group is on, by group id.
    outputs: HashMap<u32, Vec<WlOutput>>,
}

impl WorkspacesState {
    /// Everything known, each group's outputs named by `name_of`.
    pub fn snapshot(&self, name_of: impl Fn(&WlOutput) -> Option<String>) -> WorkspacesSnapshot {
        let mut groups: Vec<_> = self
            .groups
            .values()
            .map(|(_, info)| WorkspaceGroupInfo {
                outputs: self
                    .outputs
                    .get(&info.id)
                    .into_iter()
                    .flatten()
                    .filter_map(&name_of)
                    .collect(),
                ..info.clone()
            })
            .collect();
        groups.sort_by_key(|g| g.id);
        let mut workspaces: Vec<_> = self
            .handles
            .values()
            .map(|(_, info)| info.clone())
            .collect();
        workspaces.sort_by_key(|w| w.id);
        WorkspacesSnapshot { groups, workspaces }
    }

    pub fn handle(&self, id: u32) -> Option<&ExtWorkspaceHandleV1> {
        self.handles.get(&id).map(|(handle, _)| handle)
    }

    /// A desktop's handle and the first output its group is on, which is what a
    /// window move names. None while the desktop has no group or the group no output.
    pub fn move_target(&self, id: u32) -> Option<(&ExtWorkspaceHandleV1, &WlOutput)> {
        let (handle, info) = self.handles.get(&id)?;
        let output = self.outputs.get(&info.group?)?.first()?;
        Some((handle, output))
    }
}

impl<T> WindowState<T> {
    pub(crate) fn emit_workspaces(&mut self) {
        let snapshot = self.workspaces.snapshot(|output| self.output_name(output));
        self.message.push((
            None,
            DispatchMessageInner::Workspaces(WorkspaceEvent::Changed(snapshot)),
        ));
    }

    /// Carry out `action`, committing it in the same round trip.
    pub fn execute_workspace_action(&mut self, action: WorkspaceAction) -> bool {
        let Some(manager) = self.workspaces.manager.as_ref() else {
            log::warn!("Workspace action dropped: ext_workspace_manager_v1 not bound");
            return false;
        };
        let queued = self.workspaces.queue(action);
        manager.commit();
        queued
    }
}

impl WorkspacesState {
    /// Send `action`'s requests, uncommitted. False if anything it names is gone.
    fn queue(&self, action: WorkspaceAction) -> bool {
        match action {
            WorkspaceAction::Activate(id) => {
                let Some(handle) = self.handle(id) else {
                    return false;
                };
                handle.activate();
            }
            WorkspaceAction::Create { group, name } => {
                let Some((group, _)) = self.groups.get(&group) else {
                    return false;
                };
                group.create_workspace(name);
            }
            WorkspaceAction::Remove(id) => {
                let Some(handle) = self.handle(id) else {
                    return false;
                };
                handle.remove();
            }
            WorkspaceAction::Assign { workspace, group } => {
                let (Some(handle), Some((group, _))) =
                    (self.handle(workspace), self.groups.get(&group))
                else {
                    return false;
                };
                handle.assign(group);
            }
            WorkspaceAction::Batch(actions) => {
                // Every action is sent, even after one that names something gone.
                let mut all = true;
                for action in actions {
                    all &= self.queue(action);
                }
                return all;
            }
        }
        true
    }
}

impl<T: 'static> Dispatch<ExtWorkspaceManagerV1, WorkspaceManagerData> for WindowState<T> {
    fn event(
        state: &mut Self,
        _proxy: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _data: &WorkspaceManagerData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
                let id = workspace_group.id().protocol_id();
                let info = WorkspaceGroupInfo {
                    id,
                    realm: None,
                    on_screen: false,
                    outputs: Vec::new(),
                    can_create: false,
                };
                state.workspaces.groups.insert(id, (workspace_group, info));
            }
            ext_workspace_manager_v1::Event::Workspace { workspace } => {
                let id = workspace.id().protocol_id();
                let info = WorkspaceInfo {
                    id,
                    group: None,
                    name: String::new(),
                    ext_id: None,
                    coordinates: Vec::new(),
                    active: false,
                    urgent: false,
                    hidden: false,
                    can_activate: false,
                    can_remove: false,
                };
                state.workspaces.handles.insert(id, (workspace, info));
            }
            ext_workspace_manager_v1::Event::Done => state.emit_workspaces(),
            ext_workspace_manager_v1::Event::Finished => {
                state.workspaces.groups.clear();
                state.workspaces.handles.clear();
                state.workspaces.manager = None;
                state.emit_workspaces();
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            0 => qh.make_data::<ExtWorkspaceGroupHandleV1, _>(WorkspaceGroupData),
            1 => qh.make_data::<ExtWorkspaceHandleV1, _>(WorkspaceHandleData),
            _ => panic!("Unknown ext_workspace_manager_v1 opcode {opcode}"),
        }
    }
}

impl<T: 'static> Dispatch<ExtWorkspaceGroupHandleV1, WorkspaceGroupData> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _data: &WorkspaceGroupData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let id = proxy.id().protocol_id();
        match event {
            ext_workspace_group_handle_v1::Event::Capabilities { capabilities } => {
                if let Some((_, info)) = state.workspaces.groups.get_mut(&id) {
                    info.can_create = matches!(capabilities, WEnum::Value(c) if c.contains(GroupCapabilities::CreateWorkspace));
                }
            }
            ext_workspace_group_handle_v1::Event::OutputEnter { output } => {
                let outputs = state.workspaces.outputs.entry(id).or_default();
                if !outputs.contains(&output) {
                    outputs.push(output);
                }
                if let Some((_, info)) = state.workspaces.groups.get_mut(&id) {
                    info.on_screen = true;
                }
            }
            ext_workspace_group_handle_v1::Event::OutputLeave { output } => {
                let outputs = state.workspaces.outputs.entry(id).or_default();
                outputs.retain(|o| *o != output);
                let on_screen = !outputs.is_empty();
                if let Some((_, info)) = state.workspaces.groups.get_mut(&id) {
                    info.on_screen = on_screen;
                }
            }
            ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
                let wid = workspace.id().protocol_id();
                if let Some((_, info)) = state.workspaces.handles.get_mut(&wid) {
                    info.group = Some(id);
                }
            }
            ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
                let wid = workspace.id().protocol_id();
                if let Some((_, info)) = state.workspaces.handles.get_mut(&wid) {
                    if info.group == Some(id) {
                        info.group = None;
                    }
                }
            }
            ext_workspace_group_handle_v1::Event::Removed => {
                if let Some((group, _)) = state.workspaces.groups.remove(&id) {
                    group.destroy();
                }
                state.workspaces.outputs.remove(&id);
            }
            _ => {}
        }
    }
}

impl<T: 'static> Dispatch<ExtWorkspaceHandleV1, WorkspaceHandleData> for WindowState<T> {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _data: &WorkspaceHandleData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let id = proxy.id().protocol_id();
        let Some((_, info)) = state.workspaces.handles.get_mut(&id) else {
            return;
        };
        match event {
            ext_workspace_handle_v1::Event::Id { id } => info.ext_id = Some(id),
            ext_workspace_handle_v1::Event::Name { name } => info.name = name,
            ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
                info.coordinates = coordinates
                    .chunks_exact(4)
                    .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
            }
            ext_workspace_handle_v1::Event::State { state: bits } => {
                let bits = match bits {
                    WEnum::Value(s) => s,
                    WEnum::Unknown(_) => State::empty(),
                };
                info.active = bits.contains(State::Active);
                info.urgent = bits.contains(State::Urgent);
                info.hidden = bits.contains(State::Hidden);
            }
            ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
                let caps = match capabilities {
                    WEnum::Value(c) => c,
                    WEnum::Unknown(_) => WorkspaceCapabilities::empty(),
                };
                info.can_activate = caps.contains(WorkspaceCapabilities::Activate);
                info.can_remove = caps.contains(WorkspaceCapabilities::Remove);
            }
            ext_workspace_handle_v1::Event::Removed => {
                if let Some((handle, _)) = state.workspaces.handles.remove(&id) {
                    handle.destroy();
                }
            }
            _ => {}
        }
    }
}

impl<T: 'static> Dispatch<KoraWorkspaceRealmManagerV1, RealmData> for WindowState<T> {
    fn event(
        state: &mut Self,
        _proxy: &KoraWorkspaceRealmManagerV1,
        event: kora_workspace_realm_manager_v1::Event,
        _data: &RealmData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let kora_workspace_realm_manager_v1::Event::Realm { group, id } = event;
        let gid = group.id().protocol_id();
        if let Some((_, info)) = state.workspaces.groups.get_mut(&gid) {
            info.realm = Some(id);
        }
        // Not batched under the workspace protocol's `done`, so it is its own change.
        state.emit_workspaces();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ZxdgOutputInfo;
    use std::os::unix::net::UnixStream;
    use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;

    #[test]
    fn a_group_names_the_outputs_it_is_on_as_they_come_and_go() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(socket).unwrap();
        let queue = connection.new_event_queue::<WindowState<()>>();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let built_in = registry.bind::<WlOutput, _, _>(1, 4, &qh, ());
        let external = registry.bind::<WlOutput, _, _>(2, 4, &qh, ());
        let unnamed = registry.bind::<WlOutput, _, _>(3, 4, &qh, ());
        let mut state = WindowState::<()>::new("workspace-outputs-test");
        for (output, name) in [(&built_in, "eDP-1"), (&external, "DP-1"), (&unnamed, "")] {
            let mut info =
                ZxdgOutputInfo::new(ZxdgOutputV1::inert(connection.backend().downgrade()));
            info.name = name.into();
            state.output_watch.push((output.clone(), info));
        }
        let group = ExtWorkspaceGroupHandleV1::inert(connection.backend().downgrade());
        let id = group.id().protocol_id();
        state.workspaces.groups.insert(
            id,
            (
                group.clone(),
                WorkspaceGroupInfo {
                    id,
                    realm: None,
                    on_screen: false,
                    outputs: Vec::new(),
                    can_create: false,
                },
            ),
        );
        let send = |state: &mut WindowState<()>, event| {
            <WindowState<()> as Dispatch<ExtWorkspaceGroupHandleV1, WorkspaceGroupData>>::event(
                state,
                &group,
                event,
                &WorkspaceGroupData,
                &connection,
                &qh,
            );
        };
        let snapshot = |state: &WindowState<()>| {
            let groups = state.workspaces.snapshot(|o| state.output_name(o)).groups;
            (groups[0].outputs.clone(), groups[0].on_screen)
        };

        for output in [&external, &built_in, &unnamed] {
            send(
                &mut state,
                ext_workspace_group_handle_v1::Event::OutputEnter {
                    output: output.clone(),
                },
            );
        }
        // In the order they entered; one without a name yet is left out.
        assert_eq!(
            snapshot(&state),
            (vec!["DP-1".into(), "eDP-1".into()], true)
        );

        send(
            &mut state,
            ext_workspace_group_handle_v1::Event::OutputLeave {
                output: external.clone(),
            },
        );
        assert_eq!(snapshot(&state), (vec!["eDP-1".into()], true));

        for output in [&built_in, &unnamed] {
            send(
                &mut state,
                ext_workspace_group_handle_v1::Event::OutputLeave {
                    output: output.clone(),
                },
            );
        }
        assert_eq!(snapshot(&state), (Vec::<String>::new(), false));
    }

    #[test]
    fn a_batch_naming_a_desktop_that_is_gone_says_so() {
        let state = WorkspacesState::default();
        assert!(!state.queue(WorkspaceAction::Batch(vec![
            WorkspaceAction::Activate(7),
            WorkspaceAction::Assign {
                workspace: 7,
                group: 3,
            },
        ])));
        assert!(state.queue(WorkspaceAction::Batch(Vec::new())));
    }

    /// A group on `outputs`, holding one desktop; returns the desktop's id.
    fn desktop_on(
        state: &mut WorkspacesState,
        connection: &Connection,
        group: u32,
        desktop: u32,
        outputs: Vec<WlOutput>,
    ) -> u32 {
        let backend = connection.backend().downgrade();
        state.groups.insert(
            group,
            (
                ExtWorkspaceGroupHandleV1::inert(backend.clone()),
                WorkspaceGroupInfo {
                    id: group,
                    realm: None,
                    on_screen: !outputs.is_empty(),
                    outputs: Vec::new(),
                    can_create: false,
                },
            ),
        );
        state.outputs.insert(group, outputs);
        state.handles.insert(
            desktop,
            (
                ExtWorkspaceHandleV1::inert(backend),
                WorkspaceInfo {
                    id: desktop,
                    group: Some(group),
                    name: String::new(),
                    ext_id: None,
                    coordinates: Vec::new(),
                    active: false,
                    urgent: false,
                    hidden: false,
                    can_activate: true,
                    can_remove: false,
                },
            ),
        );
        desktop
    }

    #[test]
    fn a_move_names_the_desktop_and_the_first_output_its_group_is_on() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(socket).unwrap();
        let queue = connection.new_event_queue::<WindowState<()>>();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let external = registry.bind::<WlOutput, _, _>(1, 4, &qh, ());
        let built_in = registry.bind::<WlOutput, _, _>(2, 4, &qh, ());
        let mut state = WorkspacesState::default();

        let shown = desktop_on(
            &mut state,
            &connection,
            1,
            10,
            vec![external.clone(), built_in],
        );
        let (_, output) = state.move_target(shown).unwrap();
        assert_eq!(*output, external);

        // A desktop of a workspace that is not on screen has nowhere to go.
        let hidden = desktop_on(&mut state, &connection, 2, 20, Vec::new());
        assert!(state.move_target(hidden).is_none());
        state.handles.get_mut(&shown).unwrap().1.group = None;
        assert!(state.move_target(shown).is_none());
        assert!(state.move_target(99).is_none());
    }

    #[cfg(feature = "cosmic-toplevel")]
    #[test]
    fn a_window_moves_to_a_desktop_only_when_the_compositor_can_say_where() {
        use crate::foreign_toplevel::{
            CosmicToplevelManagerData, ToplevelAction, execute_toplevel_action,
        };
        use cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1;
        use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1;
        use std::io::Read;

        let (socket, mut peer) = UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        let connection = Connection::from_socket(socket).unwrap();
        let queue = connection.new_event_queue::<WindowState<()>>();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let output = registry.bind::<WlOutput, _, _>(1, 4, &qh, ());
        let mut state = WindowState::<()>::new("move-to-desktop-test");
        let move_it = ToplevelAction::MoveToDesktop {
            toplevel: 5,
            desktop: 10,
        };
        // The requests the client sent since the last call: (sender, opcode, args).
        let mut sent = || {
            connection.flush().unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = peer.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            let words: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|w| u32::from_ne_bytes(w.try_into().unwrap()))
                .collect();
            let mut messages = Vec::new();
            let mut at = 0;
            while at + 2 <= words.len() {
                let len = (words[at + 1] >> 16) as usize / 4;
                messages.push((
                    words[at],
                    words[at + 1] & 0xffff,
                    words[at + 2..at + len].to_vec(),
                ));
                at += len;
            }
            messages
        };
        sent();

        assert!(
            !execute_toplevel_action(&state, move_it.clone(), None),
            "no manager"
        );

        state.cosmic_toplevel_manager = Some(registry.bind::<ZcosmicToplevelManagerV1, _, _>(
            2,
            3,
            &qh,
            CosmicToplevelManagerData,
        ));
        // Bound by hand so the requests carry live objects; a compositor sends these.
        let toplevel = registry.bind::<ZcosmicToplevelHandleV1, _, _>(
            3,
            1,
            &qh,
            crate::foreign_toplevel::CosmicToplevelHandleData { ext_handle_id: 5 },
        );
        let desktop = registry.bind::<ExtWorkspaceHandleV1, _, _>(4, 1, &qh, WorkspaceHandleData);
        state.cosmic_toplevel_handles.insert(5, toplevel.clone());
        desktop_on(
            &mut state.workspaces,
            &connection,
            1,
            10,
            vec![output.clone()],
        );
        state.workspaces.handles.get_mut(&10).unwrap().0 = desktop.clone();
        sent();
        assert!(
            !execute_toplevel_action(&state, move_it.clone(), None),
            "v3 has no move"
        );
        assert!(sent().is_empty());

        let manager =
            registry.bind::<ZcosmicToplevelManagerV1, _, _>(2, 4, &qh, CosmicToplevelManagerData);
        let manager_id = manager.id().protocol_id();
        state.cosmic_toplevel_manager = Some(manager);
        sent();
        assert!(!execute_toplevel_action(
            &state,
            ToplevelAction::MoveToDesktop {
                toplevel: 6,
                desktop: 10
            },
            None
        ));
        assert!(!execute_toplevel_action(
            &state,
            ToplevelAction::MoveToDesktop {
                toplevel: 5,
                desktop: 11
            },
            None
        ));
        assert!(
            sent().is_empty(),
            "nothing is sent for a window or desktop that is gone"
        );

        assert!(execute_toplevel_action(&state, move_it, None));
        // move_to_ext_workspace is request 13: toplevel, workspace, then the output.
        let moves = sent();
        assert_eq!(moves.len(), 1);
        let (sender, opcode, args) = &moves[0];
        assert_eq!((*sender, *opcode), (manager_id, 13));
        let ids = [toplevel.id(), desktop.id(), output.id()].map(|id| id.protocol_id());
        assert_eq!(args[..], ids);
    }
}
