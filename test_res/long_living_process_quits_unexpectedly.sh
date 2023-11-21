#!/usr/bin/env bash

function start_long_living_task() {
    echo "Beginning..."
    sleep 6
}

start_long_living_task &
sleep 1
exit 1
