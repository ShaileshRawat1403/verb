# I-5 emulation-cost probe. `source` this file; it needs no external program, so it runs in the shipped
# Agent Runtime shell, whose children cannot exec. It times a pure-bash arithmetic loop three times.
verb_bench_bash() {
  local n=300000 r i
  for r in 1 2 3; do
    TIMEFORMAT="bash-loop n=$n run=$r real=%3R user=%3U sys=%3S"
    time { i=0; while (( i < n )); do (( i++ )); done; }
  done
}
verb_bench_bash
