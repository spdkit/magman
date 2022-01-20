#! /bin/bash
# [[file:magman.note::de19183e][de19183e]]
version=v0.0.14
cargo im --offline
#cargo im
install -D -t bin/$version ~/.cargo/bin/magman


scp bin/$version/magman hpc44:bin/magman-$version
# de19183e ends here
