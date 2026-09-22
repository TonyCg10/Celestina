pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// Sensores: every chip as a card, every channel as a row, in a scrolling
// column. Every word is composed here from tokens; the kernel's labels are
// shown as they are.
Item {
    id: page

    required property HematitaSensors sensors

    property var cards: []

    function chipTitle(name, ordinal) {
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
        const suffix = ordinal > 1 ? " " + ordinal : ""
        return name === base ? base : base + suffix + " · " + name
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

    function number(kind, value) {
        const digits = kind === "fan" ? 0 : kind === "voltage" ? 3 : 1
        return value.toLocaleString(Qt.locale(), "f", digits)
    }

    function weave() {
        const s = page.sensors
        const chipCount = Math.min(s.chipKeys.length, s.chipNames.length, s.chipCounts.length)
        const channelCount = Math.min(s.channelChips.length, s.channelKinds.length, s.channelIndices.length,
                                      s.channelLabels.length, s.channelValues.length, s.channelMins.length,
                                      s.channelMaxs.length, s.channelLimitMax.length, s.channelLimitCrit.length,
                                      s.channelLoads.length)
        const ordinals = {}
        const woven = []
        for (let c = 0; c < chipCount; ++c) {
            const name = s.chipNames[c]
            ordinals[name] = (ordinals[name] || 0) + 1
            woven.push({ key: s.chipKeys[c], title: page.chipTitle(name, ordinals[name]), rows: [] })
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
                            ? qsTr("máx. %1 %2").arg(page.number(kind, s.channelLimitMax[i])).arg(u)
                            : ""
            woven[chip].rows.push({
                label: s.channelLabels[i].length > 0 ? s.channelLabels[i] : page.kindWord(kind, s.channelIndices[i]),
                valueText: page.number(kind, s.channelValues[i]) + " " + u,
                extremesText: qsTr("mín. %1 · máx. %2").arg(page.number(kind, s.channelMins[i])).arg(page.number(kind, s.channelMaxs[i])),
                limitText: limit,
                load: s.channelLoads[i]
            })
        }
        page.cards = woven
    }

    Connections {
        target: page.sensors
        function onRevisionChanged() { if (page.visible) page.weave() }
    }
    onVisibleChanged: if (page.visible) page.weave()
    Component.onCompleted: page.weave()

    Flickable {
        id: flick
        anchors.fill: parent
        anchors.rightMargin: CelestinaTheme.spaceLg
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Sensores")

        Column {
            id: column
            width: flick.width
            spacing: CelestinaTheme.spaceLg

            Repeater {
                model: page.cards.length

                SensorChipCard {
                    required property int index
                    readonly property var card: index < page.cards.length
                                                ? page.cards[index] : { title: "", rows: [] }
                    width: column.width
                    chipTitle: card.title
                    channels: card.rows
                }
            }

            Text {
                visible: !page.sensors.available
                text: qsTr("No se pudo leer %1").arg(page.sensors.reasonPath)
                color: CelestinaTheme.danger
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
            }
        }
    }

    CelestinaScrollBar {
        surface: flick
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        anchors.right: parent.right
    }
}
