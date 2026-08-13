import QtQuick
import QtTest
import "../qml" as Desktop

// The measured-height card, pinned at the exact numbers that failed: this
// reproduces the "cardY stuck at zero" placement the hand-rolled cards hit
// twice, so the cause can be read instead of guessed at.
TestCase {
    id: testCase

    name: "SoftCardPlacement"
    when: windowShown
    visible: true
    width: 400
    height: 300

    function test_a_measured_card_is_still_placed() {
        const component = Qt.createComponent("../qml/CalendarMenu.qml");
        verify(component.status === Component.Ready, component.errorString());
        const card = component.createObject(null, {
            "reducedMotion": true,
            "outputName": "test-output",
            "shellScale": 1.0
        });
        verify(card);
        card.width = 1920;
        card.height = 1080;
        card.menuX = 1600;
        card.menuY = 1580;
        card.visible = true;

        console.warn("probe contentWidth", card.contentWidth,
                     "contentHeight", card.contentHeight,
                     "cardWidth", card.cardWidth,
                     "cardHeight", card.cardHeight,
                     "surfaceWidth", card.surfaceWidth,
                     "surfaceHeight", card.surfaceHeight,
                     "menuY", card.menuY,
                     "cardX", card.cardX, "cardY", card.cardY);

        verify(card.contentHeight > 0, "contentHeight " + card.contentHeight);
        compare(card.cardY, 1080 - card.cardHeight);
        card.destroy();
    }
}
