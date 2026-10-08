use cuprita_core::agent::{Agent, AgentAnswer, AgentBusy, AgentRequest};

fn pin() -> AgentRequest {
    AgentRequest::Pin {
        device: "AA:BB:CC:00:00:02".into(),
    }
}

#[test]
fn a_pin_request_is_answered_once() {
    let mut agent = Agent::new();
    agent.request(pin()).unwrap();
    assert_eq!(agent.pending(), Some(&pin()));
    let answered = agent.answer(AgentAnswer::Pin("1234".into()));
    assert_eq!(answered, Some((pin(), AgentAnswer::Pin("1234".into()))));
    assert_eq!(agent.pending(), None);
}

#[test]
fn a_second_request_while_one_waits_is_busy() {
    let mut agent = Agent::new();
    agent.request(pin()).unwrap();
    let second = AgentRequest::Confirm {
        device: "AA:BB:CC:00:00:01".into(),
        passkey: 123_456,
    };
    assert_eq!(agent.request(second), Err(AgentBusy));
    assert_eq!(agent.pending(), Some(&pin()));
}

#[test]
fn cancelled_clears_the_request() {
    let mut agent = Agent::new();
    agent.request(pin()).unwrap();
    assert_eq!(
        agent.answer(AgentAnswer::Cancelled),
        Some((pin(), AgentAnswer::Cancelled))
    );
    assert_eq!(agent.pending(), None);
    assert_eq!(agent.answer(AgentAnswer::Confirmed), None);
}

#[test]
fn a_cancel_withdraws_the_waiting_request() {
    let mut agent = Agent::new();
    agent.request(pin()).unwrap();
    agent.request(AgentRequest::Cancel).unwrap();
    assert_eq!(agent.pending(), None);
    // Nothing waits, so a late answer goes nowhere.
    assert_eq!(agent.answer(AgentAnswer::Pin("1234".into())), None);
    assert_eq!(AgentRequest::Cancel.presentation(), None);
}

#[test]
fn requests_present_as_dialog_tokens() {
    assert_eq!(pin().presentation(), Some(("pin", "AA:BB:CC:00:00:02", 0)));
    let confirm = AgentRequest::Confirm {
        device: "Auriculares".into(),
        passkey: 123_456,
    };
    assert_eq!(
        confirm.presentation(),
        Some(("confirm", "Auriculares", 123_456))
    );
}

#[test]
fn dialog_tokens_become_answers() {
    assert_eq!(
        AgentAnswer::from_tokens("confirm", "yes"),
        AgentAnswer::Confirmed
    );
    assert_eq!(
        AgentAnswer::from_tokens("confirm", "no"),
        AgentAnswer::Rejected
    );
    assert_eq!(
        AgentAnswer::from_tokens("confirm", "cancel"),
        AgentAnswer::Cancelled
    );
    assert_eq!(
        AgentAnswer::from_tokens("pin", "0000"),
        AgentAnswer::Pin("0000".into())
    );
    assert_eq!(AgentAnswer::from_tokens("pin", ""), AgentAnswer::Cancelled);
    assert_eq!(
        AgentAnswer::from_tokens("display", "cancel"),
        AgentAnswer::Cancelled
    );
    // An unknown token never pairs.
    assert_eq!(
        AgentAnswer::from_tokens("confirm", "maybe"),
        AgentAnswer::Cancelled
    );
}
