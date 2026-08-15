import QtQuick
import QtTest
import "../qml" as Desktop

// What the authorization prompt shows, and where every word of it came from.
//
// The strings polkitd sends are the ones an attacker controls least and a
// person needs most, so the test that matters here is that they arrive
// unedited — not that the card looks a particular way.
TestCase {
    id: testCase

    name: "PolkitPrompt"

    QtObject {
        id: fakeSource

        property var responses: []
        property int dismissals: 0

        function respond(secret) {
            fakeSource.responses.push(secret);
        }

        function dismiss() {
            fakeSource.dismissals += 1;
        }
    }

    Desktop.PolkitPrompt {
        id: prompt

        promptSource: fakeSource
        reducedMotion: true
        actionId: "org.freedesktop.policykit.exec"
        message: "Authentication is required to run a program as another user"
        iconName: "dialog-password"
        identity: "toni"
    }

    function init() {
        fakeSource.responses = [];
        fakeSource.dismissals = 0;
        prompt.prompt = "";
        prompt.problem = "";
        prompt.notice = "";
    }

    function findByText(item, needle) {
        if (item === null)
            return null;
        if (item.text !== undefined && item.text === needle)
            return item;
        for (let index = 0; index < item.children.length; ++index) {
            const found = testCase.findByText(item.children[index], needle);
            if (found !== null)
                return found;
        }
        return null;
    }

    function findPasswordField(item) {
        if (item === null)
            return null;
        if (item.echoMode !== undefined && item.echoMode === TextInput.Password)
            return item;
        for (let index = 0; index < item.children.length; ++index) {
            const found = testCase.findPasswordField(item.children[index]);
            if (found !== null)
                return found;
        }
        return null;
    }

    // polkitd's message and the action's own id are both on screen. The id is
    // the one string a hostile caller cannot dress up, which is why it is
    // shown rather than summarized away.
    function test_showsWhatPolkitSent() {
        verify(testCase.findByText(prompt.contentItem, prompt.message) !== null);
        verify(testCase.findByText(prompt.contentItem, prompt.actionId) !== null);
        verify(testCase.findByText(prompt.contentItem, prompt.identity) !== null);
    }

    // PAM's own wording replaces this shell's placeholder as soon as it
    // arrives: what the stack asked for is more accurate than what this file
    // could guess.
    function test_pamsPromptReplacesThePlaceholder() {
        const field = testCase.findPasswordField(prompt.contentItem);
        verify(field !== null);
        const before = field.placeholderText;
        prompt.prompt = "Password for root:";
        compare(field.placeholderText, "Password for root:");
        verify(before !== field.placeholderText);
    }

    // The field never echoes, and what is typed leaves through `respond` and
    // nowhere else.
    function test_theAnswerGoesToTheSourceAndTheFieldClears() {
        const field = testCase.findPasswordField(prompt.contentItem);
        compare(field.echoMode, TextInput.Password);

        field.text = "a-passphrase";
        prompt.answer();
        compare(fakeSource.responses.length, 1);
        compare(fakeSource.responses[0], "a-passphrase");
        compare(field.text, "");

        // An empty field is not an answer. Sending one would spend the
        // person's single attempt on nothing.
        prompt.answer();
        compare(fakeSource.responses.length, 1);
    }

    // The card is tall enough for everything it holds. The first live prompt
    // was cut to its header: the sections declared `height` alone, a Column
    // sums its children's `implicitHeight`, and the card sized itself to the
    // one child that had one.
    function test_theWholeCardFitsWhatItShows() {
        const field = testCase.findPasswordField(prompt.contentItem);
        verify(field !== null);
        const bottom = field.mapToItem(null, 0, field.height).y;
        verify(bottom <= prompt.cardHeight,
               "the password field ends at " + bottom
               + " but the card is only " + prompt.cardHeight + " tall");
        const action = testCase.findByText(prompt.contentItem, prompt.actionId);
        verify(action.mapToItem(null, 0, action.height).y <= prompt.cardHeight);
    }

    // A problem from PAM is shown where the notice would be, and takes
    // precedence over it: what went wrong matters more than what was pending.
    function test_aProblemIsShownAndOutranksANotice() {
        prompt.notice = "One moment";
        verify(testCase.findByText(prompt.contentItem, "One moment") !== null);
        prompt.problem = "Authentication failure";
        verify(testCase.findByText(prompt.contentItem,
                                   "Authentication failure") !== null);
        verify(testCase.findByText(prompt.contentItem, "One moment") === null);
    }
}
