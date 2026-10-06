#!/bin/sh
set -eu

if [ "$#" -ne 5 ]; then
  printf '%s\n' "Usage: $0 TRACKER_BINARY VIDEO START_SECONDS DURATION_SECONDS OUTPUT_JSON" >&2
  exit 2
fi

tracker_binary=$1
video_path=$2
start_seconds=$3
duration_seconds=$4
output_json=$5
metadata_path="${output_json%.json}.metadata.txt"

if [ ! -x "$tracker_binary" ]; then
  printf 'Tracker is not executable: %s\n' "$tracker_binary" >&2
  exit 1
fi
if [ ! -f "$video_path" ]; then
  printf 'Input video does not exist: %s\n' "$video_path" >&2
  exit 1
fi

mkdir -p "$(dirname "$output_json")"

ffprobe -v error \
  -show_entries format=filename,duration,size:stream=index,codec_type,codec_name,width,height,avg_frame_rate \
  -of default=noprint_wrappers=1 \
  "$video_path" > "$metadata_path"
{
  printf 'tracker_binary=%s\n' "$tracker_binary"
  printf 'input_video=%s\n' "$video_path"
  printf 'start_seconds=%s\n' "$start_seconds"
  printf 'duration_seconds=%s\n' "$duration_seconds"
  cat "$metadata_path"
} > "${metadata_path}.tmp"
mv "${metadata_path}.tmp" "$metadata_path"

"$tracker_binary" "$video_path" "$start_seconds" "$duration_seconds" > "$output_json"

python3 - "$output_json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    result = json.load(handle)

podcast = result.get("podcast") or {}
people = podcast.get("people") or []
segments = podcast.get("segments") or []
ids = [person.get("id") for person in people]
print(f"people={len(people)}")
print(f"person_ids={ids}")
print(f"segments={len(segments)}")
print("identity_results=require manual review of raw keyframes and segments")
PY
