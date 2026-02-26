#!/bin/bash
# Test ES1I pipeline: locf → wkd → rolling-zscore(25)

cd /home/ubuntu/blisp
./blisp --ir-only -e '(wzs 25 1 (wkd (locf (file "../ES1I.csv"))))'
