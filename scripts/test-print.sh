#!/bin/bash

cd "$(dirname "$0")/../arceos" || exit 1

rm pflash.img -f 
rm disk.img -f

make pflash_img
make disk_img


tmp_file=$(mktemp) || exit 1
trap 'rm -f -- "$tmp_file"' EXIT
make run A=exercises/print_with_color/ | tee "$tmp_file"

context=$(tail -n 20 "$tmp_file")

has_color=false
has_text=false


if [[ "$context" == *$'\x1b['* ]]; then
    echo "Has color"
    has_color=true

    sed_context=$(echo "$context" | sed 's/\x1b\[[0-9;]*m//g')

    if grep -q "Hello, Arceos!" <<< "$sed_context" ; then
        echo "Has Hello, Arceos!"
        has_text=true
    else
        echo "No Hello, Arceos!"
    fi
else
    echo "No color"
fi


if [[ "$has_color" == true && "$has_text" == true ]]; then
    echo "print_with_color pass"
    exit 0
else
    echo "print_with_color fault"
    exit 1
fi
