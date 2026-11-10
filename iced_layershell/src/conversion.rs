mod keymap;

use crate::event::IcedButtonState;
use crate::event::WindowEvent as LayerShellEvent;
use iced_core::SmolStr;
use iced_core::input_method;
use iced_core::touch;
use iced_core::{Event as IcedEvent, keyboard, mouse};
use keymap::{key, physical_key};
use layershellev::keyboard::KeyLocation;
use layershellev::keyboard::ModifiersState;
use layershellev::xkb_keyboard::ElementState;
use layershellev::xkb_keyboard::KeyEvent as LayerShellKeyEvent;
use std::ops::Mul;

fn scale_down<T>((x, y): (T, T), application_scale_factor: f64) -> (T, T)
where
    T: Mul + TryInto<f64> + TryFrom<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Debug,
    <T as TryFrom<f64>>::Error: std::fmt::Debug,
{
    let (mut x, mut y): (f64, f64) = (x.try_into().unwrap(), y.try_into().unwrap());
    x /= application_scale_factor;
    y /= application_scale_factor;
    (x.try_into().unwrap(), y.try_into().unwrap())
}

pub fn window_event(
    layerevent: &LayerShellEvent,
    application_scale_factor: f64,
    modifiers: ModifiersState,
) -> Option<IcedEvent> {
    match layerevent {
        LayerShellEvent::CursorLeft => Some(IcedEvent::Mouse(mouse::Event::CursorLeft)),
        LayerShellEvent::CursorMoved { x, y } => {
            let (x, y) = scale_down((*x, *y), application_scale_factor);
            Some(IcedEvent::Mouse(mouse::Event::CursorMoved {
                position: iced_core::Point {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::CursorEnter { .. } => Some(IcedEvent::Mouse(mouse::Event::CursorEntered)),
        LayerShellEvent::MouseInput(state) => Some(IcedEvent::Mouse(match state {
            IcedButtonState::Pressed(btn) => mouse::Event::ButtonPressed(*btn),
            IcedButtonState::Released(btn) => mouse::Event::ButtonReleased(*btn),
        })),
        LayerShellEvent::Axis { x, y } => Some(IcedEvent::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: *x, y: *y },
        })),

        LayerShellEvent::PixelDelta { x, y } => {
            let (x, y) = scale_down((f64::from(*x), f64::from(*y)), application_scale_factor);
            Some(IcedEvent::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::KeyBoardInput { event, .. } => Some(IcedEvent::Keyboard({
            let key = event.key_without_modifiers();
            let text = event
                .text_with_all_modifiers()
                .map(SmolStr::new)
                .filter(|text| !text.as_str().chars().any(is_private_use));
            let LayerShellKeyEvent {
                state,
                location,
                logical_key,
                physical_key,
                ..
            } = event;
            let key = self::key(key);
            let modified_key = self::key(logical_key.clone());

            let physical_key = self::physical_key(*physical_key);

            let modifiers = keymap::modifiers(modifiers);

            let location = match location {
                KeyLocation::Standard => keyboard::Location::Standard,
                KeyLocation::Left => keyboard::Location::Left,
                KeyLocation::Right => keyboard::Location::Right,
                KeyLocation::Numpad => keyboard::Location::Numpad,
            };
            match state {
                ElementState::Pressed => keyboard::Event::KeyPressed {
                    key,
                    location,
                    modifiers,
                    text,
                    modified_key,
                    physical_key,
                    repeat: event.repeat,
                },
                ElementState::Released => keyboard::Event::KeyReleased {
                    key,
                    location,
                    modifiers,
                    physical_key,
                    modified_key,
                },
            }
        })),
        LayerShellEvent::TouchDown { id, x, y } => {
            let (x, y) = scale_down((*x, *y), application_scale_factor);
            Some(IcedEvent::Touch(touch::Event::FingerPressed {
                id: touch::Finger(*id as u64),
                position: iced_core::Point {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::TouchUp { id, x, y } => {
            let (x, y) = scale_down((*x, *y), application_scale_factor);
            Some(IcedEvent::Touch(touch::Event::FingerLifted {
                id: touch::Finger(*id as u64),
                position: iced_core::Point {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::TouchMotion { id, x, y } => {
            let (x, y) = scale_down((*x, *y), application_scale_factor);
            Some(IcedEvent::Touch(touch::Event::FingerMoved {
                id: touch::Finger(*id as u64),
                position: iced_core::Point {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::TouchCancel { id, x, y } => {
            let (x, y) = scale_down((*x, *y), application_scale_factor);
            Some(IcedEvent::Touch(touch::Event::FingerLost {
                id: touch::Finger(*id as u64),
                position: iced_core::Point {
                    x: x as f32,
                    y: y as f32,
                },
            }))
        }
        LayerShellEvent::ModifiersChanged(new_modifiers) => Some(IcedEvent::Keyboard(
            keyboard::Event::ModifiersChanged(keymap::modifiers(*new_modifiers)),
        )),
        LayerShellEvent::Unfocus => Some(IcedEvent::Window(iced_core::window::Event::Unfocused)),
        LayerShellEvent::Focused => Some(IcedEvent::Window(iced_core::window::Event::Focused)),
        LayerShellEvent::ScaleFactorChanged { scale_float, .. } => Some(IcedEvent::Window(
            iced_core::window::Event::Rescaled(*scale_float as f32),
        )),
        // Drag-and-drop from other apps. Wayland doesn't reveal the dragged paths
        // until the drop, so hover carries an empty path (a highlight cue only);
        // the real paths arrive on `FileDropped`.
        LayerShellEvent::DndEntered { x, y, mime_types } => {
            Some(IcedEvent::Dnd(iced_core::dnd::Event::Enter {
                x: *x,
                y: *y,
                mime_types: mime_types.clone(),
            }))
        }
        LayerShellEvent::DndMotion { x, y } => {
            Some(IcedEvent::Dnd(iced_core::dnd::Event::Motion {
                x: *x,
                y: *y,
            }))
        }
        LayerShellEvent::DndLeft => Some(IcedEvent::Dnd(iced_core::dnd::Event::Leave)),
        LayerShellEvent::DndDrop => Some(IcedEvent::Dnd(iced_core::dnd::Event::Drop)),
        LayerShellEvent::DndDataReceived { mime_type, data } => {
            Some(IcedEvent::Dnd(iced_core::dnd::Event::DataReceived {
                mime_type: mime_type.clone(),
                data: data.clone(),
            }))
        }
        LayerShellEvent::DndSelectedAction(bits) => {
            Some(IcedEvent::Dnd(iced_core::dnd::Event::SelectedAction(
                iced_core::dnd::DndAction::from_bits_truncate(*bits),
            )))
        }
        LayerShellEvent::DndSourceCancelled => Some(IcedEvent::Dnd(
            iced_core::dnd::Event::SourceEvent(iced_core::dnd::SourceEvent::Cancelled),
        )),
        LayerShellEvent::DndSourceDropPerformed => Some(IcedEvent::Dnd(
            iced_core::dnd::Event::SourceEvent(iced_core::dnd::SourceEvent::DropPerformed),
        )),
        LayerShellEvent::DndSourceFinished => Some(IcedEvent::Dnd(
            iced_core::dnd::Event::SourceEvent(iced_core::dnd::SourceEvent::Finished),
        )),
        LayerShellEvent::DndSourceAction(bits) => Some(IcedEvent::Dnd(
            iced_core::dnd::Event::SourceEvent(iced_core::dnd::SourceEvent::Action(
                iced_core::dnd::DndAction::from_bits_truncate(*bits),
            )),
        )),
        // Kept for back-compat with consumers still on the legacy file-drop API.
        LayerShellEvent::FileDropped(path) => Some(IcedEvent::Window(
            iced_core::window::Event::FileDropped(path.clone()),
        )),
        LayerShellEvent::Ime(event) => Some(IcedEvent::InputMethod(match event {
            layershellev::Ime::Enabled => input_method::Event::Opened,
            layershellev::Ime::Preedit(content, size) => {
                input_method::Event::Preedit(content.clone(), size.map(|(start, end)| start..end))
            }
            layershellev::Ime::Commit(content) => input_method::Event::Commit(content.clone()),
            layershellev::Ime::Disabled => input_method::Event::Closed,
        })),
        _ => None,
    }
}

pub fn ime_purpose(purpose: input_method::Purpose) -> layershellev::ImePurpose {
    match purpose {
        input_method::Purpose::Normal => layershellev::ImePurpose::Normal,
        input_method::Purpose::Secure => layershellev::ImePurpose::Password,
        input_method::Purpose::Terminal => layershellev::ImePurpose::Terminal,
    }
}

pub fn ime_surrounding_text(
    surrounding: &input_method::Surrounding,
) -> layershellev::ImeSurroundingText {
    layershellev::ImeSurroundingText {
        text: surrounding.text.clone(),
        cursor: surrounding.cursor,
        anchor: surrounding.anchor,
    }
}

pub(crate) fn mouse_interaction(interaction: mouse::Interaction) -> String {
    use layershellev::reexport::wp_cursor_shape_device_v1::{Shape, ShapeName};
    use mouse::Interaction;
    match interaction {
        Interaction::Hidden => "none".to_owned(),
        Interaction::None => Shape::Default.name().to_owned(),
        Interaction::Idle => Shape::Default.name().to_owned(),
        Interaction::Wait => Shape::Wait.name().to_owned(),
        Interaction::Pointer => Shape::Pointer.name().to_owned(),
        Interaction::Grab => Shape::Grab.name().to_owned(),
        Interaction::Text => Shape::Text.name().to_owned(),
        Interaction::ZoomIn => Shape::ZoomIn.name().to_owned(),
        Interaction::Grabbing => Shape::Grabbing.name().to_owned(),
        Interaction::Crosshair => Shape::Crosshair.name().to_owned(),
        Interaction::NotAllowed => Shape::NotAllowed.name().to_owned(),
        Interaction::ResizingVertically => Shape::NsResize.name().to_owned(),
        Interaction::ResizingHorizontally => Shape::EwResize.name().to_owned(),
        Interaction::Cell => Shape::Cell.name().to_owned(),
        Interaction::Move => Shape::Move.name().to_owned(),
        Interaction::Copy => Shape::Copy.name().to_owned(),
        Interaction::Help => Shape::Help.name().to_owned(),
        Interaction::ZoomOut => Shape::ZoomOut.name().to_owned(),
        Interaction::ResizingDiagonallyUp => Shape::NeswResize.name().to_owned(),
        Interaction::ResizingDiagonallyDown => Shape::NwseResize.name().to_owned(),
        Interaction::ResizingColumn => Shape::ColResize.name().to_owned(),
        Interaction::ResizingRow => Shape::RowResize.name().to_owned(),
        _ => Shape::Default.name().to_owned(),
    }
}

fn is_private_use(c: char) -> bool {
    ('\u{E000}'..='\u{F8FF}').contains(&c)
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    use layershellev::xkb_keyboard::Context;
    use std::io::Write;

    #[test]
    fn keyboard_conversion_preserves_repeat_and_release() {
        // A self-contained keymap avoids depending on the session's layout.
        let keymap = concat!(
            "xkb_keymap {",
            "xkb_keycodes \"test\" { minimum = 8; maximum = 255; <DOWN> = 116; };",
            "xkb_types \"test\" { type \"ONE_LEVEL\" { modifiers = None; map[None] = Level1; }; };",
            "xkb_compatibility \"test\" {};",
            "xkb_symbols \"test\" { key <DOWN> { type = \"ONE_LEVEL\", [ Down ] }; };",
            "};\0",
        );
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(keymap.as_bytes()).unwrap();
        let mut context = Context::new().unwrap();
        context.set_keymap_from_fd(file.into(), keymap.len());

        for (state, repeat) in [
            (ElementState::Pressed, false),
            (ElementState::Pressed, true),
            (ElementState::Released, false),
        ] {
            let event = LayerShellEvent::KeyBoardInput {
                event: context
                    .key_context()
                    .unwrap()
                    .process_key_event(116, state, repeat),
                is_synthetic: false,
            };
            let converted = window_event(&event, 1.0, ModifiersState::empty());
            match converted {
                Some(IcedEvent::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    repeat: actual,
                    ..
                })) => {
                    assert_eq!(state, ElementState::Pressed);
                    assert_eq!(key, keyboard::Key::Named(keyboard::key::Named::ArrowDown));
                    assert_eq!(
                        actual, repeat,
                        "repeated presses must not become fresh presses"
                    );
                }
                Some(IcedEvent::Keyboard(keyboard::Event::KeyReleased { key, .. })) => {
                    assert_eq!(state, ElementState::Released);
                    assert_eq!(key, keyboard::Key::Named(keyboard::key::Named::ArrowDown));
                }
                other => panic!("expected a keyboard event, got {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::mouse_interaction;
    use iced_core::mouse::Interaction;

    #[test]
    fn hidden_is_not_a_default_arrow_and_mouse_interactions_restore_a_shape() {
        assert_eq!(mouse_interaction(Interaction::Hidden), "none");
        assert_eq!(mouse_interaction(Interaction::Idle), "default");
        assert_eq!(mouse_interaction(Interaction::Pointer), "pointer");
    }

    #[test]
    fn each_resize_points_its_own_way() {
        // iced: "up" runs bottom-left to top-right, "down" top-left to bottom-right.
        assert_eq!(
            mouse_interaction(Interaction::ResizingDiagonallyUp),
            "nesw_resize"
        );
        assert_eq!(
            mouse_interaction(Interaction::ResizingDiagonallyDown),
            "nwse_resize"
        );
        assert_eq!(
            mouse_interaction(Interaction::ResizingVertically),
            "ns_resize"
        );
        assert_eq!(
            mouse_interaction(Interaction::ResizingHorizontally),
            "ew_resize"
        );
        assert_eq!(mouse_interaction(Interaction::ResizingColumn), "col_resize");
        assert_eq!(mouse_interaction(Interaction::ResizingRow), "row_resize");
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::*;
    use layershellev::reexport::wayland_client::wl_pointer::AxisSource;
    use layershellev::{AxisScroll, DispatchMessage};

    /// What iced gets for a touchpad swipe of `finger` surface pixels upwards.
    fn swipe(finger: f64, output_scale: f64, application_scale: f64) -> Option<IcedEvent> {
        let message = DispatchMessage::Axis {
            time: 0,
            scale: output_scale,
            horizontal: AxisScroll::default(),
            vertical: AxisScroll {
                absolute: finger,
                ..AxisScroll::default()
            },
            source: Some(AxisSource::Finger),
        };
        let event = LayerShellEvent::from(&message);
        window_event(&event, application_scale, ModifiersState::empty())
    }

    #[test]
    fn a_swipe_scrolls_by_the_finger_distance_at_any_output_scale() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            assert_eq!(
                swipe(4.0, scale, 1.0),
                Some(IcedEvent::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Pixels { x: -0.0, y: -4.0 },
                })),
                "at output scale {scale}"
            );
        }
    }

    #[test]
    fn a_swipe_follows_the_application_scale_like_the_cursor() {
        assert_eq!(
            swipe(4.0, 1.5, 2.0),
            Some(IcedEvent::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: -0.0, y: -2.0 },
            }))
        );
    }
}
