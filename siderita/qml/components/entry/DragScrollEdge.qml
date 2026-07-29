import QtQuick
import org.celestina.siderita 1.0

// ─── DragScrollEdge ───────────────────────────────────────────────────────────
// El borde que desplaza la vista mientras se arrastra algo sobre él. Sin esto,
// arrastrar a una carpeta que no se ve exige soltar, desplazar y volver a
// coger. No sabe nada de lo que se arrastra: sólo de la vista y del paso.
// ──────────────────────────────────────────────────────────────────────────────
DropArea {
    id: edge

    required property Flickable view
    required property int step

    signal externalDrop(var drop)

    z: 6
    height: 30

    Timer {
        running: edge.containsDrag && edge.view !== null
        interval: 16
        repeat: true
        onTriggered: {
            const minimum = edge.view.originY - edge.view.topMargin
            const maximum = Math.max(
                minimum, edge.view.originY + edge.view.contentHeight
                         - edge.view.height)
            edge.view.contentY = Math.max(
                minimum, Math.min(maximum, edge.view.contentY + edge.step))
        }
    }

    onDropped: function(drop) {
        if (drop.hasUrls)
            edge.externalDrop(drop)
    }
}
