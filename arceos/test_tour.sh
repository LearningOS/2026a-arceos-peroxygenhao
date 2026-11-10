#!/bin/sh

rm pflash.img
rm disk.img

make pflash_img
make disk_img

make run A=tour/u_1_0
make run A=tour/u_2_0
make run A=tour/u_3_0
make run A=tour/u_4_0
make run A=tour/u_5_0
make run A=tour/u_6_0
make run A=tour/u_6_1
make run A=tour/u_7_0 BLK=y
make run A=tour/u_8_0 BLK=y
