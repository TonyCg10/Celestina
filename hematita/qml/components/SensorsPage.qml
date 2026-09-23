pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// Sensores: every chip as a card, every channel as a row, in one list the
// keyboard crosses by card. Every word is composed here from tokens; a
// kernel label this page knows is put into words, any other is shown as it is.
Item {
    id: page

    required property HematitaSensors sensors

    property var cards: []

    // `ordinal` is which chip of this driver name this is, `total` how many
    // the machine has. A driver that appears once needs no number; one that
    // appears twice numbers both, because "Disco NVMe" beside "Disco NVMe 2"
    // reads as if the first were the only one.
    function chipTitle(name, ordinal, total) {
        let base
        switch (name) {
        case "k10temp": base = qsTr("Procesador"); break
        case "amdgpu": base = qsTr("Gráfica"); break
        case "nvme": base = qsTr("Disco NVMe"); break
        case "it8696": base = qsTr("Placa base"); break
        case "gigabyte_wmi": base = qsTr("Placa base (WMI)"); break
        case "acpitz": base = qsTr("ACPI"); break
        case "ath12k_hwmon": base = qsTr("Wi-Fi"); break
        default: base = name
        }
        const suffix = total > 1 ? " " + ordinal : ""
        return name === base ? base + suffix : base + suffix + " · " + name
    }

    function kindWord(kind, index) {
        switch (kind) {
        case "temperature": return qsTr("Temperatura %1").arg(index)
        case "fan": return qsTr("Ventilador %1").arg(index)
        case "voltage": return qsTr("Tensión %1").arg(index)
        case "power": return qsTr("Potencia %1").arg(index)
        case "current": return qsTr("Corriente %1").arg(index)
        }
        return kind + " " + index
    }

    // The hwmon labels this machine's drivers publish, in words. A label not
    // listed here is shown raw: it is the driver's own name for the channel.
    function labelWord(label) {
        switch (label) {
        case "Tctl": return qsTr("CPU (control)")
        case "Tccd1": return qsTr("CPU chiplet 1")
        case "Composite": return qsTr("Compuesta")
        case "Sensor 1": return qsTr("Sensor 1")
        case "Sensor 2": return qsTr("Sensor 2")
        case "edge": return qsTr("Borde")
        case "junction": return qsTr("Unión")
        case "mem": return qsTr("Memoria")
        case "vddgfx": return qsTr("Tensión del núcleo")
        case "PPT": return qsTr("Potencia total")
        case "3VSB": return qsTr("3,3 V en espera")
        case "Vbat": return qsTr("Pila")
        case "+3.3V": return qsTr("3,3 V")
        }
        return label
    }

    function unit(kind) {
        switch (kind) {
        case "temperature": return "°C"
        case "fan": return "rpm"
        case "voltage": return "V"
        case "power": return "W"
        case "current": return "A"
        }
        return ""
    }

    // A reading in the locale's decimal mark and without a thousands
    // separator: "3,650 rpm" reads as three and a bit in a comma-decimal
    // locale. `toLocaleString` always groups, so the digits are fixed here and
    // only the decimal mark is the locale's.
    function number(kind, value) {
        const digits = kind === "fan" ? 0 : kind === "voltage" ? 3 : 1
        return value.toFixed(digits).replace(".", Qt.locale().decimalPoint)
    }

    function weave() {
        const s = page.sensors
        const chipCount = Math.min(s.chipKeys.length, s.chipNames.length, s.chipCounts.length)
        const channelCount = Math.min(s.channelChips.length, s.channelKinds.length, s.channelIndices.length,
                                      s.channelLabels.length, s.channelValues.length, s.channelMins.length,
                                      s.channelMaxs.length, s.channelLimitMax.length, s.channelLimitCrit.length,
                                      s.channelLoads.length)
        // A plain object literal inherits `Object.prototype`, so a driver
        // literally named `constructor` or `toString` would read as a count;
        // a null-prototype object is a map and nothing else.
        const totals = Object.create(null)
        for (let c = 0; c < chipCount; ++c) {
            const name = s.chipNames[c]
            totals[name] = (totals[name] || 0) + 1
        }
        const ordinals = Object.create(null)
        const woven = []
        for (let c = 0; c < chipCount; ++c) {
            const name = s.chipNames[c]
            ordinals[name] = (ordinals[name] || 0) + 1
            woven.push({ key: s.chipKeys[c],
                         title: page.chipTitle(name, ordinals[name], totals[name]),
                         rows: [] })
        }
        for (let i = 0; i < channelCount; ++i) {
            const chip = s.channelChips[i]
            if (chip < 0 || chip >= woven.length)
                continue
            const kind = s.channelKinds[i]
            const u = page.unit(kind)
            const limit = s.channelLimitCrit[i] > 0
                          ? qsTr("crítico %1 %2").arg(page.number(kind, s.channelLimitCrit[i])).arg(u)
                          : s.channelLimitMax[i] > 0
                            ? qsTr("límite %1 %2").arg(page.number(kind, s.channelLimitMax[i])).arg(u)
                            : ""
            woven[chip].rows.push({
                label: s.channelLabels[i].length > 0 ? page.labelWord(s.channelLabels[i])
                                                     : page.kindWord(kind, s.channelIndices[i]),
                valueText: page.number(kind, s.channelValues[i]) + " " + u,
                extremesText: qsTr("mín. %1 · máx. %2").arg(page.number(kind, s.channelMins[i])).arg(page.number(kind, s.channelMaxs[i])),
                limitText: limit,
                load: s.channelLoads[i],
                kind: kind
            })
        }
        // The model is the card count, so a hot-plug is a reset that puts
        // the view back at the top; the offset read before it is put back,
        // clamped to the new length. The reset also rewrites the cursor, and
        // `rebuilding` keeps that from scrolling as if the person had moved.
        page.rebuilding = true
        const offset = list.contentY
        page.cards = woven
        list.forceLayout()
        const reach = Math.max(0, list.contentHeight - list.height)
        list.contentY = list.originY + Math.min(Math.max(0, offset - list.originY), reach)
        page.rebuilding = false
    }

    property bool rebuilding: false

    Connections {
        target: page.sensors
        function onRevisionChanged() { if (page.visible) page.weave() }
    }
    onVisibleChanged: if (page.visible) page.weave()
    Component.onCompleted: page.weave()

    // A card-shaped nothing, for a delegate whose index is momentarily past
    // the woven array.
    readonly property var emptyCard: ({ title: "", rows: [] })

    Text {
        id: note

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        visible: !page.sensors.available
        text: qsTr("No se pudo leer %1").arg(page.sensors.reasonPath)
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.WordWrap
    }

    // A list, not a scrolling column of focusable rows: the page is one Tab
    // stop and the arrows move it card by card, so every chip is reachable
    // without forty-six stops and without a focus stranded off-screen where
    // nothing can scroll to it. The delegates are the cards; the model is
    // their count, so a tick that changes no chip leaves them where they are.
    ListView {
        id: list

        anchors.fill: parent
        anchors.topMargin: note.visible ? note.implicitHeight + CelestinaTheme.spaceLg : 0
        anchors.rightMargin: CelestinaTheme.spaceLg
        clip: true
        spacing: CelestinaTheme.spaceLg
        model: page.cards.length
        activeFocusOnTab: true
        keyNavigationEnabled: true
        // Only the person's arrows move the view, never a rebuild.
        highlightFollowsCurrentItem: false
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Sensores")

        onCurrentIndexChanged: {
            if (!page.rebuilding && list.currentIndex >= 0)
                list.positionViewAtIndex(list.currentIndex, ListView.Contain)
        }

        delegate: SensorChipCard {
            required property int index
            readonly property var card: index < page.cards.length
                                        ? page.cards[index] : page.emptyCard

            width: list.width
            chipTitle: card.title
            channels: card.rows
        }
    }

    // Beside the list, not attached to it: a child of the list would scroll
    // away with the cards.
    CelestinaScrollBar {
        surface: list
        anchors.top: list.top
        anchors.bottom: list.bottom
        anchors.right: parent.right
    }
}
