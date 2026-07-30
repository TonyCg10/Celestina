import QtQuick
import QtQuick.Controls as Controls

Item {
    component LocalButton: Controls.Button {
        text: "inline component"
    }

    property Component buttonFactory: Controls.Button {
        text: "component property"
    }

    Loader {
        sourceComponent: Controls.Button {
            text: "object-valued property"
        }
    }

    ListView {
        delegate: Controls.Button {
            text: "delegate property"
        }
    }
}
