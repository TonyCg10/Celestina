import CelestinaStyle
import QtTest
import "../qml" as Desktop

// Shell foregrounds stay light and information-bearing glass stays dark. The
// contextual carrier remains a separate, nearly transparent material.
TestCase {
    id: testCase

    name: "BackdropInk"

    Desktop.BackdropInk {
        id: ink
    }

    function test_fixed_light_ink_and_dark_content_material() {
        compare(ink.primary, CelestinaTheme.text);
        compare(ink.muted, CelestinaTheme.text);
        compare(ink.faint, CelestinaTheme.text);
        compare(ink.accent, CelestinaTheme.text);
        compare(ink.danger, CelestinaTheme.text);
        compare(ink.warning, CelestinaTheme.text);
        compare(ink.focus, CelestinaTheme.text);
        compare(ink.materialTint, CelestinaTheme.glassHighlight);
        compare(ink.contentMaterialTint, CelestinaTheme.canvas);
    }

    function test_interaction_layers_keep_the_light_ink_contract() {
        compare(ink.divider, CelestinaTheme.divider);
        compare(ink.dividerStrong, CelestinaTheme.dividerStrong);
        compare(ink.controlFill, CelestinaTheme.controlFill);
        compare(ink.hoverFill, CelestinaTheme.surfaceHover);
        compare(ink.pressedFill, CelestinaTheme.surfaceStrong);
        compare(ink.selectedFill, CelestinaTheme.surfaceSelected);
        compare(ink.accentFill, CelestinaTheme.accentSoft);
        compare(ink.selectedRestFill, CelestinaTheme.badgeAccentFill);
    }
}
