#pragma once

#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSGSimpleRectNode>
#include <QWindow>

// One guaranteed commit for a surface whose interesting change is invisible.
//
// Layer-shell and effect state are double-buffered: a keyboard-interactivity
// switch, an input-region change or a blur region all ride the surface's next
// commit — and a window whose scene has nothing dirty renders nothing and
// commits nothing, which is how a parked carrier once kept the compositor's
// idea of its keyboard and input for as long as it stayed clean. The old
// answer was dirtying the clear colour, and the colour chosen was
// `QColor(1, 0, 0, 0)`: with alpha zero that red unit reaches the compositor
// unpremultiplied and composites additively — one red count per surface,
// measured as the light one-hertz red blink the author filmed (2026-08-20).
//
// The dirt here is a one-pixel, fully transparent scene node nudged between
// two positions: the scene is really dirty, so the frame and the commit
// happen, and the composited pixels are identical either way.
class CommitNudgeItem final : public QQuickItem
{
public:
    explicit CommitNudgeItem(QQuickItem *parent = nullptr)
        : QQuickItem(parent)
    {
        setFlag(ItemHasContents);
        setSize(QSizeF(1, 1));
    }

protected:
    QSGNode *updatePaintNode(QSGNode *old, UpdatePaintNodeData *) override
    {
        auto *node = static_cast<QSGSimpleRectNode *>(old);
        if (!node)
            node = new QSGSimpleRectNode(QRectF(0, 0, 1, 1), Qt::transparent);
        node->setRect(0, 0, 1, 1);
        return node;
    }
};

inline void nudgeSurfaceCommit(QWindow *window)
{
    if (!window)
        return;
    auto *quick = qobject_cast<QQuickWindow *>(window);
    if (!quick || !quick->contentItem()) {
        // A plain window has no scene to dirty; the request is all there is.
        window->requestUpdate();
        return;
    }

    QQuickItem *dirt = nullptr;
    const auto children = quick->contentItem()->childItems();
    for (QQuickItem *const child : children) {
        if (child->objectName() == QLatin1String("celestina-commit-nudge")) {
            dirt = child;
            break;
        }
    }
    if (!dirt) {
        dirt = new CommitNudgeItem(quick->contentItem());
        dirt->setObjectName(QStringLiteral("celestina-commit-nudge"));
    }
    dirt->setX(dirt->x() == 0 ? 1 : 0);
    quick->requestUpdate();
}
