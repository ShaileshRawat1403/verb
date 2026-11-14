#!/bin/sh
# Deterministic Antigravity stand-in; invoked only in a disposable test working directory.
printf '%s\n' "$@" >> args
echo '---' >> args
sleep 0.3
echo '{"event":"init","conversation_id":"conv-9"}'
echo '{"event":"result","result":{"conversation_id":"conv-9","status":"SUCCESS","response":"done"}}'
