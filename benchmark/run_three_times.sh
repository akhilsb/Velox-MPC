#!/bin/bash
for i in {1..3}
do
	echo "Running iteration $i"
	fab rerun
	sleep 30s
	fab logs
	mkdir logs/$i
	mv logs/*.log logs/$i/
	fab kill
done
