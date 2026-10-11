pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Shapes
import org.celestina.calcita 1.0

// One page's form fields, laid over the page. The page's image shows every
// widget, stamped into the copy QtPdf draws (QtPdf draws no widget itself).
// While reading, a tap on a field asks the window to start editing on it.
// While editing, each text field and checkbox Calcita fills is a control
// over its widget (the copy leaves those widgets out), in the field's
// Helvetica, size and colour, in the page's points scaled with the page; its
// value goes to the editor when it is committed (the keyboard leaves it,
// Enter on one line, Tab, Escape), a value the editor refuses (it says why)
// giving way to the field's own again; Tab and Shift+Tab go on to the next field in
// the form's order, on any page. A field Calcita only shows (radio buttons,
// lists, signatures, read-only fields) says why when tapped. Everything is
// in the displayed page's points, as the editor publishes it; the editor
// decides what a value means.
Item {
    id: form

    // Every widget of the form, as the editor (or, while reading, the
    // document) publishes them; a fill goes to the editor by the field's id.
    required property var fields
    required property CalcitaEditor editor
    required property bool editing
    // The fields take the pointer (not while highlighting or drawing).
    required property bool interactive
    // The widget (its `order`) the window wants to have the keyboard, while
    // `focusPending`; `focusSerial` changes with each ask.
    required property int focusOrder
    required property bool focusPending
    required property int focusSerial
    // The space between a field's edge and its text, in points, as the
    // saved appearance keeps it.
    required property real textPadding
    // Set by the page that loads the layer.
    property int page: -1
    property real renderScale: 1

    // While reading: the field at `order` was tapped.
    signal activated(int order)
    // A field Calcita only shows was tapped: what to say.
    signal noticeRequested(string text)
    // Tab (`forward`) or Shift+Tab from the field at `order`.
    signal tabbed(int order, bool forward)
    // The control of the field at `order` took the keyboard.
    signal focused(int order)
    // Escape in a field or on a checkbox: the keyboard goes back to the
    // pages.
    signal doneTyping()

    // This page's widgets. The repeater counts them, so a new value (the
    // same widgets) updates each control in place instead of building it
    // again, and the one being typed in keeps the keyboard.
    readonly property var items: form.page < 0 ? [] : form.fields.filter(entry => entry.page === form.page)

    objectName: "formLayer" + form.page

    Repeater {
        model: form.items.length

        delegate: Item {
            id: box

            required property int index
            readonly property var entry: form.items[box.index] !== undefined ? form.items[box.index] : ({})
            readonly property bool control: form.editing && box.entry.fillable === true
            readonly property bool isText: box.control && box.entry.kind === "text"
            readonly property bool isCheck: box.control && box.entry.kind === "checkbox"
            readonly property bool multiline: box.entry.multiline === true
            readonly property bool hasKeyboard: singleLine.activeFocus || lines.activeFocus || check.activeFocus

            // The control takes the keyboard when the window asks for it.
            function claim() {
                if (!form.focusPending || form.focusOrder !== box.entry.order || !box.control)
                    return
                if (box.isCheck)
                    check.forceActiveFocus()
                else if (box.multiline)
                    lines.forceActiveFocus()
                else
                    singleLine.forceActiveFocus()
            }

            // What the control shows, from the published value, unless it
            // is being typed in.
            function refresh() {
                if (!singleLine.activeFocus)
                    singleLine.text = box.entry.value !== undefined ? box.entry.value : ""
                if (!lines.activeFocus)
                    lines.text = box.entry.value !== undefined ? box.entry.value : ""
            }

            // The control shows the field's value again, typed over or not.
            function revert() {
                const value = box.entry.value !== undefined ? box.entry.value : ""
                singleLine.text = value
                lines.text = value
            }

            // The typed text goes to the editor at once, so leaving or
            // closing right after typing finds it in the edit set; the new
            // value updates the fields in place (the repeater counts them).
            // A refused text (the editor's notice says why) is not kept.
            function commit(text) {
                if (box.control && text !== box.entry.value
                        && !form.editor.fillText(box.entry.field, text))
                    box.revert()
            }

            // A tap while reading, or on a field Calcita only shows.
            function press() {
                if (box.entry.fillable === true)
                    form.activated(box.entry.order)
                else
                    form.noticeRequested(box.entry.notice)
            }

            function toggle() {
                if (box.isCheck)
                    form.editor.setChecked(box.entry.field, !box.entry.checked)
            }

            function tab(text, forward) {
                box.commit(text)
                form.tabbed(box.entry.order, forward)
            }

            objectName: "formField_" + box.entry.order
            x: box.entry.x * form.renderScale
            y: box.entry.y * form.renderScale
            width: box.entry.w * form.renderScale
            height: box.entry.h * form.renderScale
            enabled: form.interactive

            onEntryChanged: box.refresh()
            onControlChanged: claimLater.restart()
            Component.onCompleted: {
                box.refresh()
                box.claim()
            }

            Connections {
                target: form
                function onFocusSerialChanged() { box.claim() }
            }

            // A control that has just appeared claims the keyboard once its
            // own bindings have followed; the timer goes with the field.
            Timer {
                id: claimLater
                interval: 0
                onTriggered: box.claim()
            }

            // Reading, or a field Calcita only shows: a tap.
            Item {
                anchors.fill: parent
                visible: !box.control
                Accessible.role: Accessible.Button
                Accessible.name: box.entry.label !== undefined ? box.entry.label : ""
                Accessible.description: box.entry.fillable === true ? "" : box.entry.notice
                Accessible.onPressAction: box.press()

                HoverHandler {
                    enabled: !box.control
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    enabled: !box.control
                    gesturePolicy: TapHandler.ReleaseWithinBounds
                    onTapped: box.press()
                }
            }

            // The field's place, tinted as a field the person can fill;
            // framed while it has the keyboard.
            Rectangle {
                objectName: "formFieldFrame"
                anchors.fill: parent
                visible: box.control
                color: CelestinaTheme.accentSoft
                radius: CelestinaTheme.radiusNone
                border.width: box.hasKeyboard ? CelestinaTheme.borderFocus : CelestinaTheme.borderHairline
                border.color: box.hasKeyboard ? CelestinaTheme.focusRing : CelestinaTheme.accentSoftBorder
            }

            // A text field in the page's points, scaled with the page.
            Item {
                id: sheet
                visible: box.isText
                width: box.entry.w !== undefined ? box.entry.w : 0
                height: box.entry.h !== undefined ? box.entry.h : 0
                scale: form.renderScale
                transformOrigin: Item.TopLeft
                clip: true

                TextInput {
                    id: singleLine
                    objectName: "formText"
                    visible: !box.multiline
                    enabled: box.isText && !box.multiline
                    anchors.fill: parent
                    leftPadding: CelestinaTheme.borderHairline + form.textPadding
                    rightPadding: CelestinaTheme.borderHairline + form.textPadding
                    verticalAlignment: TextInput.AlignVCenter
                    horizontalAlignment: box.entry.align === "center" ? TextInput.AlignHCenter
                                         : box.entry.align === "right" ? TextInput.AlignRight
                                         : TextInput.AlignLeft
                    color: box.entry.color !== undefined ? box.entry.color : CelestinaTheme.clear
                    // The standard font the saved appearance names.
                    font.family: "Helvetica"
                    font.pixelSize: Math.max(1, box.entry.size !== undefined ? box.entry.size : 12)
                    maximumLength: box.entry.maxLen > 0 ? box.entry.maxLen : 32767
                    selectByMouse: true
                    selectionColor: CelestinaTheme.accentSoft
                    selectedTextColor: singleLine.color
                    activeFocusOnTab: true
                    Accessible.role: Accessible.EditableText
                    Accessible.name: box.entry.label !== undefined ? box.entry.label : ""
                    onActiveFocusChanged: {
                        if (singleLine.activeFocus)
                            form.focused(box.entry.order)
                        else
                            box.commit(singleLine.text)
                    }
                    onAccepted: box.commit(singleLine.text)
                    Keys.onTabPressed: box.tab(singleLine.text, true)
                    Keys.onBacktabPressed: box.tab(singleLine.text, false)
                    Keys.onEscapePressed: {
                        box.commit(singleLine.text)
                        form.doneTyping()
                    }
                }

                TextEdit {
                    id: lines
                    objectName: "formText"
                    visible: box.multiline
                    enabled: box.isText && box.multiline
                    anchors.fill: parent
                    padding: CelestinaTheme.borderHairline + form.textPadding
                    horizontalAlignment: box.entry.align === "center" ? TextEdit.AlignHCenter
                                         : box.entry.align === "right" ? TextEdit.AlignRight
                                         : TextEdit.AlignLeft
                    color: box.entry.color !== undefined ? box.entry.color : CelestinaTheme.clear
                    font.family: "Helvetica"
                    font.pixelSize: Math.max(1, box.entry.size !== undefined ? box.entry.size : 12)
                    wrapMode: TextEdit.Wrap
                    selectByMouse: true
                    selectionColor: CelestinaTheme.accentSoft
                    selectedTextColor: lines.color
                    activeFocusOnTab: true
                    Accessible.role: Accessible.EditableText
                    Accessible.name: box.entry.label !== undefined ? box.entry.label : ""
                    onActiveFocusChanged: {
                        if (lines.activeFocus)
                            form.focused(box.entry.order)
                        else
                            box.commit(lines.text)
                    }
                    Keys.onTabPressed: box.tab(lines.text, true)
                    Keys.onBacktabPressed: box.tab(lines.text, false)
                    Keys.onEscapePressed: {
                        box.commit(lines.text)
                        form.doneTyping()
                    }
                }
            }

            // A checkbox: Space or a tap toggles it; its mark in the
            // document's colour, like the widget's own.
            Item {
                id: check
                objectName: "formCheck"
                anchors.fill: parent
                visible: box.isCheck
                enabled: box.isCheck
                activeFocusOnTab: true
                Accessible.role: Accessible.CheckBox
                Accessible.name: box.entry.label !== undefined ? box.entry.label : ""
                Accessible.checkable: true
                Accessible.checked: box.entry.checked === true
                Accessible.onToggleAction: box.toggle()
                Accessible.onPressAction: box.toggle()
                onActiveFocusChanged: {
                    if (check.activeFocus)
                        form.focused(box.entry.order)
                }
                // Space is the window's next page otherwise.
                Keys.onShortcutOverride: event => event.accepted = event.key === Qt.Key_Space
                Keys.onSpacePressed: box.toggle()
                Keys.onTabPressed: box.tab("", true)
                Keys.onBacktabPressed: box.tab("", false)
                Keys.onEscapePressed: form.doneTyping()

                TapHandler {
                    enabled: box.isCheck
                    gesturePolicy: TapHandler.ReleaseWithinBounds
                    onTapped: {
                        check.forceActiveFocus()
                        box.toggle()
                    }
                }

                // The check, a cross like the widget's own.
                Shape {
                    objectName: "formCheckMark"
                    anchors.fill: parent
                    anchors.margins: parent.width / 5
                    visible: box.entry.checked === true

                    ShapePath {
                        strokeColor: box.entry.color !== undefined ? box.entry.color : CelestinaTheme.clear
                        strokeWidth: Math.max(1, form.renderScale * 2)
                        fillColor: CelestinaTheme.clear
                        capStyle: ShapePath.RoundCap
                        startX: 0
                        startY: 0
                        PathLine { x: check.width * 3 / 5; y: check.height * 3 / 5 }
                        PathMove { x: 0; y: check.height * 3 / 5 }
                        PathLine { x: check.width * 3 / 5; y: 0 }
                    }
                }
            }
        }
    }
}
