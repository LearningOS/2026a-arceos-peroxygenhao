#!/bin/bash

tmp_file=$(mktemp) || exit 1
trap 'rm -f -- "$tmp_file"' EXIT
grep_content="Memory tests run OK!"

cd "$(dirname "$0")/../arceos" || exit 1


rm pflash.img -f
rm disk.img -f 

make pflash_img
make disk_img


make run A=exercises/support_hashmap/ | tee "$tmp_file"

output=$(tail -n1 "$tmp_file" | grep -a "$grep_content")


if [[ -z "$output" ]]; then
    echo "support_hashmap default"
    exit 1
else 
    echo "support_hashmap pass"
    exit 0
fi
