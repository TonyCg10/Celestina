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
