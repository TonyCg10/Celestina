import QtQuick
import QtTest
import CelestinaStyle

// CelestinaTheme reads every token from the generated CelestinaDesignTokens:
// each scheme role, through the scheme and the flat sys.* property, is the
// generated colour, and the scalars the theme renames are the same numbers.
TestCase {
    name: "CelestinaDesignTokens"

    function test_every_dark_role_is_the_generated_colour() {
        const roles = CelestinaDesignTokens.schemeDarkRoles
        verify(roles.length > 80)
        for (let index = 0; index < roles.length; ++index) {
            const role = roles[index]
            verify(CelestinaDesignTokens.schemeDark[role] !== undefined, role)
            verify(Qt.colorEqual(CelestinaTheme.scheme[role],
                                 CelestinaDesignTokens.schemeDark[role]), role)
            verify(Qt.colorEqual(CelestinaTheme[role],
                                 CelestinaDesignTokens.schemeDark[role]), role)
        }
    }

    // A colour as its [alpha, red, green, blue] bytes.
    function bytes(value) {
        return [value.a, value.r, value.g, value.b].map(
                    channel => Math.round(channel * 255))
    }

    function test_resolved_roles_keep_their_bytes() {
        // The accent mixed 0.24 towards accentLift, resolved to 8 bits.
        compare(bytes(CelestinaTheme.focusRing), [255, 108, 171, 255])
        // One white step above controlHover's 41.
        compare(bytes(CelestinaTheme.controlPressed), [56, 255, 255, 255])
        // The canvas at 0.70: 178.5 rounds half-up to 179.
        compare(bytes(CelestinaTheme.glassTint), [179, 5, 6, 8])
        verify(Qt.colorEqual(CelestinaTheme.glassFallback, CelestinaTheme.canvas))
    }

    function test_renamed_scalars_are_the_generated_numbers() {
        compare(CelestinaTheme.radiusLg, CelestinaDesignTokens.radiusLg)
        compare(CelestinaTheme.space2xl, CelestinaDesignTokens.spacingXxl)
        compare(CelestinaTheme.fontBodyBase, CelestinaDesignTokens.typeBody)
        compare(CelestinaTheme.motionFast, CelestinaDesignTokens.motionFast)
        compare(CelestinaTheme.pressRecoilScale, CelestinaDesignTokens.motionPressScale)
        compare(CelestinaTheme.compMenuWidth, CelestinaDesignTokens.controlMenuWidth)
        compare(CelestinaTheme.glassNoiseOpacity, CelestinaDesignTokens.hazeGrain)
        compare(CelestinaTheme.easeOneUi, [0.22, 0.25, 0, 1, 1, 1])
        compare(CelestinaTheme.weightRegular, CelestinaDesignTokens.typeWeightBody)
        compare(CelestinaTheme.weightDemiBold, CelestinaDesignTokens.typeWeightTitle)
    }
}
