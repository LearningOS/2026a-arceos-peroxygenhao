#!/bin/bash


tmp_file=$(mktemp) || exit 1
trap 'rm -f -- "$tmp_file"' EXIT
grep_content="Bump tests run OK!"

cd "$(dirname "$0")/../arceos" || exit 1


rm pflash.img -f
rm disk.img -f

make pflash_img
make disk_img


make A=exercises/alt_alloc/ run | tee "$tmp_file"

output=$(tail -n1 "$tmp_file" | grep -a "$grep_content")


if [[ -z "$output" ]] ;then
    echo "alt_alloc default"
    exit 1
else 
    echo "alt_alloc pass"
    exit 0
fi
