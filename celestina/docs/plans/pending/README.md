# Authorized plans waiting for their checkpoint

A plan lives in `active/` only while its checkpoint is the one this project's
ROADMAP names as active, and exactly one checkpoint is active at a time. These
plans are written, authorized and bounded, but their checkpoint has not opened
yet: they are not work in progress and no unit in them may be built from here.

Each moves to `active/` unchanged, keeping its `Plan ID`, when the ROADMAP
makes its checkpoint the active one — which is how the lock's plan and then
the polkit agent's left this directory.

No plan is waiting here. The next one to arrive will be written before its
checkpoint opens, as both of the last two were.
