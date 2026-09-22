import QtQuick
import org.celestina.hematita 1.0

// Aplicaciones: the same processes, gathered under the application that
// launched them.
Item {
    id: page

    required property HematitaProcesses processes
    required property Item backdrop

    ProcessTable {
        anchors.fill: parent
        processes: page.processes
        grouped: true
        backdrop: page.backdrop
    }
}
