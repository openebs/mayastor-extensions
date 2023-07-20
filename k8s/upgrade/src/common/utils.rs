use openapi::models::{Child, ChildState};

/// Returns true if child is faulted or degrade.
pub fn is_child_degraded_or_faulted(child: &Child) -> bool {
    match child.state {
        ChildState::Degraded => true,
        ChildState::Faulted => true,
        _ => false,
    }
}
