#!/bin/sh
# Build the read-only C++ reference and the external capture driver.
set -eu

: "${QT_PREFIX:?set QT_PREFIX to the Qt 5.15.16 installation prefix}"
: "${CAPTURE_BUILD_DIR:=/home/user/.cache/morid-capture/legacy-build}"
: "${CAPTURE_DRIVER:=/home/user/.cache/morid-capture/qt-driver.so}"
export CAPTURE_DRIVER

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
mkdir -p "$CAPTURE_BUILD_DIR"

cmake -S "$ROOT/legacy" -B "$CAPTURE_BUILD_DIR" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_WITH_QT6=OFF \
    -DCMAKE_PREFIX_PATH="$QT_PREFIX"
cmake --build "$CAPTURE_BUILD_DIR" -j2

g++ -std=c++14 -O2 -fPIC -shared "$ROOT/tools/capture/qt_driver.cpp" \
    -I"$QT_PREFIX/include" \
    -I"$QT_PREFIX/include/QtCore" \
    -I"$QT_PREFIX/include/QtGui" \
    -I"$QT_PREFIX/include/QtWidgets" \
    -L"$QT_PREFIX/lib" \
    -Wl,-rpath,"$QT_PREFIX/lib" \
    -lQt5Widgets -lQt5Gui -lQt5Core -ldl \
    -o "$CAPTURE_DRIVER"

printf 'legacy=%s\ndriver=%s\n' \
    "$CAPTURE_BUILD_DIR/media-downloader" "$CAPTURE_DRIVER"
