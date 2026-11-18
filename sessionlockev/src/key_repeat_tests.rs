use super::{DispatchMessageInner, WindowState, xkb_keyboard};
use std::io::Write;
use std::os::unix::net::UnixStream;
use wayland_client::protocol::{wl_keyboard, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, WEnum};

const ONE_KEY_KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "test" { minimum = 8; maximum = 255; <AC01> = 38; };
    xkb_types "test" {
        type "ONE_LEVEL" { modifiers = none; map[none] = Level1; level_name[Level1] = "Any"; };
    };
    xkb_compat "test" { };
    xkb_symbols "test" { key <AC01> { [ a ] }; };
};"#;

#[test]
fn a_held_key_repeats_as_a_repeat_not_as_a_new_press() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let connection = Connection::from_socket(socket).unwrap();
    let queue = connection.new_event_queue::<WindowState<()>>();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let seat = registry.bind::<WlSeat, _, _>(1, 5, &qh, ());
    let keyboard = seat.get_keyboard(&qh, ());
    let mut state = WindowState::<()>::new();
    state.keyboard_state = Some(xkb_keyboard::KeyboardState::new(keyboard.clone()));

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
        wl_keyboard::Event::Key {
            serial: 1,
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
        .filter_map(|(_, message)| match message {
            DispatchMessageInner::KeyboardInput { event, .. } => Some(event.repeat),
            _ => None,
        })
        .collect();
    assert_eq!(repeats, [false, true, true], "the press, then its repeats");
}
