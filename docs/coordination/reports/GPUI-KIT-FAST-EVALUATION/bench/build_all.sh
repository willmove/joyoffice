#!/bin/bash
# Sequential clean release builds; one fresh target dir per variant.
B="$(cd "$(dirname "$0")" && pwd)"
LOG="$B/build_times.txt"
: > "$LOG"
for v in v022 vpre vfast vkit vkitfast; do
  rm -rf "$B/target-rel/$v"
  cd "$B/$v"
  start=$(python3 -c 'import time;print(time.time())')
  cargo build --release --locked --target-dir "$B/target-rel/$v" > "$B/build-$v.log" 2>&1 || cargo build --release --target-dir "$B/target-rel/$v" > "$B/build-$v.log" 2>&1
  status=$?
  end=$(python3 -c 'import time;print(time.time())')
  units=$(grep -c "Compiling " "$B/build-$v.log")
  size=$(stat -f %z "$B/target-rel/$v/release/bench" 2>/dev/null)
  echo "$v status=$status secs=$(python3 -c "print(round($end-$start,1))") compiled_units=$units bin_bytes=$size" >> "$LOG"
done
echo DONE >> "$LOG"
