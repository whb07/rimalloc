# Benchmark host profiles

Release baselines are valid only for the host profile named by each schema-2
threshold entry. A profile records the stable machine and operating conditions
used for five complete runs; it is not a portable performance promise.

Before accepting a baseline:

1. use one clean commit for all five runs;
2. verify the captured CPU model, core counts, NUMA topology, governor, turbo
   state, kernel, compilers, target, artifact mode, and features;
3. inspect the five runs for thermal throttling, CPU migration, and outliers;
4. generate a proposal without `--write-reviewed-baseline`;
5. review the proposal, then repeat with that flag in a dedicated baseline
   commit.

Do not compare results across host profiles, artifact modes, or feature sets.
