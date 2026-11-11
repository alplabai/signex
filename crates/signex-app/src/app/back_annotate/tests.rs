use super::*;

#[test]
fn a_project_without_a_board_has_nothing_to_read() {
    let state = BackAnnotateState::new(ProjectId::test_id(1), "p.snxprj".into(), None);
    assert!(matches!(state.phase, Phase::NoBoard));
    assert!(state.ready_list().is_none());
}

#[test]
fn the_list_shows_only_once_ready() {
    let mut state = BackAnnotateState::new(
        ProjectId::test_id(1),
        "p.snxprj".into(),
        Some("b.snxpcb".into()),
    );
    assert!(matches!(state.phase, Phase::Opening));
    assert_eq!(state.board_name(), "b.snxpcb");
    state.list = Some(BackChangeList::default());
    assert!(state.ready_list().is_none(), "not while opening");
    state.phase = Phase::Ready;
    assert!(state.ready_list().is_some());
    assert_eq!((state.next_job(), state.next_job()), (1, 2));
}
