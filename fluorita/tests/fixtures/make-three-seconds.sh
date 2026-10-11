#!/bin/sh
set -eu

# Writes `three-seconds.mp4`, the trim's fixture: three seconds of ffmpeg's
# test pattern at 320 × 240 and 30 frames a second (90 frames), H.264 with a
# keyframe only where x264 decides, and a sine tone as AAC. Run once; the
# file is committed, so the tests never need an encoder to build their input.
#
#   sh fluorita/tests/fixtures/make-three-seconds.sh

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

ffmpeg -hide_banner -nostdin -y \
    -f lavfi -i testsrc=duration=3:size=320x240:rate=30 \
    -f lavfi -i sine=duration=3 \
    -c:v libx264 -crf 30 -c:a aac \
    "$here/three-seconds.mp4" </dev/null
