#pragma once

#include <QObject>
#include <QPointer>
#include <QRect>
#include <QRegion>
#include <QSize>
#include <QTimer>
#include <QVariantList>

class QWindow;

// Build the finite pixel region for one rounded glass rectangle. The radius is
// explicit because a tall menu is not a pill and its shape cannot be inferred
// from its aspect ratio.
QRegion roundedGlassRegion(const QRect &rect, int radius);

// Convert the geometry published by QML into one finite compositor region.
// Typed shapes may provide a sampled `polygon` (QPointF values in window
// coordinates); a valid polygon takes precedence over its rounded-rectangle
// fallback. The legacy `glassRects` list is consulted only when no typed shape
// is published.
QRegion glassRegionFromPublishedShapes(
    const QVariantList &typedShapes,
    const QVariantList &legacyRects = {}
);

// A first arm needs an exposed Wayland surface. Once that arm succeeded,
// layer-shell may keep rendering while Qt temporarily reports `isExposed()`
// as false; the still-visible, sized surface remains safe to update.
bool blurProbeCanUseEffect(
    bool alreadyArmed,
    bool surfaceVisible,
    bool surfaceExposed,
    bool surfaceSized,
    bool effectAvailable,
    bool glassPresent
);

// Owns the compositor-blur lifecycle for one layer-shell window that publishes
// finite rectangles, rounded rectangles or sampled polygons through
// `glassRegions`.
//
// This remains manual C++ because KWindowEffects' Wayland surface integration
// is not available through CXX-Qt. The controller is deliberately limited to
// effect capability, surface geometry and the QML fallback flag.
class PanelBlurController final : public QObject
{
    Q_OBJECT

public:
    explicit PanelBlurController(QWindow *window, QObject *parent = nullptr);

    void start();

private:
    // PANEL-1. The blur follows the declared glass shapes, not the surface.
    QRegion glassRegion() const;

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void glassRegionsChanged();

private:

    enum class State {
        Pending,
        Enabled,
        Fallback,
    };

    void geometryChanged();
    void probe();
    void scheduleProbe(int delayMs);
    void setAvailable(bool available);

    QPointer<QWindow> m_window;
    QTimer m_probeTimer;
    QSize m_armedSize;
    QRegion m_armedRegion;
    State m_state = State::Pending;
    int m_fastAttemptsRemaining = 0;
};
