import os
import random

# Small fallback wordlist (can be replaced with a larger dictionary)
WORDS = [
    "apple", "orange", "banana", "grape", "melon", "kiwi", "pear", "peach",
    "table", "chair", "window", "book", "paper", "light", "phone", "clock",
    "green", "blue", "red", "white", "black", "silver", "gold", "yellow",
    "river", "mountain", "forest", "desert", "ocean", "valley", "sky", "cloud"
]

# Parameters
num_files = 112
lines_per_file = 100
max_bytes = 31  # must be < 248 bits

for file_idx in range(0, num_files):
    filename = f"input_{file_idx}.txt"
    with open(filename, "w") as f:
        for _ in range(lines_per_file):
            line = ""
            # keep adding random words until we reach the limit
            while True:
                word = random.choice(WORDS)
                if len(line) + len(word) + (1 if line else 0) > max_bytes:
                    break
                line += (" " if line else "") + word
            f.write(line + "\n")

print(f"Generated {num_files} files in current folder")

