#!/bin/bash

echo "Bundling CI files..."

echo "DLIB = ${CI_DLIB}"
echo "SLIB = ${CI_SLIB}"
echo "EXE = ${CI_EXE}"
echo "PFIX = ${CI_PFIX}"

mkdir ci_build
mkdir ci_build/release
mkdir ci_build/debug

mv "target/release/asset_migrator${CI_EXE}" ci_build/release
mv "target/debug/asset_migrator${CI_EXE}" ci_build/debug