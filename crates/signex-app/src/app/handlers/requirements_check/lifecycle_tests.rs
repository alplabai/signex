//! The state names `row_states` reports are the names a `parts_state`
//! requirement may list (#2339).

use signex_library::LifecycleState;

#[test]
fn every_lifecycle_state_is_a_name_a_requirement_may_list() {
    let names: Vec<&str> = LifecycleState::ALL
        .into_iter()
        .map(LifecycleState::as_str)
        .collect();
    assert_eq!(names.len(), signex_req::LIFECYCLE_STATES.len());
    for name in &names {
        assert!(signex_req::LIFECYCLE_STATES.contains(name), "{name}");
    }
}
