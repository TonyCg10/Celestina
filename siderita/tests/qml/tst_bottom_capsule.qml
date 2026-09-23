import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The bottom capsule: what is shown, how it is arranged, how big it is — three
// ghost icons on one glass surface. The eye is a toggle whose state must keep
// following the controller after it has been clicked; the middle icon wears the
// current view mode and opens the menu; the magnifier now sits on the left, so
// its popup opens to the right of its own edge instead of off the window.
TestCase {
    id: testCase
    name: "BottomCapsule"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property int hiddenToggles: 0

    // The window's own sizing properties, which the capsule's popup edits.
    property real contentIconScale: 1
    property real contentTextScale: 1
    property real interfaceIconScale: 1
    property real interfaceTextScale: 1
    property real sidebarIconScale: 1
    property real sidebarTextScale: 1
    function persistSizing() { }

    QtObject {
        id: controllerStub
        property bool showHidden: false
        property int sortField: 0
        property bool sortAscending: true
        function toggleHidden() {
            testCase.hiddenToggles++
            controllerStub.showHidden = !controllerStub.showHidden
        }
        function changeSortField(field) { controllerStub.sortField = field }
        function toggleSortDirection() {
            controllerStub.sortAscending = !controllerStub.sortAscending
        }
    }

    QtObject {
        id: panelStub
        property string viewMode: "grid"
        function persist() { }
    }

    Item {
        id: overlayStub
        anchors.fill: parent
    }

    ViewSortMenu {
        id: menuStub
        controller: controllerStub
        panel: panelStub
        backdropSource: overlayStub
    }

    BottomControls {
        id: capsule
        x: 10
        y: 340
        controller: controllerStub
        panel: panelStub
        bottomView: overlayStub
        bottomFloating: true
        overlayParent: overlayStub
        viewSortMenu: menuStub
        hostWindow: testCase
    }

    function init() {
        testCase.hiddenToggles = 0
        controllerStub.showHidden = false
        panelStub.viewMode = "grid"
        menuStub.close()
        capsule.sizePopup.close()
        mouseMove(testCase, 590, 10)
    }

    function test_capsule_is_one_surface_of_three_icons() {
        compare(capsule.icons.children.length, 3,
                "the capsule holds exactly three icons")
        verify(capsule.implicitWidth < 130,
               "three icons and their padding stay well under the 330 the four "
               + "pills took: " + capsule.implicitWidth)
    }

    function test_eye_keeps_following_the_controller_after_a_click() {
        const eye = capsule.hiddenButton
        mouseClick(eye)
        compare(testCase.hiddenToggles, 1)
        compare(eye.checked, true, "the eye is on after the first click")
        mouseClick(eye)
        compare(testCase.hiddenToggles, 2)
        compare(eye.checked, false,
                "the binding to showHidden survived the button's own toggle")
    }

    function test_middle_icon_wears_the_current_view_mode() {
        compare(capsule.viewSortButton.iconName, "view-grid")
        panelStub.viewMode = "list"
        compare(capsule.viewSortButton.iconName, "view-list")
        panelStub.viewMode = "details"
        compare(capsule.viewSortButton.iconName, "view-details")
    }

    function test_sizes_popup_opens_inside_the_window_from_the_left() {
        const popup = capsule.sizePopup
        mouseClick(capsule.sizeButton)
        verify(popup.opened, "pressing the magnifier opens the sizes popup")
        const origin = capsule.sizeButton.mapToItem(testCase, 0, 0)
        verify(origin.x + popup.width <= testCase.width,
               "the popup grows rightwards from a button on the left and stays "
               + "inside the window: " + (origin.x + popup.width))
    }
}
