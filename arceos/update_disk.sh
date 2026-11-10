#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "Usage: ./update_disk.sh <file>" >&2
    exit 1
fi

# make disk_img creates /sbin in the FAT image.
mcopy -o -i disk.img "$1" ::/sbin/
