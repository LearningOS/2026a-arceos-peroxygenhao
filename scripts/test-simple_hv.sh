#!/bin/bash

tmp_file=$(mktemp) || exit 1
trap 'rm -f -- "$tmp_file"' EXIT
grep_content="Shutdown vm normally!"

cd "$(dirname "$0")/../arceos" || exit 1


rm pflash.img
rm disk.img

make pflash_img
make disk_img

make payload
./update_disk.sh payload/skernel2/skernel2

make run A=exercises/simple_hv/ BLK=y | tee "$tmp_file"

output=$(grep -a "$grep_content" "$tmp_file" | tail -n1  )


if [[ -z "$output" ]]; then
    echo "simple_hv default"
    exit 1
else 
    echo "simple_hv pass"
    exit 0
fi
