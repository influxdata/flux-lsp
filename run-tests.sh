#!/bin/bash

FILE=$HOME/.cargo/env
if test -f "$FILE"; then
    source $FILE
fi

make test