pragma Singleton

import QtQuick

// The one way Hematita's pages word an amount of bytes: binary units up to
// TiB, one decimal from MiB up and none below, in the locale's notation. The
// unit symbols read the same in every language; the bytes line goes through
// qsTr() so a translation can place the number.
QtObject {
    id: units

    function size(bytes) {
        if (bytes >= 1099511627776) return (bytes / 1099511627776).toLocaleString(Qt.locale(), "f", 1) + " TiB"
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return qsTr("%1 B").arg(Math.round(bytes))
    }

    // An amount per second, in the same units.
    function rate(bytesPerSecond) {
        return units.size(bytesPerSecond) + "/s"
    }
}
