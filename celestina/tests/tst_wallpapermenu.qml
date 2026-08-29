import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

TestCase {
    id: testCase

    name: "WallpaperMenu"

    QtObject {
        id: fakeLedger

        property int revision: 0
        property var state: ({})
        property int sendCount: 0
        property string sentProvider: ""
        property string sentVerb: ""
        property var sentOptions: ({})
        property string sentTarget: ""
        property string sentPolicy: ""

        function stateOf(provider, target) {
            return fakeLedger.state;
        }

        function send(provider, verb, options, target, policy) {
            fakeLedger.sendCount += 1;
            fakeLedger.sentProvider = provider;
            fakeLedger.sentVerb = verb;
            fakeLedger.sentOptions = options;
            fakeLedger.sentTarget = target;
            fakeLedger.sentPolicy = policy;
        }
    }

    QtObject {
        id: fakeSource

        property int revision: 1
        property var requests: fakeLedger
        property var providers: ({
            "wallpaper-gallery": {
                "state": "ready",
                "folder": "/pictures/walls",
                "folderUrl": "file:///pictures/walls",
                "catalogue": "7",
                "page": 1,
                "pageCount": 2,
                "total": 65,
                "hasPrevious": false,
                "hasNext": true,
                "images": [
                    {
                        "id": "first",
                        "name": "primero.png",
                        "previewUrl": "file:///pictures/walls/primero.png",
                        "revision": "1"
                    },
                    {
                        "id": "second",
                        "name": "segundo.jpg",
                        "previewUrl": "file:///pictures/walls/segundo.jpg",
                        "revision": "2"
                    }
                ],
                "truncated": true,
                "skipped": 0
            }
        })
    }

    Desktop.WallpaperMenu {
        id: menu

        outputName: "DP-1"
        reducedMotion: true
        providerSource: fakeSource
    }

    SignalSpy {
        id: chooserSpy

        target: menu
        signalName: "chooseRequested"
    }

    SignalSpy {
        id: dismissedSpy

        target: menu
        signalName: "dismissed"
    }

    function init() {
        chooserSpy.clear();
        dismissedSpy.clear();
        fakeLedger.state = ({});
        fakeLedger.revision += 1;
        fakeLedger.sendCount = 0;
        fakeLedger.sentProvider = "";
        fakeLedger.sentVerb = "";
        fakeLedger.sentOptions = ({});
        fakeLedger.sentTarget = "";
        fakeLedger.sentPolicy = "";
    }

    function test_it_reads_one_bounded_folder_catalogue() {
        compare(menu.folder, "/pictures/walls");
        compare(menu.folderUrl, "file:///pictures/walls");
        compare(menu.catalogue, "7");
        compare(menu.images.length, 2);
        compare(menu.page, 1);
        compare(menu.pageCount, 2);
        compare(menu.totalImages, 65);
        verify(menu.hasNextPage);
        verify(!menu.hasPreviousPage);
        verify(menu.folderSummary().indexOf("limitada") === -1);
        verify(menu.contentWidth > 300);
        verify(menu.contentHeight > 300);
        verify(findChild(menu, "celestina-wallpaper-gallery") !== null);
        verify(findChild(menu, "celestina-wallpaper-page-controls") !== null);
        verify(findChild(menu, "celestina-wallpaper-previous-page") !== null);
        verify(findChild(menu, "celestina-wallpaper-next-page") !== null);
        tryVerify(function() {
            return menu.glassRegions.length >= 1;
        });
        // The sections are the glass cards, each with the shared tint.
        const section = findChild(menu, "celestina-menu-section");
        verify(section);
        const tint = findChild(section, "celestina-panel-tint");
        verify(tint);
        fuzzyCompare(tint.color.r, CelestinaTheme.elevated.r, 0.01);
        fuzzyCompare(tint.color.a, 0.55, 0.01);
    }

    function test_the_folder_action_hands_selection_to_the_permanent_panel() {
        menu.chooseFolder();

        compare(chooserSpy.count, 1);
        compare(fakeLedger.sendCount, 0);
    }

    function test_a_thumbnail_selects_by_catalogue_identity_and_stays_open() {
        // The delegate forwards its bounded model row to this one request
        // seam; exercising the seam keeps the test independent from whether
        // an offscreen GridView has materialised its first delegate yet.
        menu.selectImage(menu.images[0]);

        compare(fakeLedger.sendCount, 1);
        compare(fakeLedger.sentProvider, "wallpaper-gallery");
        compare(fakeLedger.sentVerb, "select");
        compare(fakeLedger.sentOptions.output, "DP-1");
        compare(fakeLedger.sentOptions.catalogue, "7");
        compare(fakeLedger.sentOptions.id, "first");
        compare(fakeLedger.sentTarget, "select:DP-1");
        compare(fakeLedger.sentPolicy, "immediate");
        compare(dismissedSpy.count, 0);
    }

    function test_every_catalogue_page_is_reachable_without_replacing_its_identity() {
        menu.previousPage();
        compare(fakeLedger.sendCount, 0);

        menu.nextPage();
        compare(fakeLedger.sendCount, 1);
        compare(fakeLedger.sentProvider, "wallpaper-gallery");
        compare(fakeLedger.sentVerb, "set-page");
        compare(fakeLedger.sentOptions.catalogue, "7");
        compare(fakeLedger.sentOptions.page, 2);
        compare(fakeLedger.sentTarget, "page");
        compare(fakeLedger.sentPolicy, "immediate");

        fakeSource.providers = ({
            "wallpaper-gallery": {
                "state": "ready",
                "folder": "/pictures/walls",
                "folderUrl": "file:///pictures/walls",
                "catalogue": "7",
                "page": 2,
                "pageCount": 2,
                "total": 65,
                "hasPrevious": true,
                "hasNext": false,
                "images": [{
                    "id": "last",
                    "name": "last.webp",
                    "previewUrl": "file:///pictures/walls/ultimo.webp",
                    "revision": "65"
                }],
                "truncated": true,
                "skipped": 0
            }
        });
        fakeSource.revision += 1;

        tryCompare(menu, "page", 2);
        compare(menu.images.length, 1);
        verify(menu.hasPreviousPage);
        verify(!menu.hasNextPage);
        fakeLedger.sendCount = 0;
        menu.nextPage();
        compare(fakeLedger.sendCount, 0);
        menu.previousPage();
        compare(fakeLedger.sendCount, 1);
        compare(fakeLedger.sentOptions.page, 1);
        compare(fakeLedger.sentOptions.catalogue, "7");
    }

    function test_a_stale_empty_catalogue_cannot_select_a_path_directly() {
        fakeSource.providers = ({
            "wallpaper-gallery": {
                "state": "ready",
                "folder": "/pictures/walls",
                "folderUrl": "file:///pictures/walls",
                "catalogue": "",
                "images": [{
                    "id": "first",
                    "name": "primero.png",
                    "previewUrl": "file:///pictures/walls/primero.png",
                    "revision": "1"
                }],
                "truncated": false,
                "skipped": 0
            }
        });
        fakeSource.revision += 1;

        tryCompare(menu, "catalogue", "");
        menu.selectImage(menu.images[0]);
        compare(fakeLedger.sendCount, 0);
    }

    function test_an_older_catalogue_without_page_metadata_remains_usable() {
        fakeSource.providers = ({
            "wallpaper-gallery": {
                "state": "ready",
                "folder": "/pictures/walls",
                "folderUrl": "file:///pictures/walls",
                "catalogue": "7",
                "images": [{
                    "id": "first",
                    "name": "primero.png",
                    "previewUrl": "file:///pictures/walls/primero.png",
                    "revision": "1"
                }],
                "truncated": false,
                "skipped": 0
            }
        });
        fakeSource.revision += 1;

        tryCompare(menu, "pageCount", 1);
        compare(menu.page, 1);
        compare(menu.totalImages, 1);
        verify(!menu.hasPreviousPage);
        verify(!menu.hasNextPage);
        menu.selectImage(menu.images[0]);
        compare(fakeLedger.sentVerb, "select");
        compare(fakeLedger.sentOptions.catalogue, "7");
    }

    function cleanup() {
        fakeSource.providers = ({
            "wallpaper-gallery": {
                "state": "ready",
                "folder": "/pictures/walls",
                "folderUrl": "file:///pictures/walls",
                "catalogue": "7",
                "page": 1,
                "pageCount": 2,
                "total": 65,
                "hasPrevious": false,
                "hasNext": true,
                "images": [
                    {
                        "id": "first",
                        "name": "primero.png",
                        "previewUrl": "file:///pictures/walls/primero.png",
                        "revision": "1"
                    },
                    {
                        "id": "second",
                        "name": "segundo.jpg",
                        "previewUrl": "file:///pictures/walls/segundo.jpg",
                        "revision": "2"
                    }
                ],
                "truncated": true,
                "skipped": 0
            }
        });
        fakeSource.revision += 1;
    }
}
