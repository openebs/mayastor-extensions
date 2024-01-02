#!/usr/bin/env bash

# Takes a lock by way of a local file with the name <file-name>.lock.
advisory_lock_acquire() {
  local -r filepath=$1
  local -r lock_filepath=$2
  local -r owner_name=$3
  local -r lock_probe_frequency=${4:-1s}

  until ! (stat "$lock_filepath" --printf="" 2>/dev/null \
    && [[ $(cat "$lock_filepath") != "owner: $owner_name" ]])
  do
    echo "Waiting for a lock on the file $filepath... (sleep $lock_probe_frequency)"
    sleep "$lock_probe_frequency"
  done

  cat > "$lock_filepath" <<< "owner: $owner_name"

  echo "Acquired a lock on the file $filepath."
}

advisory_lock_remove() {
  local -r filepath=$1
  local -r lock_filepath=$2
  local -r owner_name=$3

  if stat "$lock_filepath" --printf="" 2>/dev/null && [[ $(cat "$lock_filepath") == "owner: $owner_name" ]]; then
    rm -f "$lock_filepath"
    echo "Removed the lock on the file $filepath."
  fi
}
