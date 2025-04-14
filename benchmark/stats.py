import glob
import re
import json
from collections import defaultdict, OrderedDict

# Regex to extract latency array and value from line
line_pattern = re.compile(r"with latency\s+(\[[^\]]+\])\s+and value\s+\{\"([^\"]+)\"\}")

# Ordered dictionary to maintain insertion order
latency_by_category = OrderedDict()

# Process all matching log files
for filepath in sorted(glob.glob("syncer-*.log")):
    with open(filepath, 'r') as file:
        for line in file:
            match = line_pattern.search(line)
            if match:
                latency_str, category = match.groups()
                try:
                    latencies = json.loads(latency_str)
                    if category not in latency_by_category:
                        latency_by_category[category] = []
                    latency_by_category[category].extend(latencies)
                except json.JSONDecodeError:
                    print(f"Invalid latency array in line: {line.strip()}")

# Compute average latencies
average_latencies = OrderedDict()
for category, latencies in latency_by_category.items():
    if latencies:
        avg = sum(latencies) / len(latencies)
        average_latencies[category] = avg
    else:
        average_latencies[category] = None

# Print average latencies
print("\nAverage latencies per category:")
for category, avg in average_latencies.items():
    print(f"  {category}: {avg:.2f} ms")

# Compute and print latency differences
print("\nLatency differences between subsequent categories:")
previous_category = None
previous_avg = None
for category, avg in average_latencies.items():
    if previous_category is not None:
        diff = avg - previous_avg
        print(f"  {previous_category} → {category}: {diff:.2f} ms")
    previous_category, previous_avg = category, avg
