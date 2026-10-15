.pragma library

// The one way a surface reads a provider out of the host's snapshot.
// Dynamic `QVariantMap` keys may appear after a binding was created. Keeping
// `revision` in the returned expression gives every lookup a stable dependency
// that changes with each accepted provider frame.
function read(source, name) {
    if (!source || !source.providers || source.revision < 0)
        return undefined;
    return source.providers[name];
}
