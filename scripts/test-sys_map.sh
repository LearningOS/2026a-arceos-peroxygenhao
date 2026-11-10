#!/bin/bash

tmp_file=$(mktemp) || exit 1
trap 'rm -f -- "$tmp_file"' EXIT
grep_content="Read back content: hello, arceos!"

cd "$(dirname "$0")/../arceos" || exit 1

rm pflash.img -f
rm disk.img -f

make pflash_img
make disk_img

make payload
./update_disk.sh payload/mapfile_c/mapfile

make run A=exercises/sys_map/ BLK=y | tee "$tmp_file"

output=$(grep -Ea "$grep_content" "$tmp_file")


if [[ -z "$output" ]]; then
    echo "sys_mmap default"
    exit 1
else 
    echo "sys_mmap pass"
    exit 0
fi
