#!/bin/zsh
# Capture process-separated CPU samples for a fixed Foco performance scenario.
#
# The script intentionally samples only process metadata. It never reads Foco
# logs, source files, prompts, tool payloads, or environment variables.

set -euo pipefail
export LC_ALL=C

usage() {
  cat <<'USAGE'
Usage:
  zsh scripts/perf/capture-macos-process-baseline.zsh \
    --name <scenario> --pids <pid[,pid...]> [--duration-seconds <seconds>] \
    [--sample-seconds <seconds>] [--browser-version <version>] \
    [--output-dir <directory>] [--powermetrics]

The supplied PIDs are treated as roots. Direct and nested children alive at
each sample are included so cargo/node/rg subprocesses remain separately
visible in the CSV. CPU percentages are derived from adjacent cumulative CPU
time samples rather than `ps %cpu`. `--powermetrics` requires an interactive
sudo prompt and captures macOS task wakeups plus process-energy output.
USAGE
}

scenario=""
root_pids=""
duration_seconds=300
sample_seconds=1
output_dir=".foco/perf-baselines"
capture_powermetrics=0
browser_version="unknown"

while (( $# > 0 )); do
  case "$1" in
    --name)
      scenario="${2:-}"
      shift 2
      ;;
    --pids)
      root_pids="${2:-}"
      shift 2
      ;;
    --duration-seconds)
      duration_seconds="${2:-}"
      shift 2
      ;;
    --sample-seconds)
      sample_seconds="${2:-}"
      shift 2
      ;;
    --output-dir)
      output_dir="${2:-}"
      shift 2
      ;;
    --browser-version)
      browser_version="${2:-}"
      shift 2
      ;;
    --powermetrics)
      capture_powermetrics=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      print -u2 -- "Unknown option: $1"
      usage >&2
      exit 64
      ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  print -u2 -- "This collector is macOS-only. Use an OS-specific collector instead."
  exit 69
fi

if [[ -z "$scenario" || -z "$root_pids" || ! "$duration_seconds" =~ '^[1-9][0-9]*$' || ! "$sample_seconds" =~ '^[1-9][0-9]*$' ]]; then
  usage >&2
  exit 64
fi

for pid in ${(s:,:)root_pids}; do
  if [[ ! "$pid" =~ '^[1-9][0-9]*$' ]] || ! kill -0 "$pid" 2>/dev/null; then
    print -u2 -- "PID is not running: $pid"
    exit 69
  fi
done

run_id="${scenario//[^A-Za-z0-9._-]/-}-$(date -u +%Y%m%dT%H%M%SZ)"
run_dir="$output_dir/$run_id"
mkdir -p "$run_dir"
samples_csv="$run_dir/process-samples.csv"
intervals_csv="$run_dir/cpu-interval-samples.csv"
summary_csv="$run_dir/process-summary.csv"
group_summary_csv="$run_dir/group-summary.csv"

print -- "timestamp_utc,sample,pid,ppid,cpu_time,rss_kib,command" > "$samples_csv"
print -- "scenario=$scenario" > "$run_dir/metadata.txt"
print -- "root_pids=$root_pids" >> "$run_dir/metadata.txt"
print -- "duration_seconds=$duration_seconds" >> "$run_dir/metadata.txt"
print -- "sample_seconds=$sample_seconds" >> "$run_dir/metadata.txt"
print -- "macos_version=$(sw_vers -productVersion 2>/dev/null || print unknown)" >> "$run_dir/metadata.txt"
print -- "cpu_model=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || print unknown)" >> "$run_dir/metadata.txt"
memory_bytes="$(sysctl -n hw.memsize 2>/dev/null || print 0)"
print -- "memory_gib=$(( memory_bytes / 1024 / 1024 / 1024 ))" >> "$run_dir/metadata.txt"
print -- "browser_version=$browser_version" >> "$run_dir/metadata.txt"
print -- "git_commit=$(git rev-parse --verify HEAD 2>/dev/null || print unknown)" >> "$run_dir/metadata.txt"
print -- "started_at_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$run_dir/metadata.txt"

collect_descendants() {
  local parent="$1"
  local child
  for child in ${(f)$(pgrep -P "$parent" 2>/dev/null || true)}; do
    print -- "$child"
    collect_descendants "$child"
  done
}

powermetrics_pid=""
energy_proxy="not_requested"
if (( capture_powermetrics )); then
  # `tasks` is system-wide rather than PID-filtered. `--show-process-energy`
  # adds per-process energy impact so analysis can match it to the CSV PID/name.
  sudo powermetrics --samplers tasks --show-process-energy -i $((sample_seconds * 1000)) -n $(( (duration_seconds + sample_seconds - 1) / sample_seconds )) > "$run_dir/powermetrics-tasks.txt" 2>&1 &
  powermetrics_pid="$!"
fi

sample_count=$(( (duration_seconds + sample_seconds - 1) / sample_seconds ))
for sample in {1..$sample_count}; do
  observed_pids=(${(s:,:)root_pids})
  for root_pid in ${(s:,:)root_pids}; do
    observed_pids+=(${(f)$(collect_descendants "$root_pid")})
  done
  observed_pids=(${(u)observed_pids})
  timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  if (( ${#observed_pids} > 0 )); then
    ps -o pid= -o ppid= -o time= -o rss= -o comm= -p "${(j:,:)observed_pids}" 2>/dev/null | while read -r pid ppid cpu_time rss command; do
      [[ -z "$pid" ]] && continue
      command="${command//,/_}"
      print -- "$timestamp,$sample,$pid,$ppid,$cpu_time,$rss,$command" >> "$samples_csv"
    done
  fi

  (( sample < sample_count )) && sleep "$sample_seconds"
done

if [[ -n "$powermetrics_pid" ]]; then
  if wait "$powermetrics_pid"; then
    energy_proxy="powermetrics_tasks_with_process_energy"
  else
    energy_proxy="not_collected"
    print -u2 -- "powermetrics ended unsuccessfully; inspect $run_dir/powermetrics-tasks.txt"
  fi
fi

print -- "sample,pid,cpu_percent" > "$intervals_csv"
awk -F, -v sample_seconds="$sample_seconds" '
    function cpu_seconds(value, pieces, time_parts, day_count, count) {
      day_count = 0
      if (index(value, "-") > 0) {
        split(value, pieces, "-")
        day_count = pieces[1]
        value = pieces[2]
      }
      count = split(value, time_parts, ":")
      if (count == 3) return day_count * 86400 + time_parts[1] * 3600 + time_parts[2] * 60 + time_parts[3]
      if (count == 2) return day_count * 86400 + time_parts[1] * 60 + time_parts[2]
      return day_count * 86400 + value
    }
    NR > 1 {
      current_cpu_seconds = cpu_seconds($5)
      pid = $3
      if (pid in previous_cpu_seconds && $2 > previous_sample[pid]) {
        elapsed_seconds = ($2 - previous_sample[pid]) * sample_seconds
        cpu_delta_seconds = current_cpu_seconds - previous_cpu_seconds[pid]
        if (cpu_delta_seconds >= 0 && elapsed_seconds > 0) {
          printf "%s,%s,%.2f\n", $2, pid, cpu_delta_seconds * 100 / elapsed_seconds
        }
      }
      previous_cpu_seconds[pid] = current_cpu_seconds
      previous_sample[pid] = $2
    }
  ' "$samples_csv" >> "$intervals_csv"

print -- "pid,average_cpu_percent,p95_cpu_percent,samples" > "$summary_csv"
awk -F, 'NR > 1 { print $2 "," $3 }' "$intervals_csv" \
  | sort -t, -k1,1n -k2,2n \
  | awk -F, '
      function emit() {
        if (count == 0) return
        rank = int((count * 95 + 99) / 100)
        printf "%s,%.2f,%.2f,%d\n", pid, sum / count, values[rank], count
      }
      $1 != pid { emit(); delete values; pid = $1; count = 0; sum = 0 }
      { values[++count] = $2; sum += $2 }
      END { emit() }
    ' >> "$summary_csv"

print -- "average_group_cpu_percent,p95_group_cpu_percent,samples" > "$group_summary_csv"
awk -F, 'NR > 1 { cpu[$1] += $3 } END { for (sample in cpu) print cpu[sample] }' "$intervals_csv" \
  | sort -n \
  | awk '
      { values[++count] = $1; sum += $1 }
      END {
        if (count > 0) {
          rank = int((count * 95 + 99) / 100)
          printf "%.2f,%.2f,%d\n", sum / count, values[rank], count
        }
      }
    ' >> "$group_summary_csv"

print -- "finished_at_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$run_dir/metadata.txt"
print -- "energy_proxy=$energy_proxy" >> "$run_dir/metadata.txt"
print -- "Collected $sample_count samples in $run_dir"
print -- "CPU summaries derived from adjacent CPU-time samples: $summary_csv and $group_summary_csv"
if (( ! capture_powermetrics )); then
  print -- "No wakeup/energy proxy capture: rerun with --powermetrics after reviewing the sudo prompt."
fi
