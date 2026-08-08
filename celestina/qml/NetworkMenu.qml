// What the session is on, and which saved networks it could be on instead.
//
// Everything here is the provider's truth or a request's own life. Nothing runs
// a process, nothing asks for a password, nothing creates or edits a profile,
// and no row paints the state it asked for: a profile becomes the active one on
// screen when a later provider snapshot says its UUID is active, never when the
// tool that was asked to join it exited zero.
//
// Rows are identified by `id` — NetworkManager's UUID — and never by name. A
// profile's label and the network it joins are different things that agree only
// by convention, and two saved profiles may carry the same name.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

AnchoredMenu {
    id: root

    required property var providerSource

    readonly property var network: ProviderReading.read(root.providerSource, "network")
    // The bounded inventory, or an empty list while there is none. The
    // difference between "no rows" and "no reading" is carried by the state
    // word below, never by the length of this.
    readonly property var rows: root.network !== undefined
                                && root.network.networks !== undefined
                                ? root.network.networks : []
    // `pending`, `fresh`, `held` or `unavailable` — and the empty string while
    // the provider has published nothing at all.
    readonly property string listState: root.network !== undefined
                                        && root.network.networksState !== undefined
                                        ? root.network.networksState : ""
    readonly property bool linkPresent: root.network !== undefined
                                        && root.network.kind !== undefined
    // The ledger lives on the bridge, not in this window: activating a row
    // closes the menu and destroys everything the window owned, and a result
    // that arrived then would answer nothing. Reopening shows what happened.
    readonly property var ledger: root.providerSource
                                  ? root.providerSource.requests : null
    readonly property bool refreshing: root.requestsPending("refresh")

    readonly property string linkLine: {
        if (root.network === undefined)
            return qsTr("Sin información de red");

        if (!root.linkPresent)
            return qsTr("Sin conexión");

        return root.network.kind === "ethernet"
               ? qsTr("Conectado por cable: %1").arg(root.network.connection)
               : qsTr("Conectado por Wi-Fi: %1").arg(root.network.connection);
    }

    // Said in full, because each word means something different about whether
    // the list below can be trusted or acted on.
    readonly property string listLine: {
        switch (root.listState) {
        case "fresh":
            return root.rows.length > 0
                   ? qsTr("Redes guardadas")
                   : qsTr("No hay redes guardadas");
        case "held":
            return qsTr("Redes guardadas (lectura anterior)");
        case "unavailable":
            return qsTr("No se puede consultar: falta NetworkManager");
        case "pending":
            return qsTr("Leyendo las redes guardadas…");
        default:
            return qsTr("Sin lectura de redes todavía");
        }
    }

    // One ordered stream, because a `Menu` has exactly one and mixing static
    // children with an `Instantiator` would leave their order to insertion
    // arithmetic. Every entry says what it is; the delegate reads that.
    readonly property var entries: {
        const built = [
            {"kind": "note", "text": root.linkLine},
            {"kind": "note", "text": root.listLine}
        ];
        const represented = {"refresh": true};
        for (let index = 0; index < root.rows.length; ++index)
        {
            built.push({"kind": "profile", "row": root.rows[index]});
            represented["activate-saved:" + root.rows[index].id] = true;
        }

        // A failed activation may remove the profile it named before this
        // menu is opened again. The durable ledger still knows what happened;
        // keep one dismissible report for every target that no current row can
        // carry, rather than making the failure disappear with that row.
        if (root.ledger && root.ledger.revision >= 0) {
            const failed = root.ledger.failures("network");
            for (let index = 0; index < failed.length; ++index) {
                if (represented[failed[index].target] !== true)
                    built.push({"kind": "failure", "failure": failed[index]});
            }
        }

        built.push({"kind": "refresh", "text": ""});
        return built;
    }

    title: qsTr("Menú de red")

    // The sentence beside a row. The ledger reports a typed cause; the Spanish
    // is decided here, because copy is the surface's business and the helper's
    // own reasons are English diagnostics that stay in the log.
    function noteFor(target) {
        if (!root.ledger || root.ledger.revision < 0)
            return "";

        const known = root.ledger.stateOf("network", target);
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

    // The menu's two actions, named. The delegate below is presentation: it
    // decides what a row looks like and calls one of these, so what a request
    // is has exactly one owner.
    //
    // `confirmed` is the contract: the helper answering `accepted` means a tool
    // ran, and only a later observation of the machine ends the wait.
    function activate(uuid) {
        if (root.ledger)
            root.ledger.send("network", "activate-saved", {"id": uuid}, "activate-saved:" + uuid, "confirmed");
    }

    function refresh() {
        if (root.ledger)
            root.ledger.send("network", "refresh", {}, "refresh", "confirmed");
    }

    // Whether a target is waiting on the machine. Reading `revision` is what
    // gives a binding a dependency that really moves when the ledger changes.
    function requestsPending(target) {
        return root.ledger !== null && root.ledger.revision >= 0
               && root.ledger.isPending("network", target);
    }

    Instantiator {
        model: root.entries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: GlassMenuItem {
            id: entry

            required property var modelData

            readonly property bool isProfile: entry.modelData.kind === "profile"
            readonly property bool isRefresh: entry.modelData.kind === "refresh"
            readonly property bool isFailure: entry.modelData.kind === "failure"
            readonly property var row: entry.isProfile ? entry.modelData.row : null
            readonly property string key: entry.isProfile
                                          ? "activate-saved:" + entry.row.id
                                          : (entry.isFailure
                                             ? entry.modelData.failure.target
                                             : "refresh")
            readonly property bool waiting: (entry.isProfile || entry.isRefresh)
                                            && root.requestsPending(entry.key)
            readonly property bool active: entry.isProfile && entry.row.active === true

            text: {
                if (entry.isRefresh)
                    return qsTr("Actualizar") + root.noteFor("refresh");

                if (entry.isProfile)
                    return entry.row.name + root.noteFor(entry.key);

                if (entry.isFailure)
                    return qsTr("No se pudo completar una acción anterior · descartar");

                return entry.modelData.text;
            }
            // Marks which profile the session is really on, and says so to
            // assistive technology as a mutually-exclusive state.
            choice: entry.isProfile
            current: entry.active
            // A note is a line to read, not a thing to do. The profile the
            // session is already on has nothing to ask for, and a row whose own
            // request is in flight must not be asked again — that is the
            // contradictory interaction the helper would refuse anyway.
            enabled: entry.isRefresh
                     ? !entry.waiting
                     : entry.isProfile && !entry.active && !entry.waiting
            Accessible.name: {
                if (entry.isRefresh)
                    return qsTr("Actualizar la lista de redes");

                if (!entry.isProfile)
                    return entry.isFailure
                           ? qsTr("Descartar el aviso de una acción de red fallida")
                           : entry.modelData.text;

                if (entry.active)
                    return qsTr("%1, red activa").arg(entry.row.name);

                return entry.waiting
                       ? qsTr("%1, conectando").arg(entry.row.name)
                       : qsTr("Conectar a %1").arg(entry.row.name);
            }
            Accessible.description: entry.isProfile && !entry.active
                                    ? qsTr("Activa este perfil guardado")
                                    : ""
            onTriggered: {
                if (entry.isRefresh) {
                    root.refresh();
                    return;
                }
                if (entry.isProfile)
                    root.activate(entry.row.id);
                else if (entry.isFailure && root.ledger)
                    root.ledger.forget("network", entry.key);
            }
        }

    }

}
