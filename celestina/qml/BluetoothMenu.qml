// The adapter, and the devices this session already knows.
//
// The same rule as every other control in this shell: nothing here paints what
// it asked for. The adapter row shows the power the provider last reported and
// a device row shows the connection BlueZ last confirmed, so a request that
// fails leaves the state where it really is instead of where it was aimed.
//
// Read and switch only. No discovery is started, nothing is paired, forgotten
// or trusted, and no PIN is ever asked for — those need decisions and secrets
// that belong to BlueZ's own agent.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

AnchoredMenu {
    id: root

    required property var providerSource

    readonly property var bluetooth: ProviderReading.read(root.providerSource, "bluetooth")
    // `on`, `off`, `absent`, or empty while nothing has been published — an
    // adapter nobody could read withdraws the provider entirely.
    readonly property string adapter: root.bluetooth !== undefined
                                      && root.bluetooth.adapter !== undefined
                                      ? root.bluetooth.adapter : ""
    readonly property var rows: root.bluetooth !== undefined
                                && root.bluetooth.devices !== undefined
                                ? root.bluetooth.devices : []
    readonly property string listState: root.bluetooth !== undefined
                                        && root.bluetooth.devicesState !== undefined
                                        ? root.bluetooth.devicesState : ""
    readonly property bool powered: root.adapter === "on"
    // A machine with no controller has nothing to switch. One that is merely
    // off does, and that is the distinction the switch depends on.
    readonly property bool switchable: root.adapter === "on" || root.adapter === "off"
    // See `NetworkMenu`: the ledger outlives this window because activating a
    // row destroys it.
    readonly property var ledger: root.providerSource
                                  ? root.providerSource.requests : null
    readonly property bool refreshing: root.requestsPending("refresh")
    readonly property bool switching: root.requestsPending("set-powered")

    readonly property string adapterLine: {
        switch (root.adapter) {
        case "on":
            return qsTr("Bluetooth encendido");
        case "off":
            return qsTr("Bluetooth apagado");
        case "absent":
            return qsTr("Este equipo no tiene Bluetooth");
        default:
            return qsTr("Sin información de Bluetooth");
        }
    }

    readonly property string listLine: {
        // A radio that is off has nothing on it, and saying "no devices" there
        // would read as a fact about the devices rather than about the switch.
        if (!root.powered)
            return "";

        switch (root.listState) {
        case "fresh":
            return root.rows.length > 0
                   ? qsTr("Dispositivos conocidos")
                   : qsTr("No hay dispositivos conocidos");
        case "held":
            return qsTr("Dispositivos conocidos (lectura anterior)");
        case "unavailable":
            return qsTr("No se puede consultar: falta bluetoothctl");
        case "pending":
            return qsTr("Leyendo los dispositivos…");
        default:
            return qsTr("Sin lectura de dispositivos todavía");
        }
    }

    // One ordered stream: the switch, a line about the list, the devices, and
    // the refresh. A `Menu` has exactly one, and mixing static children with an
    // `Instantiator` would leave their order to insertion arithmetic.
    readonly property var entries: {
        const built = [{"kind": "adapter", "text": ""}];
        const represented = {"set-powered": true, "refresh": true};
        if (root.listLine.length > 0)
            built.push({"kind": "note", "text": root.listLine});

        if (root.powered) {
            for (let index = 0; index < root.rows.length; ++index) {
                built.push({"kind": "device", "row": root.rows[index]});
                represented["device:" + root.rows[index].id] = true;
            }
        }
        // A device may disappear from BlueZ's next inventory before its
        // request fails. Preserve that result as a dismissible report instead
        // of requiring the vanished row to display it.
        if (root.ledger && root.ledger.revision >= 0) {
            const failed = root.ledger.failures("bluetooth");
            for (let index = 0; index < failed.length; ++index) {
                if (represented[failed[index].target] !== true)
                    built.push({"kind": "failure", "failure": failed[index]});
            }
        }
        built.push({"kind": "refresh", "text": ""});
        return built;
    }

    title: qsTr("Menú de Bluetooth")

    // The sentence beside a row. The ledger reports a typed cause; the Spanish
    // is decided here, and the helper's English reasons stay in the log.
    function noteFor(target) {
        if (!root.ledger || root.ledger.revision < 0)
            return "";

        const known = root.ledger.stateOf("bluetooth", target);
        if (known.state === undefined)
            return "";

        if (known.state === "pending")
            return qsTr(" — solicitando…");

        if (known.state !== "failed")
            return "";

        switch (known.cause) {
        case "unsent":
            return qsTr(" — no se pudo: el shell no pudo enviarlo");
        case "generation-lost":
            return qsTr(" — no se pudo: el asistente se reinició");
        default:
            return qsTr(" — no se pudo");
        }
    }

    // The menu's three actions, named. The delegate below is presentation: it
    // decides what a row looks like and calls one of these, so what a request
    // is has exactly one owner.
    //
    // `setPowered` and `toggleDevice` take the state to ask for rather than
    // reading it themselves, because what is on screen is the provider's answer
    // and the caller is the thing that knows which row was pressed.
    function setPowered(wanted) {
        if (root.ledger)
            root.ledger.send("bluetooth", "set-powered", {"powered": wanted}, "set-powered", "confirmed");
    }

    function toggleDevice(address, connected) {
        if (!root.ledger)
            return;

        root.ledger.send(
            "bluetooth",
            connected ? "disconnect-known" : "connect-known",
            {"id": address},
            "device:" + address,
            "confirmed"
        );
    }

    function refresh() {
        if (root.ledger)
            root.ledger.send("bluetooth", "refresh", {}, "refresh", "confirmed");
    }

    // Whether a target is waiting on the machine. Reading `revision` is what
    // gives a binding a dependency that really moves when the ledger changes.
    function requestsPending(target) {
        return root.ledger !== null && root.ledger.revision >= 0
               && root.ledger.isPending("bluetooth", target);
    }

    Instantiator {
        model: root.entries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: GlassMenuItem {
            id: entry

            required property var modelData

            readonly property bool isAdapter: entry.modelData.kind === "adapter"
            readonly property bool isDevice: entry.modelData.kind === "device"
            readonly property bool isRefresh: entry.modelData.kind === "refresh"
            readonly property bool isFailure: entry.modelData.kind === "failure"
            readonly property var row: entry.isDevice ? entry.modelData.row : null
            readonly property string key: {
                if (entry.isAdapter)
                    return "set-powered";

                if (entry.isDevice)
                    return "device:" + entry.row.id;

                if (entry.isFailure)
                    return entry.modelData.failure.target;

                return "refresh";
            }
            readonly property bool waiting: root.requestsPending(entry.key)
            readonly property bool connected: entry.isDevice && entry.row.connected === true

            text: {
                if (entry.isAdapter)
                    return root.adapterLine + root.noteFor(entry.key);

                if (entry.isRefresh)
                    return qsTr("Actualizar") + root.noteFor(entry.key);

                if (entry.isDevice)
                    return entry.row.name + root.noteFor(entry.key);

                if (entry.isFailure)
                    return qsTr("No se pudo completar una acción anterior · descartar");

                return entry.modelData.text;
            }
            // Both the switch and a device show a state the provider confirmed,
            // never the one that was requested.
            choice: entry.isAdapter || entry.isDevice
            current: entry.isAdapter ? root.powered : entry.connected
            enabled: {
                if (entry.isAdapter)
                    return root.switchable && !entry.waiting;

                if (entry.isRefresh)
                    return !entry.waiting;

                // A device whose own request is in flight cannot be asked for
                // the opposite at the same time, and neither can one while the
                // adapter it lives on is being switched.
                return entry.isDevice && !entry.waiting && !root.switching;
            }
            Accessible.name: {
                if (entry.isAdapter) {
                    if (!root.switchable)
                        return root.adapterLine;

                    return root.powered
                           ? qsTr("Apagar el Bluetooth")
                           : qsTr("Encender el Bluetooth");
                }
                if (entry.isRefresh)
                    return qsTr("Actualizar la lista de dispositivos");

                if (!entry.isDevice)
                    return entry.isFailure
                           ? qsTr("Descartar el aviso de una acción de Bluetooth fallida")
                           : entry.modelData.text;

                if (entry.waiting) {
                    return entry.connected
                           ? qsTr("%1, desconectando").arg(entry.row.name)
                           : qsTr("%1, conectando").arg(entry.row.name);
                }
                return entry.connected
                       ? qsTr("Desconectar %1").arg(entry.row.name)
                       : qsTr("Conectar %1").arg(entry.row.name);
            }
            Accessible.description: {
                if (entry.isDevice) {
                    return entry.connected
                           ? qsTr("Desconecta este dispositivo conocido")
                           : qsTr("Conecta este dispositivo conocido");
                }
                if (entry.isAdapter && root.switchable)
                    return qsTr("Enciende o apaga el adaptador");

                return "";
            }
            onTriggered: {
                if (entry.isAdapter) {
                    root.setPowered(!root.powered);
                    return;
                }
                if (entry.isRefresh) {
                    root.refresh();
                    return;
                }
                if (entry.isDevice)
                    root.toggleDevice(entry.row.id, entry.connected);
                else if (entry.isFailure && root.ledger)
                    root.ledger.forget("bluetooth", entry.key);
            }
        }

    }

}
