| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-empty | 1 | baseline | 51.024 [24.670, 77.378] | 15.677 | 45.5 |
| buffers-empty | 2 | baseline | 27.069 [27.014, 27.123] | 7.104 | 44.2 |
| buffers-empty | 4 | baseline | 26.596 [25.907, 27.284] | 6.895 | 44.7 |
| buffers-empty | 8 | baseline | 25.815 [25.246, 26.384] | 6.869 | 45.3 |
| buffers-grow | 1 | baseline | 34.446 [33.077, 35.816] | 2.112 | 40.6 |
| buffers-grow | 2 | baseline | 31.725 [31.156, 32.295] | 1.892 | 38.9 |
| buffers-grow | 4 | baseline | 54.482 [34.254, 74.710] | 4.538 | 40.6 |
| buffers-grow | 8 | baseline | 49.380 [32.463, 66.298] | 2.604 | 39.6 |
| buffers-pointers | 1 | baseline | 18.548 [18.387, 18.709] | 1.540 | 42.8 |
| buffers-pointers | 2 | baseline | 25.512 [25.396, 25.628] | 1.563 | 42.4 |
| buffers-pointers | 4 | baseline | 25.556 [25.433, 25.680] | 1.676 | 42.8 |
| buffers-pointers | 8 | baseline | 26.008 [25.464, 26.552] | 1.630 | 42.8 |
| bytes-survivors | 1 | baseline | 174.191 [122.134, 226.247] | 2.745 | 39.9 |
| bytes-survivors | 2 | baseline | 102.026 [99.278, 104.774] | 2.002 | 40.1 |
| bytes-survivors | 4 | baseline | 167.844 [111.316, 224.373] | 2.784 | 40.2 |
| bytes-survivors | 8 | baseline | 104.938 [104.240, 105.636] | 1.991 | 40.1 |
| bytes-transient | 1 | baseline | 116.087 [113.814, 118.361] | 2.162 | 39.1 |
| bytes-transient | 2 | baseline | 111.739 [110.461, 113.018] | 1.969 | 40.2 |
| bytes-transient | 4 | baseline | 105.574 [105.303, 105.846] | 1.975 | 39.8 |
| bytes-transient | 8 | baseline | 104.428 [103.454, 105.403] | 1.967 | 39.2 |
| messages-int-fanout-sustained | 1 | baseline | 188.586 [131.900, 245.271] | 13.462 | 39.3 |
| messages-int-fanout-sustained | 2 | baseline | 136.721 [133.890, 139.551] | 13.013 | 47.4 |
| messages-int-fanout-sustained | 4 | baseline | 319.223 [310.552, 327.894] | 12.025 | 52.1 |
| messages-int-fanout-sustained | 8 | baseline | 322.359 [303.334, 341.383] | 13.089 | 61.3 |
| messages-int-retained-sustained | 1 | baseline | 149.153 [101.037, 197.268] | 13.124 | 40.0 |
| messages-int-retained-sustained | 2 | baseline | 115.209 [114.634, 115.784] | 11.330 | 51.9 |
| messages-int-retained-sustained | 4 | baseline | 129.606 [125.330, 133.882] | 10.563 | 79.6 |
| messages-int-retained-sustained | 8 | baseline | 140.788 [139.113, 142.463] | 11.171 | 93.2 |
| messages-int-slow-sustained | 1 | baseline | 154.708 [153.054, 156.363] | 9.662 | 40.4 |
| messages-int-slow-sustained | 2 | baseline | 175.892 [173.543, 178.241] | 11.138 | 51.9 |
| messages-int-slow-sustained | 4 | baseline | 383.959 [270.862, 497.056] | 22.166 | 52.2 |
| messages-int-slow-sustained | 8 | baseline | 289.805 [280.438, 299.171] | 11.265 | 55.7 |
| messages-int-sustained | 1 | baseline | 107.024 [106.349, 107.699] | 9.716 | 40.3 |
| messages-int-sustained | 2 | baseline | 167.776 [106.862, 228.690] | 11.792 | 53.7 |
| messages-int-sustained | 4 | baseline | 242.148 [110.846, 373.450] | 21.428 | 63.1 |
| messages-int-sustained | 8 | baseline | 227.695 [102.641, 352.749] | 21.116 | 62.3 |
| persistent-4096 | 1 | baseline | 74.858 [33.597, 116.119] | 23.392 | 47.7 |
| persistent-4096 | 2 | baseline | 45.296 [32.842, 57.751] | 11.874 | 45.6 |
| persistent-4096 | 4 | baseline | 31.505 [30.942, 32.069] | 7.577 | 45.8 |
| persistent-4096 | 8 | baseline | 48.593 [32.410, 64.777] | 12.854 | 45.6 |
| words-transient | 1 | baseline | 195.681 [134.191, 257.170] | 10.411 | 39.3 |
| words-transient | 2 | baseline | 193.029 [138.915, 247.143] | 10.380 | 40.2 |
| words-transient | 4 | baseline | 130.564 [129.947, 131.180] | 7.423 | 39.8 |
| words-transient | 8 | baseline | 221.008 [126.372, 315.644] | 11.547 | 39.6 |

Diagnostic GC phases summed across measured repetitions (wall time):
