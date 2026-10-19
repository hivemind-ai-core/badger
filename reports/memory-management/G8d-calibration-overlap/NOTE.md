# Calibration retained, excluded from timing conclusions

A fixture compile/check overlapped the final baseline processes. It failed at
check time because user modules cannot declare reserved intrinsics. Preserve
all raw samples here, but use G8d-baseline's clean repeat for timing baselines.
Allocation counts remain diagnostic observations, not elapsed-time evidence.
