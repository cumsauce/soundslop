//! Gesture wrapping for parameter writes (pure state machine).
//!
//! Hosts use begin/end ("begin_edit"/"end_edit") to decide what counts as
//! one automation gesture. This module decides, from the three facts a
//! widget knows each frame (was a gesture already open, is the pointer
//! pressed on this control, did the value change), how the write must be
//! wrapped and whether the gesture stays open for the next frame.

/// The wrapping to apply around this frame's parameter write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing happened; no host call.
    None,
    /// Open a gesture (pointer pressed before any change).
    Begin,
    /// Open a gesture and write the value (press that already carries a value).
    BeginSet,
    /// Write inside the already-open gesture (drag continues).
    Set,
    /// Close the open gesture (pointer released without a final change).
    End,
    /// Self-contained gesture: typed entry, keyboard step, or a release frame
    /// that still carries a final change (the open gesture is closed first).
    BeginSetEnd,
}

/// `open` — a gesture was opened for this control on a previous frame.
/// `pressed` — the pointer is down on this control this frame.
/// `changed` — the control produced a new value this frame.
///
/// Returns the step to apply and whether a gesture is open afterwards.
pub fn next(open: bool, pressed: bool, changed: bool) -> (Step, bool) {
    match (open, pressed, changed) {
        (_, true, true) if open => (Step::Set, true),
        (_, true, true) => (Step::BeginSet, true),
        (_, true, false) => (Step::Begin, true),
        (true, false, true) => (Step::BeginSetEnd, false),
        (true, false, false) => (Step::End, false),
        (false, false, true) => (Step::BeginSetEnd, false),
        (false, false, false) => (Step::None, false),
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, next};

    #[test]
    fn press_opens_gesture_before_any_change() {
        assert_eq!(next(false, true, false), (Step::Begin, true));
        assert_eq!(next(true, true, false), (Step::Begin, true));
    }

    #[test]
    fn press_that_already_carries_a_value_begins_and_sets() {
        assert_eq!(next(false, true, true), (Step::BeginSet, true));
        assert_eq!(next(true, true, true), (Step::Set, true));
    }

    #[test]
    fn release_without_final_change_closes() {
        assert_eq!(next(true, false, false), (Step::End, false));
    }

    #[test]
    fn release_frame_with_final_change_is_self_contained() {
        // The open drag gesture is closed, then the final value stands as its
        // own complete gesture.
        assert_eq!(next(true, false, true), (Step::BeginSetEnd, false));
    }

    #[test]
    fn typed_entry_is_a_self_contained_gesture() {
        assert_eq!(next(false, false, true), (Step::BeginSetEnd, false));
    }

    #[test]
    fn idle_frames_do_nothing() {
        assert_eq!(next(false, false, false), (Step::None, false));
    }
}
