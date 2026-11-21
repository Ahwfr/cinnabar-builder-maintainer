//! Cached, paged observations of the player's normal Keyboard & Mouse settings.

use crate::input;
use std::cell::RefCell;

/// The guest features bound in the player's Keyboard & Mouse settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Opens and closes the Cinnamatica window.
    Main,
    Render,
    Ghosts,
    NextMode,
    PreviousMode,
    NextLayer,
    PreviousLayer,
    /// Turns the tool item on and off.
    Tool,
    /// Litematica's Execute Operation; unbound by default.
    Execute,
    ToolNext,
    ToolPrevious,
}

impl Action {
    const ALL: [Action; 11] = [
        Action::Main,
        Action::Render,
        Action::Ghosts,
        Action::NextMode,
        Action::PreviousMode,
        Action::NextLayer,
        Action::PreviousLayer,
        Action::Tool,
        Action::Execute,
        Action::ToolNext,
        Action::ToolPrevious,
    ];
}

pub(super) const TRANSFORMS: [(&str, &str); 8] = [
    ("schematic-forward", "ViewForward"),
    ("schematic-left", "ViewLeft"),
    ("schematic-right", "ViewRight"),
    ("schematic-back", "ViewBack"),
    ("schematic-down", "PageDown"),
    ("schematic-up", "PageUp"),
    ("schematic-rotate-left", "RotateLeft"),
    ("schematic-rotate-right", "KeyR"),
];

#[derive(Default)]
struct Cache {
    revision: Option<u64>,
    collecting: Option<u64>,
    pending: Vec<input::ActionBinding>,
    bindings: Vec<input::ActionBinding>,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// Collects at most one page per frame and publishes only complete revisions.
pub fn sample() {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let offset = cache.pending.len() as u32;
        let after = if offset == 0 { cache.revision } else { None };
        let page = match input::read_bindings(after, offset, 16) {
            Ok(Some(page)) => page,
            Ok(None) => return,
            Err(_) => {
                cache.pending.clear();
                cache.collecting = None;
                return;
            }
        };
        if cache
            .collecting
            .is_some_and(|revision| revision != page.revision)
        {
            cache.pending.clear();
            cache.collecting = None;
            return;
        }
        cache.collecting = Some(page.revision);
        cache.pending.extend(page.entries);
        if cache.pending.len() == page.total as usize {
            cache.bindings = std::mem::take(&mut cache.pending);
            cache.revision = Some(page.revision);
            cache.collecting = None;
        }
    });
}

fn name(action: Action) -> &'static str {
    match action {
        Action::Main => "schematic-menu",
        Action::Render => "schematic-render",
        Action::Ghosts => "schematic-ghosts",
        Action::NextMode => "schematic-next-mode",
        Action::PreviousMode => "schematic-previous-mode",
        Action::NextLayer => "schematic-next-layer",
        Action::PreviousLayer => "schematic-previous-layer",
        Action::Tool => "schematic-tool",
        Action::Execute => "schematic-execute",
        Action::ToolNext => "schematic-tool-next",
        Action::ToolPrevious => "schematic-tool-previous",
    }
}

fn held(binding: &input::ActionBinding, keys: &input::Controls) -> bool {
    let held = |key: &str| keys.keys_held.iter().any(|value| value == key);
    (!binding.modifiers.shift || held("ShiftLeft") || held("ShiftRight"))
        && (!binding.modifiers.control || held("ControlLeft") || held("ControlRight"))
        && (!binding.modifiers.alt || held("AltLeft") || held("AltRight"))
        && (!binding.modifiers.super_key || held("SuperLeft") || held("SuperRight"))
}

/// A physical press of the named current gameplay binding.
pub fn pressed(action: &str, keys: &input::Controls) -> bool {
    CACHE.with(|cache| {
        cache.borrow().bindings.iter().any(|binding| {
            binding.action == action
                && binding.context == input::BindingContext::Gameplay
                && held(binding, keys)
                && keys.keys_pressed.contains(&binding.key)
        })
    })
}

pub fn is_held(action: &str, keys: &input::Controls) -> bool {
    CACHE.with(|cache| {
        cache.borrow().bindings.iter().any(|binding| {
            binding.action == action
                && binding.context == input::BindingContext::Gameplay
                && held(binding, keys)
                && keys.keys_admitted.contains(&binding.key)
        })
    })
}

fn fallback(action: Action) -> Option<(&'static str, &'static str)> {
    match action {
        Action::Main => Some(("KeyM", "M")),
        _ => None,
    }
}

/// Resolves guest features from the same physical settings rows the native router observes.
pub fn action(keys: &input::Controls, armed: &mut bool) -> Option<Action> {
    for action in Action::ALL {
        if pressed(name(action), keys) {
            *armed = false;
            return Some(action);
        }
    }
    for action in [Action::Main] {
        let bound = CACHE.with(|cache| {
            cache.borrow().bindings.iter().any(|binding| {
                binding.action == name(action) && binding.context == input::BindingContext::Gameplay
            })
        });
        if !bound
            && fallback(action)
                .is_some_and(|(key, _)| keys.keys_pressed.iter().any(|value| value == key))
        {
            *armed = false;
            return Some(action);
        }
    }
    None
}

/// The physical key of the menu binding, or M while it is unbound.
pub fn main_key() -> String {
    CACHE.with(|cache| {
        cache
            .borrow()
            .bindings
            .iter()
            .find(|binding| {
                binding.action == name(Action::Main)
                    && binding.context == input::BindingContext::Gameplay
            })
            .map_or_else(|| "KeyM".to_owned(), |binding| binding.key.clone())
    })
}

/// Whether this frame pressed the window's menu key: its current binding, or M while unbound.
pub fn menu_pressed(keys: &input::Controls) -> bool {
    action(keys, &mut false) == Some(Action::Main)
}

/// The bound key's caption, or `default` while the action is unbound.
pub fn caption_or(action: Action, default: &str) -> String {
    let bound = CACHE.with(|cache| {
        cache.borrow().bindings.iter().any(|binding| {
            binding.action == name(action) && binding.context == input::BindingContext::Gameplay
        })
    });
    if bound {
        caption(action)
    } else {
        default.to_owned()
    }
}

pub fn caption(action: Action) -> String {
    CACHE.with(|cache| {
        cache
            .borrow()
            .bindings
            .iter()
            .find(|binding| {
                binding.action == name(action) && binding.context == input::BindingContext::Gameplay
            })
            .map_or_else(
                || {
                    fallback(action)
                        .map_or("Unbound", |(_, title)| title)
                        .into()
                },
                |binding| {
                    binding
                        .key
                        .strip_prefix("Key")
                        .unwrap_or(&binding.key)
                        .to_owned()
                },
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remapped_menu_key_works_with_a_default_fallback() {
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.revision = Some(1);
            cache.bindings = vec![input::ActionBinding {
                action: "schematic-menu".into(),
                context: input::BindingContext::Gameplay,
                key: "F4".into(),
                modifiers: input::BindingModifiers {
                    shift: false,
                    control: false,
                    alt: false,
                    super_key: false,
                },
            }]
        });
        let mut keys = input::Controls {
            seconds: 0.,
            focused: true,
            gameplay: true,
            panel_open: false,
            keys_pressed: vec!["KeyM".into()],
            keys_held: vec!["KeyM".into()],
            keys_admitted: vec![],
            events: vec![],
            pointer: None,
            wheel: 0.,
        };
        assert!(action(&keys, &mut false).is_none());
        keys.keys_pressed = vec!["F4".into()];
        assert!(action(&keys, &mut false) == Some(Action::Main));
        CACHE.with(|cache| cache.borrow_mut().bindings.clear());
        assert!(action(&keys, &mut false).is_none());
        keys.keys_pressed = vec!["KeyM".into()];
        assert!(action(&keys, &mut false) == Some(Action::Main));
        assert_eq!(caption(Action::Main), "M");
    }

    #[test]
    fn tool_rows_work_only_once_bound_and_the_menu_key_follows_its_row() {
        CACHE.with(|cache| cache.borrow_mut().bindings.clear());
        let mut keys = input::Controls {
            seconds: 0.,
            focused: true,
            gameplay: true,
            panel_open: false,
            keys_pressed: vec!["KeyK".into()],
            keys_held: vec!["KeyK".into()],
            keys_admitted: vec![],
            events: vec![],
            pointer: None,
            wheel: 0.,
        };
        assert!(action(&keys, &mut false).is_none());
        assert_eq!(main_key(), "KeyM");
        let row = |action: &str, key: &str| input::ActionBinding {
            action: action.into(),
            context: input::BindingContext::Gameplay,
            key: key.into(),
            modifiers: input::BindingModifiers {
                shift: false,
                control: false,
                alt: false,
                super_key: false,
            },
        };
        CACHE.with(|cache| {
            cache.borrow_mut().bindings = vec![
                row("schematic-execute", "KeyJ"),
                row("schematic-menu", "F4"),
            ]
        });
        keys.keys_pressed = vec!["KeyJ".into()];
        assert!(action(&keys, &mut false) == Some(Action::Execute));
        assert_eq!(main_key(), "F4");
        CACHE.with(|cache| cache.borrow_mut().bindings.clear());
    }
}
