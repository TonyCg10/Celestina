use calcita_core::pagelabel::{GoTo, GoToError};

#[test]
fn an_absolute_page() {
    assert_eq!(GoTo::parse("12", 1, 20), Ok(12));
    assert_eq!(GoTo::parse(" 3 ", 1, 20), Ok(3));
}

#[test]
fn a_relative_step() {
    assert_eq!(GoTo::parse("+3", 5, 20), Ok(8));
    assert_eq!(GoTo::parse("-2", 5, 20), Ok(3));
}

#[test]
fn a_step_before_the_first_page_is_refused() {
    assert_eq!(GoTo::parse("-2", 1, 20), Err(GoToError::OutOfRange));
}

#[test]
fn the_words_name_the_ends() {
    assert_eq!(GoTo::parse("fin", 3, 20), Ok(20));
    assert_eq!(GoTo::parse("inicio", 3, 20), Ok(1));
    assert_eq!(GoTo::parse("FIN", 3, 20), Ok(20));
}

#[test]
fn page_zero_and_past_the_end_are_refused() {
    assert_eq!(GoTo::parse("0", 3, 20), Err(GoToError::OutOfRange));
    assert_eq!(GoTo::parse("21", 3, 20), Err(GoToError::OutOfRange));
}

#[test]
fn words_that_are_no_page_are_refused() {
    assert_eq!(GoTo::parse("doce", 3, 20), Err(GoToError::NotAPage));
    assert_eq!(GoTo::parse("", 3, 20), Err(GoToError::NotAPage));
}

#[test]
fn a_document_without_pages_has_nowhere_to_go() {
    assert_eq!(GoTo::parse("1", 1, 0), Err(GoToError::OutOfRange));
}

#[test]
fn every_error_speaks_spanish() {
    assert!(!GoToError::NotAPage.message_es().is_empty());
    assert!(!GoToError::OutOfRange.message_es().is_empty());
}
