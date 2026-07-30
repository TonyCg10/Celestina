import QtQuick

Item {
    property string marker: "/* (not a comment) */"

    // Controls.Button { must not count from a comment.
    // delegate: Controls.Button { must not count either.
    /*
       sourceComponent: Controls.Button {
       }
    */
    function appendPoint(x) {
        points.append({x: x})
    }
}
