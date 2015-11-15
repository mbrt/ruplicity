#!/bin/bash
pushd .
cd ..
duplicity restore --no-encryption --time 2015-06-17T20:25:46+02:00 file://`pwd` restored/1
duplicity restore --no-encryption --time 2015-06-17T20:26:30+02:00 file://`pwd` restored/2
duplicity restore --no-encryption --time 2015-06-17T20:26:51+02:00 file://`pwd` restored/3
popd
