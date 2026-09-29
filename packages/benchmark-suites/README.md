# Built-in benchmark suites

`core-v1.json` contains 12 original cases across reasoning, coding, summarization,
structured extraction, instruction following, and retrieval. The data is dedicated
to the public domain under CC0-1.0; see [LICENSE](LICENSE).

The benchmark engine computes the SHA-256 of the exact dataset bytes and snapshots
that checksum with each run. Changes to the dataset prevent resuming an old run
silently. Warm-ups are excluded from measured scores. These small suites are smoke
comparisons, not general model-capability evaluations.
