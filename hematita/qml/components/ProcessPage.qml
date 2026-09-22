import QtQuick
import org.celestina.hematita 1.0

// Procesos: every process the kernel lists, flat.
Item {
    id: page

    required property HematitaProcesses processes
    required property Item backdrop

    ProcessTable {
        anchors.fill: parent
        processes: page.processes
        grouped: false
        backdrop: page.backdrop
    }
}
