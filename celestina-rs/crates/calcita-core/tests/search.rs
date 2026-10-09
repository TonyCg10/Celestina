use calcita_core::search::{next_hit, Direction, SearchRequest};

#[test]
fn no_hits_gives_no_hit() {
    assert_eq!(next_hit(None, 0, Direction::Forward, true), None);
    assert_eq!(next_hit(Some(3), 0, Direction::Backward, true), None);
}

#[test]
fn the_first_step_lands_on_the_first_or_the_last_hit() {
    assert_eq!(next_hit(None, 5, Direction::Forward, false), Some(0));
    assert_eq!(next_hit(None, 5, Direction::Backward, false), Some(4));
}

#[test]
fn forward_and_backward_step_by_one() {
    assert_eq!(next_hit(Some(1), 5, Direction::Forward, true), Some(2));
    assert_eq!(next_hit(Some(1), 5, Direction::Backward, true), Some(0));
}

#[test]
fn wrapping_goes_round_both_ends() {
    assert_eq!(next_hit(Some(4), 5, Direction::Forward, true), Some(0));
    assert_eq!(next_hit(Some(0), 5, Direction::Backward, true), Some(4));
}

#[test]
fn without_wrapping_the_ends_stay_put() {
    assert_eq!(next_hit(Some(4), 5, Direction::Forward, false), Some(4));
    assert_eq!(next_hit(Some(0), 5, Direction::Backward, false), Some(0));
}

#[test]
fn a_hit_past_a_shrunk_list_starts_again() {
    assert_eq!(next_hit(Some(9), 3, Direction::Forward, true), Some(0));
    assert_eq!(next_hit(Some(9), 3, Direction::Backward, true), Some(2));
}

#[test]
fn a_request_trims_its_query_and_an_empty_one_searches_nothing() {
    let request = SearchRequest::new("  chapter two  ");
    assert_eq!(
        request.as_ref().map(|r| r.query.as_str()),
        Some("chapter two")
    );
    assert_eq!(SearchRequest::new("   "), None);
}
