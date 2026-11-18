use super::{DispatchMessageInner, Shell, WindowState, WindowStateUnitBuilder, id, xkb_keyboard};
use std::os::unix::net::UnixStream;
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_keyboard, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

#[test]
fn keyboard_enter_reports_focus_after_a_new_window_was_preselected() {
    for preselected in [true, false] {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(socket).unwrap();
        let queue = connection.new_event_queue::<WindowState<()>>();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
        let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(2, 4, &qh, ());
        let seat = registry.bind::<WlSeat, _, _>(3, 5, &qh, ());
        let keyboard = seat.get_keyboard(&qh, ());
        let mut state = WindowState::new("keyboard-focus-test");
        state.keyboard_state = Some(xkb_keyboard::KeyboardState::new(keyboard.clone()));

        let mut create = || {
            let surface = compositor.create_surface(&qh, ());
            let shell = layer_shell.get_layer_surface(
                &surface,
                None,
                Layer::Top,
                "keyboard-focus-test".into(),
                &qh,
                (),
            );
            let id = id::Id::unique();
            state.push_window(
                WindowStateUnitBuilder::new(
                    id,
                    qh.clone(),
                    connection.display(),
                    surface.clone(),
                    Shell::LayerShell(shell),
                )
                .build(),
            );
            (id, surface)
        };
        let (_, main) = create();
        let (drawer_id, drawer) = create();
        if !preselected {
            state.update_current_surface(Some(main.clone()));
        }
        // Creation's event can precede the consumer registering its window alias.
        state.message.clear();
        <WindowState<()> as Dispatch<wl_keyboard::WlKeyboard, ()>>::event(
            &mut state,
            &keyboard,
            wl_keyboard::Event::Leave {
                serial: 1,
                surface: main,
            },
            &(),
            &connection,
            &qh,
        );
        <WindowState<()> as Dispatch<wl_keyboard::WlKeyboard, ()>>::event(
            &mut state,
            &keyboard,
            wl_keyboard::Event::Enter {
                serial: 2,
                surface: drawer.clone(),
                keys: Vec::new(),
            },
            &(),
            &connection,
            &qh,
        );

        let focused: Vec<_> = state
            .message
            .iter()
            .filter_map(|(target, message)| match message {
                DispatchMessageInner::Focused(id) => Some((*target, *id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            focused,
            [(Some(drawer_id), drawer_id)],
            "each real Enter reports focus once, even after preselection={preselected}"
        );
        assert_eq!(state.current_surface, Some(drawer));
    }
}

// A keymap with one key, so the test needs no xkb data files.
const ONE_KEY_KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "test" { minimum = 8; maximum = 255; <AC01> = 38; };
    xkb_types "test" {
        type "ONE_LEVEL" { modifiers = none; map[none] = Level1; level_name[Level1] = "Any"; };
    };
    xkb_compat "test" { };
    xkb_symbols "test" { key <AC01> { [ a ] }; };
};"#;

#[test]
fn keys_follow_the_keyboard_focus_not_the_last_click() {
    use std::io::Write;
    use wayland_client::WEnum;
    use wayland_client::protocol::wl_pointer;

    let (socket, _peer) = UnixStream::pair().unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
    let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(2, 4, &qh, ());
    let seat = registry.bind::<WlSeat, _, _>(3, 5, &qh, ());
    let keyboard = seat.get_keyboard(&qh, ());
    let pointer = seat.get_pointer(&qh, ());
    let mut state = WindowState::new("keyboard-focus-test");
    state.keyboard_state = Some(xkb_keyboard::KeyboardState::new(keyboard.clone()));

    let mut create = || {
        let surface = compositor.create_surface(&qh, ());
        let shell = layer_shell.get_layer_surface(
            &surface,
            None,
            Layer::Overlay,
            "keyboard-focus-test".into(),
            &qh,
            (),
        );
        let id = id::Id::unique();
        state.push_window(
            WindowStateUnitBuilder::new(
                id,
                qh.clone(),
                connection.display(),
                surface.clone(),
                Shell::LayerShell(shell),
            )
            .build(),
        );
        (id, surface)
    };
    // A card that takes the keyboard, and a shield behind it that never does.
    let (card_id, card) = create();
    let (shield_id, shield) = create();

    let mut keymap = tempfile::tempfile().unwrap();
    keymap.write_all(ONE_KEY_KEYMAP.as_bytes()).unwrap();
    keymap.write_all(&[0]).unwrap();
    let key = |state: &mut WindowState<()>, event| {
        <WindowState<()> as Dispatch<wl_keyboard::WlKeyboard, ()>>::event(
            state,
            &keyboard,
            event,
            &(),
            &connection,
            &qh,
        );
    };
    key(
        &mut state,
        wl_keyboard::Event::Keymap {
            format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
            fd: keymap.into(),
            size: ONE_KEY_KEYMAP.len() as u32 + 1,
        },
    );
    key(
        &mut state,
        wl_keyboard::Event::Enter {
            serial: 1,
            surface: card,
            keys: Vec::new(),
        },
    );

    let point = |state: &mut WindowState<()>, event| {
        <WindowState<()> as Dispatch<wl_pointer::WlPointer, ()>>::event(
            state,
            &pointer,
            event,
            &(),
            &connection,
            &qh,
        );
    };
    point(
        &mut state,
        wl_pointer::Event::Enter {
            serial: 2,
            surface: shield,
            surface_x: 1.0,
            surface_y: 1.0,
        },
    );
    for (serial, button_state) in [
        (3, wl_pointer::ButtonState::Pressed),
        (4, wl_pointer::ButtonState::Released),
    ] {
        point(
            &mut state,
            wl_pointer::Event::Button {
                serial,
                time: 0,
                button: 0x110,
                state: WEnum::Value(button_state),
            },
        );
    }
    state.message.clear();

    // The compositor kept the keyboard on the card, so a key goes there.
    key(
        &mut state,
        wl_keyboard::Event::Key {
            serial: 5,
            time: 0,
            key: 30,
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
        },
    );

    let targets: Vec<_> = state
        .message
        .iter()
        .filter_map(|(target, message)| match message {
            DispatchMessageInner::KeyboardInput { .. } => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(targets, [Some(card_id)], "the key goes to the card");
    assert_eq!(state.keyboard_surface_id(), Some(card_id));
    assert_eq!(
        state.current_surface_id(),
        Some(shield_id),
        "the click still moves the pointer's own target"
    );
}

#[test]
fn a_held_key_repeats_as_a_repeat_not_as_a_new_press() {
    use std::io::Write;
    use wayland_client::WEnum;

    let (socket, _peer) = UnixStream::pair().unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let compositor = registry.bind::<WlCompositor, _, _>(1, 4, &qh, ());
    let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(2, 4, &qh, ());
    let seat = registry.bind::<WlSeat, _, _>(3, 5, &qh, ());
    let keyboard = seat.get_keyboard(&qh, ());
    let mut state = WindowState::new("key-repeat-test");
    state.keyboard_state = Some(xkb_keyboard::KeyboardState::new(keyboard.clone()));

    let surface = compositor.create_surface(&qh, ());
    let shell = layer_shell.get_layer_surface(
        &surface,
        None,
        Layer::Overlay,
        "key-repeat-test".into(),
        &qh,
        (),
    );
    let id = id::Id::unique();
    state.push_window(
        WindowStateUnitBuilder::new(
            id,
            qh.clone(),
            connection.display(),
            surface.clone(),
            Shell::LayerShell(shell),
        )
        .build(),
    );

    let mut keymap = tempfile::tempfile().unwrap();
    keymap.write_all(ONE_KEY_KEYMAP.as_bytes()).unwrap();
    keymap.write_all(&[0]).unwrap();
    let key = |state: &mut WindowState<()>, event| {
        <WindowState<()> as Dispatch<wl_keyboard::WlKeyboard, ()>>::event(
            state,
            &keyboard,
            event,
            &(),
            &connection,
            &qh,
        );
    };
    key(
        &mut state,
        wl_keyboard::Event::Keymap {
            format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
            fd: keymap.into(),
            size: ONE_KEY_KEYMAP.len() as u32 + 1,
        },
    );
    key(
        &mut state,
        wl_keyboard::Event::RepeatInfo {
            rate: 25,
            delay: 600,
        },
    );
    key(
        &mut state,
        wl_keyboard::Event::Enter {
            serial: 1,
            surface,
            keys: Vec::new(),
        },
    );
    state.message.clear();

    key(
        &mut state,
        wl_keyboard::Event::Key {
            serial: 2,
            time: 0,
            key: 30,
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
        },
    );
    // What the repeat timer runs each time it fires while the key is held.
    let held = state
        .repeat_delay
        .take()
        .expect("a held key arms the repeat");
    for _ in 0..2 {
        assert!(
            state
                .push_key_repeat(held.key, held.surface_id, held.pressed_state)
                .is_some()
        );
    }

    let repeats: Vec<_> = state
        .message
        .iter()
        .filter_map(|(target, message)| match message {
            DispatchMessageInner::KeyboardInput { event, .. } => Some((*target, event.repeat)),
            _ => None,
        })
        .collect();
    assert_eq!(
        repeats,
        [(Some(id), false), (Some(id), true), (Some(id), true)],
        "the press, then its repeats"
    );
}
